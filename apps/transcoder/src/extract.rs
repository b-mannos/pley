// SPDX-License-Identifier: LicenseRef-Restricted
//! One-shot extractions: video frame thumbnails, embedded cover art, and
//! subtitles converted to WebVTT (read block by block through a Matroska
//! file's Cues, or else all text tracks of a file in one FFmpeg pass, since
//! extracting any track that way means reading the whole file).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use futures::future::{BoxFuture, FutureExt, Shared};
use parking_lot::Mutex;
use pley_transcoder_api::{MediaInfo, MediaStream, ThumbnailRequest, Tonemapper};
use tokio::sync::Semaphore;

use crate::error::{ApiError, ApiResult};
use crate::ffmpeg::Ffmpeg;
use crate::matroska;
use crate::metrics::Metrics;
use crate::plan::{MAX_DURATION_MS, TextSource, ZSCALE_TONEMAP, untagged_hdr_fix};
use crate::probe::Prober;
use crate::util::{MEDIA_INPUT_OPTIONS, file_key, input_arg, secs, touch, write_atomically};
use crate::webvtt::{self, Event};

pub struct Extractor {
    ff: Arc<Ffmpeg>,
    prober: Arc<Prober>,
    metrics: Arc<Metrics>,
    subs_dir: PathBuf,
    /// Quick FFmpeg runs: thumbnails, cover art, subtitle files.
    limiter: Semaphore,
    /// Text tracks read through the Cues at once (each with threads of its
    /// own): a read holds its permit until it is over.
    cue_reads: Arc<Semaphore>,
    /// FFmpeg passes over whole files for their text tracks, which take
    /// hours for a big file on a slow share: apart from the quick runs, so
    /// they never hold those up.
    passes: Semaphore,
    /// The text tracks being extracted, by file version and index: the
    /// requests for a track wait for its extraction, which goes on without
    /// them...
    extractions: Mutex<HashMap<String, Extraction>>,
    next_extraction: AtomicU64,
    /// ...and one FFmpeg pass per file (it writes every text track).
    file_locks: Locks,
    /// The latest text tracks the Cues couldn't give (by file version and
    /// index): FFmpeg extracts them without another try.
    unreadable: Mutex<VecDeque<String>>,
    /// The latest tracks whose pass over the whole file failed (by file
    /// version and index), when, and why: for a while, their requests hear
    /// why rather than start another pass each (players ask every few
    /// seconds, a pass on a slow share takes hours).
    failed_passes: Mutex<VecDeque<(String, Instant, Arc<ApiError>)>>,
    /// How often a pass marks its outputs as being written.
    keep_fresh: Duration,
    /// How long a pass over a file may take.
    pass_time: fn(&Path) -> Duration,
}

/// FFmpeg passes over whole files at once: each reads a file from end to
/// end, so more would only share the disk or the network between them.
const PASSES: usize = 2;
/// How many text tracks the Cues couldn't give are remembered.
const UNREADABLE_KEPT: usize = 1_000;
/// How long a track whose pass failed answers with that failure.
const FAILED_PASS_KEPT: Duration = Duration::from_secs(30 * 60);
/// How many failed passes are remembered.
const FAILED_PASSES_KEPT: usize = 1_000;

pub struct Image {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

impl Extractor {
    pub fn new(
        ff: Arc<Ffmpeg>,
        prober: Arc<Prober>,
        metrics: Arc<Metrics>,
        subs_dir: PathBuf,
        concurrency: usize,
    ) -> Self {
        let _ = std::fs::create_dir_all(&subs_dir);
        Self {
            ff,
            prober,
            metrics,
            subs_dir,
            limiter: Semaphore::new(concurrency.max(1)),
            cue_reads: Arc::new(Semaphore::new(concurrency.max(1))),
            passes: Semaphore::new(PASSES),
            extractions: Mutex::default(),
            next_extraction: AtomicU64::new(0),
            file_locks: Mutex::default(),
            unreadable: Mutex::default(),
            failed_passes: Mutex::default(),
            keep_fresh: KEEP_FRESH,
            pass_time: whole_file_timeout,
        }
    }

    pub async fn thumbnail(&self, req: &ThumbnailRequest) -> ApiResult<Image> {
        let path = existing(&req.path)?;
        let hint = req.duration_ms.filter(|d| (1..=MAX_DURATION_MS).contains(d));
        // The probe (cached per file version) gives the default position, and
        // the colours of HDR video.
        let media = if req.hdr || (hint.is_none() && req.time_ms.is_none()) {
            self.prober.probe(&path).await.ok()
        } else {
            None
        };
        let duration = hint.or_else(|| media.as_ref().and_then(|m| m.duration_ms));
        let at = req.time_ms.unwrap_or_else(|| duration.map(|d| d / 5).unwrap_or(60_000));
        let at = match duration {
            Some(d) if d > 2_000 => at.clamp(0, d - 1_000),
            _ => at.max(0),
        };
        let width = req.width.unwrap_or(640).clamp(64, 1920);
        let plain = Picture::plain(width);
        let picture = if req.hdr {
            Picture::hdr(width, media.as_deref().and_then(MediaInfo::video), &self.ff.caps.tonemappers)
        } else {
            plain.clone()
        };

        let _permit = self.limiter.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        Metrics::inc(&self.metrics.thumbnails);
        let (picture, mut bytes) = match self.grab_frame(&path, at, &picture).await {
            // Tone mapping can fail where a plain frame doesn't (no Vulkan
            // device, colours it can't convert): HDR colours beat no picture.
            Err(err) if picture.tone_mapped => {
                tracing::warn!("tone-mapped frame grab of {} failed, taking it as it is: {err:?}", path.display());
                let bytes = self.grab_frame(&path, at, &plain).await?;
                (plain, bytes)
            }
            result => (picture, result?),
        };
        if bytes.is_empty() && at > 0 {
            // Some files cannot seek precisely; fall back to the first frame.
            bytes = self.grab_frame(&path, 0, &picture).await?;
        }
        if bytes.is_empty() {
            return Err(ApiError::Failed("no frame could be decoded".into()));
        }
        Ok(Image { bytes, content_type: "image/jpeg" })
    }

    async fn grab_frame(&self, path: &Path, at_ms: i64, picture: &Picture) -> ApiResult<Vec<u8>> {
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error"]).args(picture.init);
        if at_ms > 0 {
            cmd.args(["-ss", &secs(at_ms)]);
        }
        cmd.args(MEDIA_INPUT_OPTIONS)
            .arg("-i")
            .arg(input_arg(path))
            .args(["-map", "0:V:0", "-frames:v", "1", "-an", "-sn", "-dn", "-vf", &picture.vf])
            .args(["-c:v", "mjpeg", "-q:v", "3", "-f", "image2pipe", "pipe:1"]);
        run_capture(cmd, Duration::from_secs(60)).await.map(|(out, _)| out)
    }

    /// Embedded cover art of an audio/video file.
    pub async fn cover(&self, path: &str) -> ApiResult<Image> {
        let path = existing(path)?;
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error"]).args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(&path)).args([
            "-map",
            "0:v:0",
            "-c",
            "copy",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "pipe:1",
        ]);
        let _permit = self.limiter.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        let (bytes, _) = run_capture(cmd, Duration::from_secs(30)).await?;
        let content_type = sniff_image(&bytes).ok_or_else(|| ApiError::NotFound("no embedded cover art".into()))?;
        Ok(Image { bytes, content_type })
    }

    /// A subtitle track converted to WebVTT. `stream = None` means `path` is a
    /// subtitle file itself.
    pub async fn subtitle(self: &Arc<Self>, path: &str, stream: Option<i64>) -> ApiResult<String> {
        let path = existing(path)?;
        match stream {
            None => {
                Metrics::inc(&self.metrics.subtitle_extractions);
                self.convert_file(&path).await
            }
            Some(index) => self.embedded(&path, index).await,
        }
    }

    /// A subtitle track copied as it is into a Matroska file of its own, for
    /// players that show picture subtitles (PGS, VobSub, DVB) next to an HLS
    /// stream; VobSub keeps its palette, which is in the track's codec
    /// private data. Times are the original's from the start of the file
    /// (FFmpeg takes the file's start time off, as the HLS streams do), so
    /// the track lines up with the playback position. `stream = None`: `path`
    /// is a subtitle file itself (`.sup`, `.idx`). Only subtitles come out:
    /// any other track leaves FFmpeg nothing to write, and it fails. Copying
    /// a track out reads the whole file, so this waits for one of the passes
    /// over whole files. Written to a file, not a pipe, so the Matroska has
    /// its Cues and duration and players can seek in it.
    pub async fn subtitle_matroska(&self, path: &str, stream: Option<i64>) -> ApiResult<Vec<u8>> {
        let path = existing(path)?;
        let map = match stream {
            Some(index) => format!("0:{index}"),
            None => "0:s:0".to_owned(),
        };
        let out = self.subs_dir.join(format!(".{}.mks.tmp", uuid::Uuid::new_v4().simple()));
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error"])
            .args(MEDIA_INPUT_OPTIONS)
            .arg("-i")
            .arg(input_arg(&path))
            .args(["-map", &map, "-vn", "-an", "-dn", "-c", "copy", "-f", "matroska", "-y"])
            // Stopped just past the most that is read back.
            .args(["-fs", &(MAX_EXTRACTED + 1).to_string()])
            .arg(input_arg(&out));
        let _permit = self.passes.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        Metrics::inc(&self.metrics.subtitle_extractions);
        let result = match run_capture(cmd, (self.pass_time)(&path)).await {
            Ok(_) => match tokio::fs::metadata(&out).await {
                Ok(made) if made.len() > MAX_EXTRACTED => {
                    Err(ApiError::Failed(format!("the subtitle track is over {} MB", MAX_EXTRACTED >> 20)))
                }
                _ => tokio::fs::read(&out).await.map_err(ApiError::from),
            },
            Err(err) => Err(err),
        };
        let _ = tokio::fs::remove_file(&out).await;
        result
    }

    /// An embedded text track's WebVTT as far as it has been pulled out of the
    /// file, and whether that is all of it, after `wait` at most: the track's
    /// extraction (started now, unless it is under way) runs on its own, past
    /// this request. A Matroska file whose Cues list the track's blocks is
    /// read whole by then, as a rule. Otherwise FFmpeg reads the whole file
    /// (minutes for a big one on a network share): once it does, the cues it
    /// has found come at once, and players ask again for those found since.
    pub async fn subtitle_so_far(
        self: &Arc<Self>,
        path: &str,
        index: i64,
        wait: Duration,
    ) -> ApiResult<(String, bool)> {
        let path = existing(path)?;
        let key = file_key(&path)?;
        let target = self.subs_dir.join(format!("{key}_{index}.vtt"));
        if let Some(text) = cached_vtt(&target).await {
            return Ok((text, true));
        }
        let extraction = self.extraction(&path, &key, index);
        if !extraction.by_ffmpeg.load(Ordering::Relaxed)
            && let Ok(done) = tokio::time::timeout(wait, extraction.done()).await
        {
            return done.map(|text| (text, true));
        }
        // Still under way: the cues FFmpeg has found, when it is reading the file.
        let part = self.subs_dir.join(format!("{key}_{index}.vtt.part"));
        match tokio::fs::read_to_string(&part).await {
            Ok(text) => Ok((whole_cues(&text), false)),
            // Finished (and renamed) meanwhile, or not started yet.
            Err(_) => match cached_vtt(&target).await {
                Some(text) => Ok((text, true)),
                None => Ok(("WEBVTT\n\n".to_owned(), false)),
            },
        }
    }

    async fn convert_file(&self, path: &Path) -> ApiResult<String> {
        let size = tokio::fs::metadata(path).await?.len();
        if size > MAX_SUBTITLE_FILE {
            return Err(ApiError::BadRequest(format!(
                "the subtitle file is {} MB: text subtitles take {} MB at most",
                size >> 20,
                MAX_SUBTITLE_FILE >> 20
            )));
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let head = if ext == "vtt" {
            match String::from_utf8(tokio::fs::read(path).await?) {
                Ok(text) => {
                    let text = text.trim_start_matches('\u{feff}').to_owned();
                    return Ok(if text.starts_with("WEBVTT") { text } else { format!("WEBVTT\n\n{text}") });
                }
                Err(err) => {
                    let mut raw = err.into_bytes();
                    raw.truncate(CHARSET_SAMPLE);
                    raw
                }
            }
        } else {
            read_head(path, CHARSET_SAMPLE).await?
        };
        let key = file_key(path)?;
        // Not `_file.vtt`: earlier versions wrote those in place, so a crash
        // or a full disk may have left them truncated.
        let cached = self.subs_dir.join(format!("{key}_sidecar.vtt"));
        if let Some(text) = cached_vtt(&cached).await {
            return Ok(text);
        }
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error"]);
        if let Some(charset) = guess_charset(&head) {
            cmd.args(["-sub_charenc", charset]);
        }
        cmd.args(MEDIA_INPUT_OPTIONS)
            .arg("-i")
            .arg(input_arg(path))
            .args(["-map", "0:s:0", "-c:s", "webvtt", "-f", "webvtt", "pipe:1"]);
        let _permit = self.limiter.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        let (out, _) = run_capture(cmd, Duration::from_secs(60)).await?;
        let text = String::from_utf8_lossy(&out).into_owned();
        if !text.starts_with("WEBVTT") {
            return Err(ApiError::Failed("subtitle conversion produced no output".into()));
        }
        // Cached whole or not at all: a half-written file would keep being served.
        let data = text.clone().into_bytes();
        let write = tokio::task::spawn_blocking(move || write_atomically(&cached, &data)).await;
        if let Ok(Err(err)) = write {
            tracing::debug!("could not cache converted subtitles of {}: {err}", path.display());
        }
        Ok(text)
    }

    /// An embedded text track, whole: see [`Extractor::extract`].
    async fn embedded(self: &Arc<Self>, path: &Path, index: i64) -> ApiResult<String> {
        let key = file_key(path)?;
        if let Some(text) = cached_vtt(&self.subs_dir.join(format!("{key}_{index}.vtt"))).await {
            return Ok(text);
        }
        self.extraction(path, &key, index).done().await
    }

    /// The extraction of text track `index` of a file version (`key`) under
    /// way, or one started now. It runs on its own, to the cache: requests
    /// waiting for it may go away. A track whose pass over the file failed
    /// lately has that failure for an answer.
    fn extraction(self: &Arc<Self>, path: &Path, key: &str, index: i64) -> Extraction {
        let track = format!("{key}_{index}");
        let mut extractions = self.extractions.lock();
        if let Some(running) = extractions.get(&track) {
            return running.clone();
        }
        if let Some(err) = self.failed_pass(&track) {
            let failed = async move { Err(err) };
            return Extraction { id: u64::MAX, by_ffmpeg: Arc::default(), result: failed.boxed().shared() };
        }
        Metrics::inc(&self.metrics.subtitle_extractions);
        let id = self.next_extraction.fetch_add(1, Ordering::Relaxed);
        let by_ffmpeg = Arc::new(AtomicBool::new(false));
        let (this, path, key, entry) = (self.clone(), path.to_owned(), key.to_owned(), track.clone());
        let passing = by_ffmpeg.clone();
        let task = tokio::spawn(async move {
            let over = Forget { extractor: this.clone(), track: entry, id };
            let result = this.extract(&path, &key, index, &passing).await.map(Arc::new).map_err(Arc::new);
            // Before the extraction is over: no request in between starts
            // another pass.
            if let Err(err) = &result
                && passing.load(Ordering::Relaxed)
            {
                this.remember_failed_pass(&over.track, err.clone());
            }
            result
        });
        let result = async move { task.await.unwrap_or_else(|err| Err(Arc::new(ApiError::Internal(err.into())))) };
        let extraction = Extraction { id, by_ffmpeg, result: result.boxed().shared() };
        extractions.insert(track, extraction.clone());
        extraction
    }

    /// Why the latest pass over a whole file for `track` failed, when it did
    /// within [`FAILED_PASS_KEPT`].
    fn failed_pass(&self, track: &str) -> Option<Arc<ApiError>> {
        let failed = self.failed_passes.lock();
        let (_, at, err) = failed.iter().rev().find(|(t, ..)| t == track)?;
        (at.elapsed() < FAILED_PASS_KEPT).then(|| err.clone())
    }

    fn remember_failed_pass(&self, track: &str, err: Arc<ApiError>) {
        tracing::debug!("subtitle track {track}: its pass failed, said for {} min", FAILED_PASS_KEPT.as_secs() / 60);
        let mut failed = self.failed_passes.lock();
        failed.retain(|(t, at, _)| t != track && at.elapsed() < FAILED_PASS_KEPT);
        if failed.len() >= FAILED_PASSES_KEPT {
            failed.pop_front();
        }
        failed.push_back((track.to_owned(), Instant::now(), err));
    }

    /// Text track `index` of a file, cached: read through its Cues when they
    /// list its blocks (the track asked for alone: each read is a seek), and
    /// otherwise with every other text track of the file in a single pass of
    /// FFmpeg over it (`by_ffmpeg` is set then).
    async fn extract(&self, path: &Path, key: &str, index: i64, by_ffmpeg: &AtomicBool) -> ApiResult<String> {
        // Made meanwhile: by an extraction just over, or another track's pass.
        if let Some(text) = cached_vtt(&self.subs_dir.join(format!("{key}_{index}.vtt"))).await {
            return Ok(text);
        }
        let media = self.text_track(path, index).await?;
        match self.through_cues(path, key, &media, index).await? {
            Some(text) => Ok(text),
            None => {
                by_ffmpeg.store(true, Ordering::Relaxed);
                self.through_ffmpeg(path, key, &media, index).await
            }
        }
    }

    /// Text track `index` of a file read through its Cues, and cached; `None`
    /// when the file doesn't allow that (FFmpeg has to read it then).
    async fn through_cues(
        &self,
        path: &Path,
        key: &str,
        media: &Arc<MediaInfo>,
        index: i64,
    ) -> ApiResult<Option<String>> {
        let track = format!("{key}_{index}");
        // Only Matroska files have Cues, and a track they couldn't give isn't
        // tried again: no read waits for its turn for those.
        let matroska = media.format_name.as_deref().is_some_and(|names| names.split(',').any(|n| n == "matroska"));
        if !matroska || self.unreadable.lock().contains(&track) {
            return Ok(None);
        }
        let permit = self.cue_reads.clone().acquire_owned().await.map_err(|e| ApiError::Internal(e.into()))?;
        let (file, media, target) = (path.to_owned(), media.clone(), self.subs_dir.join(format!("{track}.vtt")));
        let read = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            read_through_cues(&file, &media, index, &target)
        })
        .await
        .unwrap_or_else(|e| Err(format!("the read through the cues failed: {e}")));
        match read {
            Ok(text) => Ok(Some(text)),
            Err(reason) => {
                tracing::debug!("subtitles of {} extracted by ffmpeg: {reason}", path.display());
                let mut unreadable = self.unreadable.lock();
                if unreadable.len() == UNREADABLE_KEPT {
                    unreadable.pop_front();
                }
                unreadable.push_back(track);
                Ok(None)
            }
        }
    }

    /// Text track `index` of a file as FFmpeg extracts it, with every other
    /// text track of the file in the same pass over it.
    async fn through_ffmpeg(&self, path: &Path, key: &str, media: &MediaInfo, index: i64) -> ApiResult<String> {
        let target = self.subs_dir.join(format!("{key}_{index}.vtt"));
        let entry = Lock::acquire(&self.file_locks, key);
        let _guard = entry.lock.lock().await;
        if let Some(text) = cached_vtt(&target).await {
            return Ok(text);
        }
        let text_streams: Vec<i64> = media.streams.iter().filter(|s| s.is_text_subtitle()).map(|s| s.index).collect();
        let _permit = self.passes.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        match self.extract_vtt(path, key, &text_streams).await {
            Err(err) if retry_alone(&err, text_streams.len()) => {
                tracing::debug!("subtitles of {} extracted alone: {err:?}", path.display());
                self.extract_vtt(path, key, &[index]).await?;
            }
            all => all?,
        }
        Ok(tokio::fs::read_to_string(&target).await?)
    }

    /// The media facts of a file (cached per file version), checking that
    /// stream `index` is a text track.
    async fn text_track(&self, path: &Path, index: i64) -> ApiResult<Arc<MediaInfo>> {
        let media = self.prober.probe(path).await.map_err(|e| ApiError::Failed(format!("probe failed: {e:#}")))?;
        if !media.streams.iter().any(|s| s.index == index && s.is_text_subtitle()) {
            return Err(ApiError::BadRequest(format!("stream {index} is not a text subtitle track")));
        }
        Ok(media)
    }

    async fn extract_vtt(&self, path: &Path, key: &str, streams: &[i64]) -> ApiResult<()> {
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error", "-y"]).args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(path));
        let mut outputs = Vec::new();
        for index in streams {
            let part = self.subs_dir.join(format!("{key}_{index}.vtt.part"));
            // Each cue is written as soon as it is read, for `subtitle_so_far`.
            cmd.args(["-map", &format!("0:{index}"), "-c:s", "webvtt", "-flush_packets", "1", "-f", "webvtt"])
                .arg(&part);
            outputs.push((part, self.subs_dir.join(format!("{key}_{index}.vtt"))));
        }
        let parts: Vec<&Path> = outputs.iter().map(|(part, _)| part.as_path()).collect();
        let result = kept_fresh(run_capture(cmd, (self.pass_time)(path)), &parts, self.keep_fresh).await;
        for (part, done) in &outputs {
            // Only complete files, flushed to disk, get their final name.
            if result.is_ok() && flush(part).await.is_ok() {
                let _ = tokio::fs::rename(part, done).await;
            } else {
                let _ = tokio::fs::remove_file(part).await;
            }
        }
        result.map(|_| ())
    }

    /// Write a subtitle track as ASS into a job directory for libass burn-in.
    /// This runs while the job is made, which the server waits 25 minutes
    /// for: after 20 (a big file on a slow share), it gives up with an error
    /// the server still hears.
    pub async fn extract_ass(&self, input: &Path, source: &TextSource, out: &Path) -> ApiResult<()> {
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error", "-y"]);
        match source {
            TextSource::Embedded(index) => {
                cmd.args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(input)).args(["-map", &format!("0:{index}")]);
            }
            TextSource::File(path) => {
                if let Ok(head) = read_head(path, CHARSET_SAMPLE).await
                    && let Some(charset) = guess_charset(&head)
                {
                    cmd.args(["-sub_charenc", charset]);
                }
                cmd.args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(path)).args(["-map", "0:s:0"]);
            }
        }
        cmd.args(["-c:s", "ass", "-f", "ass"]).arg(out);
        run_capture(cmd, Duration::from_secs(20 * 60)).await.map(|_| ())
    }
}

/// A text track's extraction under way, and which one it is.
#[derive(Clone)]
struct Extraction {
    id: u64,
    /// Set once FFmpeg reads the whole file for the track: that may take
    /// hours, and the cues it has found are in the track's `.part` file.
    by_ffmpeg: Arc<AtomicBool>,
    result: Shared<BoxFuture<'static, Result<Arc<String>, Arc<ApiError>>>>,
}

impl Extraction {
    /// The track's WebVTT once the extraction is over, or why it couldn't be
    /// made.
    async fn done(self) -> ApiResult<String> {
        self.result.await.map(|text| String::clone(&text)).map_err(|err| err.again())
    }
}

/// Takes a track's extraction out of those under way once it's over, however
/// it ends: the next request finds the cache, or tries again after a failure.
/// A later extraction in its place stays.
struct Forget {
    extractor: Arc<Extractor>,
    track: String,
    id: u64,
}

impl Drop for Forget {
    fn drop(&mut self) {
        let mut extractions = self.extractor.extractions.lock();
        if extractions.get(&self.track).is_some_and(|extraction| extraction.id == self.id) {
            extractions.remove(&self.track);
        }
    }
}

/// Text track `index` of a Matroska file, read block by block through its
/// Cues as FFmpeg would have written it, within the limits of such a read,
/// and cached at `target` (it is served even when it can't be); an error says
/// why FFmpeg has to read the file instead. Blocking.
fn read_through_cues(path: &Path, media: &MediaInfo, index: i64, target: &Path) -> Result<String, String> {
    let budget = matroska::Budget::new(CUE_READ_BYTES, CUE_READ_TIME);
    let (_, text) = cue_tracks(path, media, &[index], &budget)?.into_iter().next().ok_or("no track read")?;
    // Cached whole or not at all.
    if let Err(err) = write_atomically(target, text.as_bytes()) {
        tracing::debug!("could not cache the subtitles of {} read through the cues: {err}", path.display());
    }
    Ok(text)
}

/// Whether a pass over every text track of a file that failed with `err` is
/// worth running again for the one asked for alone: when another track
/// couldn't be converted, not when it ran out of time (reading the file
/// again takes as long).
fn retry_alone(err: &ApiError, tracks: usize) -> bool {
    tracks > 1 && !matches!(err, ApiError::Timeout(_))
}

/// How often a pass marks its outputs as being written. The cache's pruning
/// takes temporaries left alone for hours as abandoned, and a pass may run
/// longer than that (a big file on a slow share), with tracks that get no
/// cue for a long time.
const KEEP_FRESH: Duration = Duration::from_secs(10 * 60);

/// `work`, marking the files `parts` as being written every `every` while it
/// runs.
async fn kept_fresh<T>(work: impl Future<Output = T>, parts: &[&Path], every: Duration) -> T {
    tokio::pin!(work);
    let mut tick = tokio::time::interval(every);
    loop {
        tokio::select! {
            done = &mut work => return done,
            _ = tick.tick() => {
                for part in parts {
                    if let Ok(file) = std::fs::File::options().write(true).open(part) {
                        let _ = file.set_modified(SystemTime::now());
                    }
                }
            }
        }
    }
}

/// How long FFmpeg may take to read the whole file at `path`: see
/// [`pass_timeout`].
fn whole_file_timeout(path: &Path) -> Duration {
    pass_timeout(std::fs::metadata(path).map(|m| m.len()).unwrap_or(0))
}

/// 20 minutes, or more for a file too big to read in that time at 10 MB/s
/// (a slow disk or network share; a 70 GB film needs two hours at that pace).
fn pass_timeout(bytes: u64) -> Duration {
    Duration::from_secs((bytes / 10_000_000).max(20 * 60))
}

/// What reading a track through the Cues may keep in memory (see
/// [`matroska::Budget`]). A film's track takes under a megabyte, the most
/// elaborate ASS tens of megabytes; a file whose Cues claim more is left to
/// FFmpeg, which reads it a packet at a time.
const CUE_READ_BYTES: u64 = 64 << 20;
/// How long reading a track through the Cues may take, converting it
/// included: seconds, even on a network share. Longer, FFmpeg reads the file.
const CUE_READ_TIME: Duration = Duration::from_secs(60);

/// The text tracks `streams` of a Matroska file as WebVTT, read through its
/// Cues within `budget`, with FFmpeg's cue times: the file's start time taken
/// off, rounded the way FFmpeg rescales timestamps.
fn cue_tracks(
    path: &Path,
    media: &MediaInfo,
    streams: &[i64],
    budget: &matroska::Budget,
) -> Result<Vec<(i64, String)>, String> {
    let started = Instant::now();
    let found = matroska::text_tracks(path, media, streams, budget)?;
    tracing::debug!(
        "{} subtitle tracks of {} read through the cues in {} ms ({} reads, {} KB)",
        found.tracks.len(),
        path.display(),
        started.elapsed().as_millis(),
        found.reads,
        found.bytes_read / 1024
    );
    as_webvtt(found, media, budget)
}

/// Tracks read through the Cues as the WebVTT files FFmpeg would write.
fn as_webvtt(
    found: matroska::TextTracks,
    media: &MediaInfo,
    budget: &matroska::Budget,
) -> Result<Vec<(i64, String)>, String> {
    let tick_ns = i64::try_from(found.tick_ns).map_err(|_| "the file's timestamp scale is out of range")?;
    let tick = (tick_ns, 1_000_000_000);
    let (us, ms) = ((1, 1_000_000), (1, 1000));
    // FFmpeg takes off the file's start time in microseconds, while the media
    // info keeps it in whole milliseconds (`start_ms`, rounded from ffprobe's
    // microseconds): the same when every time is whole milliseconds.
    // Otherwise a start of a fraction of one (1.833332 s with ticks of
    // 333333 ns) would put some cues 1 ms off FFmpeg's. ffprobe gives any
    // file with audio or video a start time (0 for most), so such a file is
    // left to FFmpeg.
    if !found.whole_ms && media.start_ms.is_some() {
        return Err("the file's start isn't known to the microsecond".into());
    }
    let offset = rescale(-media.start_ms.unwrap_or(0) * 1000, us, tick).ok_or("the file's start is out of range")?;
    let mut out = Vec::new();
    for track in found.tracks {
        let index = track.stream_index;
        let mut events = Vec::with_capacity(track.blocks.len());
        for block in track.blocks {
            let start_ms =
                block.time.checked_add(offset).and_then(|t| rescale(t, tick, us)).and_then(|t| rescale(t, us, ms));
            // FFmpeg keeps how long a cue lasts in 32 bits of milliseconds:
            // a longer one (49 days, a damaged file) is left to it.
            let duration_ms = i64::try_from(block.duration).ok().and_then(|d| rescale(d, tick, ms));
            let (Some(start_ms), Some(duration_ms @ 0..=0xFFFF_FFFF)) = (start_ms, duration_ms) else {
                return Err(format!("stream {index} has a cue at a time out of range"));
            };
            // FFmpeg's muxer would shift a track starting before 0, and
            // refuses one going back in time: left to it.
            if start_ms < 0 || events.last().is_some_and(|e: &Event| e.start_ms > start_ms) {
                return Err(format!("stream {index} has cues before the start or out of order"));
            }
            events.push(Event { start_ms, duration_ms, data: block.data });
        }
        // The WebVTT made so far is kept too.
        let mut held = 0;
        let text = webvtt::to_webvtt(track.codec, &track.header, &events, |size| {
            budget.check()?;
            budget.spend(size.saturating_sub(held) as u64)?;
            held = held.max(size);
            Ok(())
        })?;
        out.push((index, text));
    }
    Ok(out)
}

/// `value` from one time base to another (`(num, den)`), rounded half away
/// from zero, as FFmpeg's `av_rescale_q`; `None` when it doesn't fit.
fn rescale(value: i64, from: (i64, i64), to: (i64, i64)) -> Option<i64> {
    let (b, c) = (from.0 as i128 * to.1 as i128, from.1 as i128 * to.0 as i128);
    let n = (value as i128 * b).abs();
    let q = (n + c / 2) / c;
    i64::try_from(if value < 0 { -q } else { q }).ok()
}

const VULKAN_DEVICE: [&str; 4] = ["-init_hw_device", "vulkan=vk", "-filter_hw_device", "vk"];

/// How a frame grab turns video into the picture: its filters, and the global
/// options they need.
#[derive(Debug, Clone, PartialEq)]
struct Picture {
    init: &'static [&'static str],
    vf: String,
    tone_mapped: bool,
}

impl Picture {
    fn plain(width: u32) -> Self {
        Self { init: &[], vf: format!("scale={width}:-2:flags=bicubic"), tone_mapped: false }
    }

    /// HDR tone-mapped to SDR the best way available. Dolby Vision without a
    /// compatible base layer (profile 5) only gets its colours right through
    /// libplacebo, which applies the RPU; other HDR is cheaper on the CPU with
    /// zimg, given the colour properties it needs.
    fn hdr(width: u32, video: Option<&MediaStream>, tonemappers: &[Tonemapper]) -> Self {
        let placebo = tonemappers.contains(&Tonemapper::Libplacebo);
        let zscale = tonemappers.contains(&Tonemapper::Zscale);
        if placebo && (!zscale || video.is_some_and(MediaStream::is_dolby_vision_only)) {
            return Self {
                init: &VULKAN_DEVICE,
                vf: format!(
                    "libplacebo=w={width}:h=-2:format=yuv420p:colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv:tonemapping=auto"
                ),
                tone_mapped: true,
            };
        }
        if zscale {
            let filters: Vec<String> = video
                .and_then(untagged_hdr_fix)
                .into_iter()
                .chain([ZSCALE_TONEMAP.to_owned(), format!("scale={width}:-2:flags=bicubic")])
                .collect();
            return Self { init: &[], vf: filters.join(","), tone_mapped: true };
        }
        Self::plain(width)
    }
}

/// A cached WebVTT file, unless it can't be read or doesn't even start like
/// one (then it is extracted again).
async fn cached_vtt(path: &Path) -> Option<String> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    if !text.starts_with("WEBVTT") {
        return None;
    }
    // In use: cache pruning keeps it.
    touch(path);
    Some(text)
}

/// WebVTT being written, up to its last complete cue (cues end with a blank line).
fn whole_cues(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    match text.rfind("\n\n") {
        Some(end) if text.starts_with("WEBVTT") => text[..end + 2].to_owned(),
        _ => "WEBVTT\n\n".to_owned(),
    }
}

/// Make sure a file another process wrote is on disk.
async fn flush(path: &Path) -> std::io::Result<()> {
    // (Windows needs write access to flush.)
    tokio::fs::OpenOptions::new().write(true).open(path).await?.sync_all().await
}

fn existing(path: &str) -> ApiResult<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_file() { Ok(path) } else { Err(ApiError::NotFound(format!("file not found: {}", path.display()))) }
}

/// The most a capture takes from ffmpeg's output (a cover, a thumbnail, a
/// subtitle track): a crafted file's several hundred megabytes of "picture"
/// would otherwise be held whole, here and in the server.
const MAX_CAPTURE: usize = 64 << 20;
/// The most of its log a capture keeps (the rest is read and dropped).
const MAX_CAPTURE_LOG: usize = 1 << 20;
/// The most a subtitle track extracted to a file may be (a Blu-ray's
/// pictures are tens of megabytes).
const MAX_EXTRACTED: u64 = 512 << 20;

/// Run ffmpeg, returning stdout. Non-zero exit becomes `ApiError::Failed`;
/// output past [`MAX_CAPTURE`] stops it.
pub(crate) async fn run_capture(mut cmd: tokio::process::Command, timeout: Duration) -> ApiResult<(Vec<u8>, String)> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| ApiError::Internal(anyhow::anyhow!("failed to start ffmpeg: {e}")))?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(ApiError::Internal(anyhow::anyhow!("ffmpeg's output isn't piped")));
    };
    // The log is read on its own, so that a full pipe never holds ffmpeg up.
    let log = tokio::spawn(read_capped(stderr, MAX_CAPTURE_LOG, true));
    let run = async {
        let (stdout, over) = read_capped(stdout, MAX_CAPTURE, false).await?;
        if over {
            let _ = child.kill().await;
            return Ok((stdout, None));
        }
        child.wait().await.map(|status| (stdout, Some(status)))
    };
    let result = tokio::time::timeout(timeout, run).await;
    let stderr = match log.await {
        Ok(Ok((log, _))) => String::from_utf8_lossy(&log).trim().to_owned(),
        _ => String::new(),
    };
    let (stdout, status) = result
        .map_err(|_| ApiError::Timeout("ffmpeg took too long".into()))?
        .map_err(|e: std::io::Error| ApiError::Internal(e.into()))?;
    let Some(status) = status else {
        return Err(ApiError::Failed(format!("ffmpeg wrote more than {} MB", MAX_CAPTURE >> 20)));
    };
    if !status.success() {
        let message = stderr.lines().last().unwrap_or("ffmpeg failed").to_owned();
        return Err(ApiError::Failed(message));
    }
    Ok((stdout, stderr))
}

/// What `reader` gives, up to `max` bytes: and whether there was more, which
/// is read and dropped with `drain`, else left (the writer is stopped).
async fn read_capped(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    max: usize,
    drain: bool,
) -> std::io::Result<(Vec<u8>, bool)> {
    use tokio::io::AsyncReadExt;
    let mut kept = Vec::new();
    let mut chunk = vec![0; 64 << 10];
    let mut over = false;
    loop {
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            return Ok((kept, over));
        }
        if kept.len() + n > max {
            over = true;
            if !drain {
                return Ok((kept, true));
            }
        } else {
            kept.extend_from_slice(&chunk[..n]);
        }
    }
}

fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}

/// The largest subtitle file converted to WebVTT: a film's text track takes
/// well under a megabyte, the most elaborate ASS tens of them. Anything
/// bigger (a picture track's `.sup`, a file that isn't subtitles) is
/// refused before it is read into memory.
const MAX_SUBTITLE_FILE: u64 = 64 << 20;
/// How much of a subtitle file its charset is told from.
const CHARSET_SAMPLE: usize = 1 << 20;

/// The first `limit` bytes of the file at `path` (all of a shorter one).
async fn read_head(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut head = Vec::new();
    tokio::fs::File::open(path).await?.take(limit as u64).read_to_end(&mut head).await?;
    Ok(head)
}

/// Text subtitle files are frequently not UTF-8. UTF-16 with a BOM is handled by
/// ffmpeg; anything else that is not valid UTF-8 is assumed to be Windows-1252.
/// `raw` may be the start of the file only: a character cut off at its end
/// is no sign of another charset.
fn guess_charset(raw: &[u8]) -> Option<&'static str> {
    let utf8 = std::str::from_utf8(raw).map_or_else(|err| err.error_len().is_none(), |_| true);
    if raw.starts_with(&[0xFF, 0xFE]) || raw.starts_with(&[0xFE, 0xFF]) || utf8 { None } else { Some("CP1252") }
}

pub(crate) type Locks = Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

/// A lock by key (a file's extractions, a set of seek previews), forgotten
/// again once nobody uses it (on every path, including errors).
pub(crate) struct Lock<'a> {
    locks: &'a Locks,
    key: String,
    pub(crate) lock: Arc<tokio::sync::Mutex<()>>,
}

impl<'a> Lock<'a> {
    pub(crate) fn acquire(locks: &'a Locks, key: &str) -> Self {
        let lock = locks.lock().entry(key.to_owned()).or_default().clone();
        Self { locks, key: key.to_owned(), lock }
    }
}

impl Drop for Lock<'_> {
    fn drop(&mut self) {
        let mut locks = self.locks.lock();
        // Only the map and this guard hold it: nobody else is waiting.
        if Arc::strong_count(&self.lock) <= 2 {
            locks.remove(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_images() {
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(sniff_image(b"hello"), None);
    }

    #[test]
    fn hdr_frame_grabs_use_a_tone_mapper_that_works() {
        use pley_transcoder_api::{DynamicRange, StreamKind};
        let p5 = MediaStream {
            kind: StreamKind::Video,
            dynamic_range: Some(DynamicRange::DolbyVision),
            dv_profile: Some(5),
            dv_bl_compat: Some(0),
            ..Default::default()
        };
        let hdr10 = MediaStream {
            kind: StreamKind::Video,
            dynamic_range: Some(DynamicRange::Hdr10),
            color_transfer: Some("smpte2084".into()),
            color_primaries: Some("bt2020".into()),
            color_space: Some("bt2020nc".into()),
            ..Default::default()
        };
        let all = [Tonemapper::Libplacebo, Tonemapper::Opencl, Tonemapper::Zscale];
        // Profile 5 went through zimg, which fails on its untagged frames
        // ("Nothing was written into output file"): libplacebo applies its RPU.
        let picture = Picture::hdr(640, Some(&p5), &all);
        assert!(picture.vf.starts_with("libplacebo=w=640:h=-2:"), "{picture:?}");
        assert_eq!(picture.init, VULKAN_DEVICE.as_slice());
        // Other HDR stays on the CPU.
        let picture = Picture::hdr(640, Some(&hdr10), &all);
        assert_eq!(picture.vf, format!("{ZSCALE_TONEMAP},scale=640:-2:flags=bicubic"));
        assert!(picture.init.is_empty() && picture.tone_mapped);
        // Without libplacebo, profile 5 frames are tagged for zimg: off
        // colours, but a picture.
        let picture = Picture::hdr(640, Some(&p5), &[Tonemapper::Zscale]);
        assert!(
            picture.vf.starts_with("setparams=color_trc=smpte2084:color_primaries=bt2020:colorspace=bt2020nc,zscale="),
            "{picture:?}"
        );
        // Unknown video (the probe failed) or no tone mapper.
        assert_eq!(Picture::hdr(640, None, &[Tonemapper::Libplacebo]).init, VULKAN_DEVICE.as_slice());
        assert_eq!(Picture::hdr(640, Some(&p5), &[Tonemapper::Opencl]), Picture::plain(640));
    }

    /// An extractor whose FFmpeg is `ffmpeg` and whose ffprobe doesn't exist.
    fn extractor_with(dir: &Path, ffmpeg: PathBuf) -> Extractor {
        let ff = Arc::new(Ffmpeg {
            ffmpeg,
            ffprobe: dir.join("no-ffprobe"),
            caps: Default::default(),
            segment_times_relative: true,
        });
        let metrics = Arc::new(Metrics::default());
        let prober = Arc::new(Prober::new(ff.clone(), metrics.clone(), 1));
        Extractor::new(ff, prober, metrics, dir.join("subtitles"), 1)
    }

    /// An extractor whose FFmpeg and ffprobe don't exist.
    fn extractor_without_ffmpeg(dir: &Path) -> Arc<Extractor> {
        Arc::new(extractor_with(dir, dir.join("no-ffmpeg")))
    }

    /// `file`, a version of the synthetic film of the Matroska tests (SubRip
    /// as stream 2, ASS as stream 4), written to `dir` and probed as ffprobe
    /// would see it: its path and its key.
    fn movie_in(extractor: &Extractor, dir: &Path, file: &matroska::tests::TestFile) -> (String, String) {
        let movie = file.write(dir);
        extractor.prober.remember(&movie, matroska::tests::movie_media());
        (movie.to_string_lossy().into_owned(), file_key(&movie).unwrap())
    }

    /// An extractor without FFmpeg, and the synthetic film: its path and key.
    fn with_movie(dir: &Path) -> (Arc<Extractor>, String, String) {
        let extractor = extractor_without_ffmpeg(dir);
        let (name, key) = movie_in(&extractor, dir, &matroska::tests::movie());
        (extractor, name, key)
    }

    /// The synthetic film with Cues that don't say where its blocks are in
    /// their clusters: FFmpeg has to read it.
    fn unplaced_movie() -> matroska::tests::TestFile {
        let mut file = matroska::tests::movie();
        file.no_relative = true;
        file
    }

    /// A stand-in for FFmpeg in `dir`. Each run writes its arguments on a
    /// line of `ffmpeg.log`, then fails at once, or runs on for 10 seconds
    /// when `hang`.
    fn stand_in_ffmpeg(dir: &Path, hang: bool) -> PathBuf {
        let log = dir.join("ffmpeg.log");
        #[cfg(windows)]
        let (path, script) = (
            dir.join("ffmpeg.cmd"),
            format!(
                "@echo %* >> \"{}\"\r\n{}\r\n",
                log.display(),
                if hang { "@ping -n 11 127.0.0.1 > nul" } else { "@exit /b 1" }
            ),
        );
        #[cfg(not(windows))]
        let (path, script) = (
            dir.join("ffmpeg"),
            format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\n{}\n",
                log.display(),
                if hang { "exec sleep 10" } else { "exit 1" }
            ),
        );
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    /// The streams each run of the stand-in FFmpeg in `dir` was given.
    fn ffmpeg_runs(dir: &Path) -> Vec<Vec<i64>> {
        let log = std::fs::read_to_string(dir.join("ffmpeg.log")).unwrap_or_default();
        log.lines()
            .map(|line| {
                let args: Vec<&str> = line.split_whitespace().map(|arg| arg.trim_matches('"')).collect();
                let mapped = args.windows(2).filter(|pair| pair[0] == "-map");
                mapped.filter_map(|pair| pair[1].strip_prefix("0:")?.parse().ok()).collect()
            })
            .collect()
    }

    fn extractions(extractor: &Extractor) -> u64 {
        extractor.metrics.subtitle_extractions.load(Ordering::Relaxed)
    }

    /// Waits for `check` to hold (work running on its own), 10 seconds at most.
    async fn eventually(check: impl Fn() -> bool) {
        for _ in 0..1_000 {
            if check() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("condition not reached");
    }

    #[tokio::test]
    async fn subtitle_caches_are_whole_or_absent() {
        let dir = tempfile::tempdir().unwrap();
        let extractor = extractor_without_ffmpeg(dir.path());
        let subs = dir.path().join("subtitles");
        let srt = dir.path().join("movie.en.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").unwrap();
        let name = srt.to_string_lossy().into_owned();

        // The conversion fails (no ffmpeg here): nothing is cached.
        assert!(extractor.subtitle(&name, None).await.is_err());
        assert_eq!(std::fs::read_dir(&subs).unwrap().count(), 0);
        // A cache entry cut short (a crash, a full disk) is not served…
        let cached = subs.join(format!("{}_sidecar.vtt", file_key(&srt).unwrap()));
        std::fs::write(&cached, "WEB").unwrap();
        assert!(extractor.subtitle(&name, None).await.is_err());
        // …a whole one is.
        std::fs::write(&cached, "WEBVTT\n\n00:01.000 --> 00:02.000\nHello\n").unwrap();
        assert!(extractor.subtitle(&name, None).await.unwrap().contains("Hello"));
    }

    #[tokio::test]
    async fn embedded_subtitles_come_as_far_as_they_are_read() {
        use pley_transcoder_api::StreamKind;
        let dir = tempfile::tempdir().unwrap();
        let extractor = extractor_without_ffmpeg(dir.path());
        let subs = dir.path().join("subtitles");
        let wait = Duration::from_millis(200);

        // An extraction that can't run (no ffprobe here) says so.
        let unknown = dir.path().join("unknown.mkv");
        std::fs::write(&unknown, "not really a movie").unwrap();
        assert!(extractor.subtitle_so_far(&unknown.to_string_lossy(), 2, wait).await.is_err());

        // A file without Cues: FFmpeg reads all of it for the track, in a
        // pass that waits for its turn (every pass is taken here)...
        let movie = dir.path().join("movie.mp4");
        std::fs::write(&movie, "not really a movie either").unwrap();
        let streams = vec![
            matroska::tests::stream(0, StreamKind::Video, "h264"),
            matroska::tests::stream(1, StreamKind::Subtitle, "subrip"),
        ];
        extractor.prober.remember(
            &movie,
            MediaInfo { format_name: Some("mov,mp4,m4a,3gp,3g2,mj2".into()), streams, ..Default::default() },
        );
        let (name, key) = (movie.to_string_lossy().into_owned(), file_key(&movie).unwrap());
        let passes = extractor.passes.acquire_many(PASSES as u32).await.unwrap();
        let started = extractions(&extractor);
        assert_eq!(extractor.subtitle_so_far(&name, 1, wait).await.unwrap(), ("WEBVTT\n\n".to_owned(), false));
        eventually(|| extractor.file_locks.lock().get(&key).is_some_and(|lock| lock.try_lock().is_err())).await;
        // ...then the cues it has read so far come at once (the pass may take
        // hours), the one being written left out, from that same extraction...
        std::fs::write(
            subs.join(format!("{key}_1.vtt.part")),
            "WEBVTT\r\n\r\n00:01.000 --> 00:02.000\r\nHello\r\n\r\n00:03.0",
        )
        .unwrap();
        let so_far = extractor.subtitle_so_far(&name, 1, Duration::from_secs(60));
        let (text, complete) = tokio::time::timeout(Duration::from_secs(10), so_far).await.expect("at once").unwrap();
        assert_eq!((text.as_str(), complete), ("WEBVTT\n\n00:01.000 --> 00:02.000\nHello\n\n", false));
        assert_eq!(extractions(&extractor), started + 1);
        // ...and once it's done, all of it.
        std::fs::write(
            subs.join(format!("{key}_1.vtt")),
            "WEBVTT\n\n00:01.000 --> 00:02.000\nHello\n\n00:03.000 --> 00:04.000\nBye\n",
        )
        .unwrap();
        let (text, complete) = extractor.subtitle_so_far(&name, 1, wait).await.unwrap();
        assert!(complete && text.contains("Bye"), "{text}");
        drop(passes);
    }

    #[tokio::test]
    async fn subtitles_read_through_the_cues_come_whole_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let (extractor, name, key) = with_movie(dir.path());
        let whole = "WEBVTT\n\n00:01.000 --> 00:02.500\nHello\n\n00:04.800 --> 00:05.800\nBefore the cluster\n\n\
            00:09.002 --> 00:09.002\nNo duration\n";

        // The first answer has all of it, so the player stops asking.
        let wait = Duration::from_secs(10);
        assert_eq!(extractor.subtitle_so_far(&name, 2, wait).await.unwrap(), (whole.to_owned(), true));
        let cached = dir.path().join("subtitles").join(format!("{key}_2.vtt"));
        assert_eq!(std::fs::read_to_string(cached).unwrap(), whole);
        // Whole-track requests read them the same way.
        assert!(extractor.subtitle(&name, Some(4)).await.unwrap().ends_with("\nHi\n"));
        assert!(matches!(extractor.subtitle(&name, Some(3)).await, Err(ApiError::BadRequest(_))), "PGS isn't text");
    }

    #[tokio::test]
    async fn tracks_of_a_file_are_read_side_by_side() {
        let dir = tempfile::tempdir().unwrap();
        let (extractor, name, key) = with_movie(dir.path());
        let soon = Duration::from_secs(10);
        // While stream 2 is being extracted...
        let (finish, finished) = tokio::sync::oneshot::channel::<String>();
        let result = async move { Ok::<_, Arc<ApiError>>(Arc::new(finished.await.unwrap_or_default())) };
        let running = Extraction { id: u64::MAX, by_ffmpeg: Arc::default(), result: result.boxed().shared() };
        extractor.extractions.lock().insert(format!("{key}_2"), running);
        // ...stream 4 is read all the same,
        let ass = tokio::time::timeout(soon, extractor.subtitle(&name, Some(4))).await.expect("not held up");
        assert!(ass.unwrap().ends_with("\nHi\n"));
        // and requests for stream 2 wait for that extraction rather than
        // start another one.
        let again = extractor.subtitle(&name, Some(2));
        tokio::pin!(again);
        assert!(tokio::time::timeout(Duration::from_millis(200), &mut again).await.is_err(), "waits");
        let wait = Duration::from_millis(50);
        assert_eq!(extractor.subtitle_so_far(&name, 2, wait).await.unwrap(), ("WEBVTT\n\n".to_owned(), false));
        finish.send("WEBVTT\n\nread meanwhile\n".to_owned()).unwrap();
        assert_eq!(again.await.unwrap(), "WEBVTT\n\nread meanwhile\n");
        assert_eq!(extractions(&extractor), 1, "stream 4's");
    }

    #[tokio::test]
    async fn tracks_the_cues_list_come_whole_while_ffmpeg_reads_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let (extractor, name, key) = with_movie(dir.path());
        // FFmpeg reading the whole file for another of its tracks (one the
        // Cues can't give), or waiting for its turn to...
        let pass = Lock::acquire(&extractor.file_locks, &key);
        let _reading = pass.lock.lock().await;
        // ...the SubRip track still comes whole, read through the Cues, rather
        // than as far as that pass has got.
        let (text, complete) = extractor.subtitle_so_far(&name, 2, Duration::from_secs(10)).await.unwrap();
        assert!(complete && text.contains("Before the cluster"), "{text}");
    }

    #[tokio::test]
    async fn reads_go_on_when_their_requests_go_away() {
        let dir = tempfile::tempdir().unwrap();
        let (extractor, name, key) = with_movie(dir.path());
        let cached = dir.path().join("subtitles").join(format!("{key}_2.vtt"));
        // While other tracks being read take every turn...
        let turns = extractor.cue_reads.clone().acquire_owned().await.unwrap();
        // ...a player asking for this one hears it isn't there yet, and asks
        // again,
        let wait = Duration::from_millis(50);
        for _ in 0..2 {
            assert_eq!(extractor.subtitle_so_far(&name, 2, wait).await.unwrap(), ("WEBVTT\n\n".to_owned(), false));
        }
        // and a whole-track request stops waiting (its client went away).
        assert!(tokio::time::timeout(wait, extractor.subtitle(&name, Some(2))).await.is_err());
        // The one read they started goes on when its turn comes, to the
        // cache.
        drop(turns);
        eventually(|| cached.exists()).await;
        assert!(std::fs::read_to_string(&cached).unwrap().contains("Before the cluster"));
        assert_eq!(extractions(&extractor), 1, "one for all the requests");
        let (text, complete) = extractor.subtitle_so_far(&name, 2, wait).await.unwrap();
        assert!(complete && text.contains("Before the cluster"), "{text}");
    }

    #[tokio::test]
    async fn long_passes_hold_up_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let (extractor, name, _) = with_movie(dir.path());
        let soon = Duration::from_secs(10);
        // Every FFmpeg pass over a whole file taken (hours each on a slow
        // share)...
        let _passes = extractor.passes.acquire_many(PASSES as u32).await.unwrap();
        // ...tracks read through the Cues come all the same...
        let subrip = tokio::time::timeout(soon, extractor.subtitle(&name, Some(2))).await.expect("not held up");
        assert!(subrip.unwrap().contains("Hello"));
        // ...and quick FFmpeg runs don't wait for them (they fail here: no
        // FFmpeg).
        let srt = dir.path().join("movie.en.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello\n").unwrap();
        let srt = srt.to_string_lossy().into_owned();
        assert!(tokio::time::timeout(soon, extractor.subtitle(&srt, None)).await.expect("not held up").is_err());
        assert!(tokio::time::timeout(soon, extractor.cover(&name)).await.expect("not held up").is_err());
    }

    #[tokio::test]
    async fn quick_runs_hold_up_no_pass() {
        let dir = tempfile::tempdir().unwrap();
        let extractor = Arc::new(extractor_with(dir.path(), stand_in_ffmpeg(dir.path(), false)));
        let (name, _) = movie_in(&extractor, dir.path(), &unplaced_movie());
        // Every quick FFmpeg run taken (thumbnails, cover art, subtitle
        // files)...
        let _quick = extractor.limiter.acquire().await.unwrap();
        // ...a pass over a file whose Cues can't be read runs all the same
        // (and fails here).
        let pass = tokio::time::timeout(Duration::from_secs(30), extractor.subtitle(&name, Some(2))).await;
        assert!(pass.expect("not held up").is_err());
        assert_eq!(ffmpeg_runs(dir.path()).first(), Some(&vec![2, 4]), "every text track at once");
    }

    #[tokio::test]
    async fn subtitles_read_through_the_cues_are_served_when_they_cant_be_cached() {
        let dir = tempfile::tempdir().unwrap();
        let (extractor, name, key) = with_movie(dir.path());
        // Something in the way of the cache file (a full disk, a file in use).
        std::fs::create_dir(dir.path().join("subtitles").join(format!("{key}_2.vtt"))).unwrap();
        // Served from memory, not left to FFmpeg (which would fail here).
        assert!(extractor.subtitle(&name, Some(2)).await.unwrap().contains("Before the cluster"));
    }

    #[tokio::test]
    async fn passes_that_ran_out_of_time_arent_run_again() {
        // A pass that fails is run again for the track asked for alone:
        // another track may be what FFmpeg couldn't convert...
        let dir = tempfile::tempdir().unwrap();
        let extractor = Arc::new(extractor_with(dir.path(), stand_in_ffmpeg(dir.path(), false)));
        let (name, _) = movie_in(&extractor, dir.path(), &unplaced_movie());
        assert!(matches!(extractor.subtitle(&name, Some(2)).await, Err(ApiError::Failed(_))));
        assert_eq!(ffmpeg_runs(dir.path()), [vec![2, 4], vec![2]]);
        // ...but not one that ran out of time: reading the file again would
        // take as long.
        let dir = tempfile::tempdir().unwrap();
        let mut slow = extractor_with(dir.path(), stand_in_ffmpeg(dir.path(), true));
        slow.pass_time = |_| Duration::from_secs(2);
        let slow = Arc::new(slow);
        let (name, _) = movie_in(&slow, dir.path(), &unplaced_movie());
        assert!(matches!(slow.subtitle(&name, Some(2)).await, Err(ApiError::Timeout(_))));
        assert_eq!(ffmpeg_runs(dir.path()), [vec![2, 4]]);
    }

    #[tokio::test]
    async fn a_failed_pass_is_told_rather_than_run_again() {
        let dir = tempfile::tempdir().unwrap();
        let extractor = Arc::new(extractor_with(dir.path(), stand_in_ffmpeg(dir.path(), false)));
        let (name, key) = movie_in(&extractor, dir.path(), &unplaced_movie());
        let wait = Duration::from_millis(100);
        // A player's first look starts the pass (which fails here, run
        // again for the track alone)...
        let _ = extractor.subtitle_so_far(&name, 2, wait).await;
        eventually(|| extractor.extractions.lock().is_empty()).await;
        assert_eq!(ffmpeg_runs(dir.path()), [vec![2, 4], vec![2]]);
        // ...and the next ones hear why, rather than read the file again.
        for _ in 0..3 {
            assert!(matches!(extractor.subtitle_so_far(&name, 2, wait).await, Err(ApiError::Failed(_))));
        }
        assert!(matches!(extractor.subtitle(&name, Some(2)).await, Err(ApiError::Failed(_))));
        assert_eq!(ffmpeg_runs(dir.path()).len(), 2);
        // Another track's failure isn't this one's.
        let track = format!("{key}_2");
        assert!(extractor.failed_pass(&track).is_some() && extractor.failed_pass(&format!("{key}_4")).is_none());
        // For a while: then a pass is tried again.
        let Some(long_ago) = Instant::now().checked_sub(FAILED_PASS_KEPT) else { return };
        extractor.failed_passes.lock()[0].1 = long_ago;
        assert!(extractor.failed_pass(&track).is_none());
        assert!(extractor.subtitle(&name, Some(2)).await.is_err());
        assert_eq!(ffmpeg_runs(dir.path()).len(), 4);
        assert!(extractor.failed_pass(&track).is_some());
    }

    #[tokio::test]
    async fn passes_keep_their_outputs_from_being_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let mut extractor = extractor_with(dir.path(), stand_in_ffmpeg(dir.path(), true));
        extractor.keep_fresh = Duration::from_millis(50);
        let extractor = Arc::new(extractor);
        let (name, key) = movie_in(&extractor, dir.path(), &unplaced_movie());
        let part = dir.path().join("subtitles").join(format!("{key}_4.vtt.part"));
        std::fs::write(&part, "WEBVTT\n").unwrap();
        let file = std::fs::File::options().write(true).open(&part).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(4 * 3600)).unwrap();
        drop(file);
        // A track with no cue for hours, its pass still running, is marked as
        // being written...
        let pass = extractor.extraction(Path::new(&name), &key, 2);
        eventually(|| {
            let modified = std::fs::metadata(&part).and_then(|m| m.modified());
            modified.is_ok_and(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(3600))
        })
        .await;
        // ...so the cache's pruning, which takes old temporaries for abandoned
        // ones, keeps it.
        let policy = crate::cache::Policy { max_age: Duration::from_secs(3600), max_bytes: u64::MAX };
        crate::cache::prune(&dir.path().join("subtitles"), &policy, SystemTime::now() + Duration::from_secs(60));
        assert!(part.exists());
        drop(pass);
    }

    /// FFmpeg and ffprobe from PATH, and the media facts of `path`.
    async fn tools(path: &Path) -> (Arc<Ffmpeg>, Arc<Prober>, Arc<Metrics>, Arc<MediaInfo>) {
        let caps =
            pley_transcoder_api::Capabilities { ffmpeg_available: true, ffprobe_available: true, ..Default::default() };
        let ff =
            Arc::new(Ffmpeg { ffmpeg: "ffmpeg".into(), ffprobe: "ffprobe".into(), caps, segment_times_relative: true });
        let metrics = Arc::new(Metrics::default());
        let prober = Arc::new(Prober::new(ff.clone(), metrics.clone(), 1));
        let media = prober.probe(path).await.expect("ffprobe reads it");
        (ff, prober, metrics, media)
    }

    /// Room to read every track of a real file, however many it has.
    fn no_limit() -> matroska::Budget {
        matroska::Budget::new(u64::MAX, Duration::from_secs(3600))
    }

    /// Every text track of a local Matroska file, read through its Cues,
    /// is what FFmpeg extracts, byte for byte:
    /// `PLEY_TEST_MKV=<file> cargo test -p pley-transcoder cues_match_ffmpeg -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn cues_match_ffmpeg() {
        let path = PathBuf::from(std::env::var("PLEY_TEST_MKV").expect("set PLEY_TEST_MKV"));
        let dir = tempfile::tempdir().unwrap();
        let (ff, prober, metrics, media) = tools(&path).await;
        let streams: Vec<i64> = media.streams.iter().filter(|s| s.is_text_subtitle()).map(|s| s.index).collect();
        let started = Instant::now();
        let ours = cue_tracks(&path, &media, &streams, &no_limit()).expect("the cues list the subtitles");
        println!("{} tracks through the cues in {:?}", ours.len(), started.elapsed());
        let extractor = Extractor::new(ff, prober, metrics, dir.path().join("subtitles"), 1);
        let started = Instant::now();
        extractor.extract_vtt(&path, "ffmpeg", &streams).await.expect("ffmpeg extracts them");
        println!("{} tracks by ffmpeg in {:?}", streams.len(), started.elapsed());
        for (index, text) in &ours {
            let theirs =
                std::fs::read_to_string(dir.path().join("subtitles").join(format!("ffmpeg_{index}.vtt"))).unwrap();
            assert_eq!(text, &theirs, "stream {index}");
            println!("stream {index}: {} cues, identical", text.matches(" --> ").count());
        }
    }

    /// How long reading a file's text tracks through its Cues takes, and how
    /// much it reads, with no FFmpeg pass: the first track, then all of them
    /// (or each of `PLEY_TEST_STREAMS=3,5`):
    /// `PLEY_TEST_MKV=<file> cargo test -p pley-transcoder cues_reading_time -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn cues_reading_time() {
        let path = PathBuf::from(std::env::var("PLEY_TEST_MKV").expect("set PLEY_TEST_MKV"));
        let (_, _, _, media) = tools(&path).await;
        let streams: Vec<i64> = media.streams.iter().filter(|s| s.is_text_subtitle()).map(|s| s.index).collect();
        let only = std::env::var("PLEY_TEST_STREAMS").ok();
        let runs: Vec<Vec<i64>> = match &only {
            Some(list) => list.split(',').map(|s| vec![s.trim().parse().expect("stream indexes")]).collect(),
            None => vec![streams[..1].to_vec(), streams.clone()],
        };
        for wanted in runs {
            let started = Instant::now();
            let budget = no_limit();
            let found = matroska::text_tracks(&path, &media, &wanted, &budget).expect("the cues list the subtitles");
            let took = started.elapsed();
            let (reads, kb) = (found.reads, found.bytes_read / 1024);
            let blocks: usize = found.tracks.iter().map(|t| t.blocks.len()).sum();
            let vtt = as_webvtt(found, &media, &budget).unwrap();
            println!(
                "stream(s) {:?}: {blocks} subtitles in {took:?} ({reads} reads, {kb} KB), {} KB of WebVTT",
                if wanted.len() > 3 { format!("{} tracks", wanted.len()) } else { format!("{wanted:?}") },
                vtt.iter().map(|(_, t)| t.len()).sum::<usize>() / 1024
            );
        }
    }

    /// Stream 2, a SubRip track, as read through the Cues: blocks of
    /// `(time, duration, text)` in ticks of `tick_ns`.
    fn read(tick_ns: u64, blocks: &[(i64, u64, &str)]) -> matroska::TextTracks {
        let blocks = blocks
            .iter()
            .map(|&(time, duration, text)| matroska::TextBlock { time, duration, data: text.as_bytes().to_vec() })
            .collect();
        let track =
            matroska::TextTrack { stream_index: 2, codec: webvtt::TextCodec::Subrip, header: Vec::new(), blocks };
        let whole_ms = tick_ns.is_multiple_of(1_000_000);
        matroska::TextTracks { tick_ns, whole_ms, tracks: vec![track], reads: 0, bytes_read: 0 }
    }

    /// Cue times are FFmpeg's: the file's start time taken off, block times
    /// rounded to microseconds and then to milliseconds, half away from zero.
    #[test]
    fn cue_times_are_ffmpegs() {
        let vtt = |found, start_ms| {
            let media = MediaInfo { start_ms, ..Default::default() };
            as_webvtt(found, &media, &no_limit()).map(|mut tracks| tracks.remove(0).1)
        };
        // Ticks of a millisecond, a file starting at 5.5 s.
        let found = read(1_000_000, &[(5_500, 1_000, "first"), (6_000, 1_500, "second")]);
        let first = "WEBVTT\n\n00:00.000 --> 00:01.000\nfirst\n\n00:00.500 --> 00:02.000\nsecond\n";
        assert_eq!(vtt(found, Some(5_500)).unwrap(), first);
        // Ticks of 100 µs: 0.5 ms is 1 ms, and so is 1.4 ms.
        let found = read(100_000, &[(5, 10, "half"), (14, 5, "rounded")]);
        let rounded = "WEBVTT\n\n00:00.001 --> 00:00.002\nhalf\n\n00:00.001 --> 00:00.002\nrounded\n";
        assert_eq!(vtt(found, None).unwrap(), rounded);
        // Ticks of 333333 ns: 3 of them are 999.999 µs, so 1000, so 1 ms...
        let found = read(333_333, &[(3, 3, "third")]);
        assert_eq!(vtt(found, None).unwrap(), "WEBVTT\n\n00:00.001 --> 00:00.002\nthird\n");
        // ...and 500003 of them, 166667.499999 ms, are 166667500 µs, so
        // 166668 ms (not 166667, as rounded at once).
        let found = read(333_333, &[(500_003, 3, "twice")]);
        assert_eq!(vtt(found, None).unwrap(), "WEBVTT\n\n02:46.668 --> 02:46.669\ntwice\n");
        // With such ticks, a start the media info has in milliseconds
        // (1.833332 s as 1833) may not be FFmpeg's: left to it. So is the
        // start of 0 that ffprobe gives most files with audio or video,
        // which may be a fraction of a millisecond.
        let found = read(333_333, &[(5_500, 3, "a")]);
        assert_eq!(vtt(found, Some(1_833)).unwrap_err(), "the file's start isn't known to the microsecond");
        let found = read(333_333, &[(3, 3, "third")]);
        assert_eq!(vtt(found, Some(0)).unwrap_err(), "the file's start isn't known to the microsecond");
        // A cue before the start, or going back in time: FFmpeg's muxer
        // would shift or refuse them, so they are left to it too.
        assert!(vtt(read(1_000_000, &[(5_000, 100, "early")]), Some(5_500)).is_err());
        assert!(vtt(read(1_000_000, &[(2_000, 100, "b"), (1_000, 100, "a")]), None).is_err());
        assert!(vtt(read(1_000_000, &[(1_000, 100, "a"), (1_000, 100, "b")]), None).is_ok(), "at once");
    }

    #[test]
    fn cues_too_long_or_too_late_are_left_to_ffmpeg() {
        let media = MediaInfo::default();
        let vtt = |found| as_webvtt(found, &media, &no_limit()).map(|mut tracks| tracks.remove(0).1);
        // FFmpeg keeps how long a cue lasts in 32 bits of milliseconds.
        let longest = vtt(read(1_000_000, &[(0, 0xFFFF_FFFF, "a")])).unwrap();
        assert_eq!(longest, "WEBVTT\n\n00:00.000 --> 1193:02:47.295\na\n");
        assert!(vtt(read(1_000_000, &[(0, 0x1_0000_0000, "a")])).is_err());
        assert!(vtt(read(1_000_000, &[(0, u64::MAX, "a")])).is_err());
        assert!(vtt(read(1_000_000, &[(i64::MAX, 1, "a")])).is_err(), "too far to tell in microseconds");
        assert!(vtt(read(u64::MAX, &[(1, 1, "a")])).is_err(), "a timestamp scale out of range");
    }

    #[test]
    fn rescaling_rounds_like_ffmpeg() {
        let (ms, us, ns) = ((1, 1000), (1, 1_000_000), (1, 1_000_000_000));
        assert_eq!(rescale(1_500, ms, us), Some(1_500_000));
        assert_eq!(rescale(1_500, us, ms), Some(2), "half away from zero");
        assert_eq!(rescale(-1_500, us, ms), Some(-2));
        assert_eq!(rescale(1_499, us, ms), Some(1));
        assert_eq!(rescale(3, (100_000, 1_000_000_000), ms), Some(0), "0.3 ms");
        assert_eq!(rescale(6_500_000, ns, ms), Some(7));
        assert_eq!(rescale(i64::MAX / 2, ms, us), None, "too far to tell");
    }

    #[test]
    fn whole_file_passes_get_time_for_big_files() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("small.mkv");
        std::fs::write(&small, b"tiny").unwrap();
        assert_eq!(whole_file_timeout(&small), Duration::from_secs(20 * 60));
        assert_eq!(whole_file_timeout(&dir.path().join("missing.mkv")), Duration::from_secs(20 * 60));
        assert_eq!(pass_timeout(5_000_000_000), Duration::from_secs(20 * 60));
        assert_eq!(pass_timeout(77_098_924_052), Duration::from_secs(7_709), "a 77 GB 4K remux: over 2 hours");
    }

    #[test]
    fn charset_guess() {
        assert_eq!(guess_charset("héllo".as_bytes()), None);
        assert_eq!(guess_charset(&[b'h', 0xE9, b'l']), Some("CP1252"));
        assert_eq!(guess_charset(&[0xFF, 0xFE, b'h', 0]), None);
        // A first megabyte that cuts "é" in two.
        assert_eq!(guess_charset(&"hé".as_bytes()[..2]), None);
    }

    #[tokio::test]
    async fn subtitle_files_too_big_to_be_text_are_refused_unread() {
        let dir = tempfile::tempdir().unwrap();
        let extractor = extractor_without_ffmpeg(dir.path());
        let sup = dir.path().join("movie.en.sup");
        let name = sup.to_string_lossy().into_owned();
        std::fs::File::create(&sup).unwrap().set_len(MAX_SUBTITLE_FILE + 1).unwrap();
        // Refused as it is, neither read nor given to FFmpeg (not there).
        match extractor.subtitle(&name, None).await {
            Err(ApiError::BadRequest(message)) => assert!(message.contains("64 MB at most"), "{message}"),
            other => panic!("{other:?}"),
        }
        // One just small enough goes to FFmpeg, its charset told from its start.
        std::fs::File::options().write(true).open(&sup).unwrap().set_len(MAX_SUBTITLE_FILE).unwrap();
        assert!(matches!(extractor.subtitle(&name, None).await, Err(ApiError::Internal(_))));
        assert_eq!(read_head(&sup, CHARSET_SAMPLE).await.unwrap().len(), CHARSET_SAMPLE);
    }
}
