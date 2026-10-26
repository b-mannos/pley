// SPDX-License-Identifier: LicenseRef-Restricted
//! Conversions: a [`JobRequest`] made into one Matroska file, in one FFmpeg
//! run at full speed (a converted download, for a device that doesn't play
//! the original as it is). One runs at a time, in a transcode slot like a
//! job's process (hardware when a session is free), so playback keeps the
//! others. Asked again for the same input and request, the same conversion
//! answers, or the file it made. The files live in `conversions/` under the
//! work folder, which the janitor prunes ([`crate::cache`]); reading one
//! counts as using it.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use parking_lot::Mutex;
use pley_transcoder_api::{ConversionState, ConversionStatus, JobRequest, MediaInfo, VideoMode};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Semaphore, watch};

use crate::adaptive::AUTO;
use crate::error::{ApiError, ApiResult};
use crate::ffmpeg::Ffmpeg;
use crate::job::Slots;
use crate::plan::{ArgsContext, MAX_DURATION_MS, Plan};
use crate::probe::Prober;
use crate::util::touch;

/// How often a conversion waiting for a transcode slot looks again.
const SLOT_WAIT: Duration = Duration::from_secs(5);
/// FFmpeg's last lines kept, for a failure's message.
const TAIL_LINES: usize = 6;
/// The software encoder's preset when the plan leaves it to the transcoder:
/// a file isn't made in real time, so it can take a little more care.
const SOFTWARE_PRESET: &str = "fast";

pub struct Conversions {
    ff: Arc<Ffmpeg>,
    prober: Arc<Prober>,
    slots: Arc<Slots>,
    shader_cache: Option<PathBuf>,
    dir: PathBuf,
    items: Mutex<HashMap<String, Arc<Conversion>>>,
    /// One conversion at a time.
    turn: Arc<Semaphore>,
}

struct Conversion {
    status: Mutex<ConversionStatus>,
    /// Set when the conversion is removed (or the transcoder stops): FFmpeg goes.
    stop: watch::Sender<bool>,
}

impl Conversion {
    fn new(status: ConversionStatus) -> Arc<Self> {
        Arc::new(Self { status: Mutex::new(status), stop: watch::channel(false).0 })
    }

    fn update(&self, change: impl FnOnce(&mut ConversionStatus)) {
        change(&mut self.status.lock());
    }

    /// Resolves once the conversion is to stop.
    async fn stopped(&self) {
        let mut stop = self.stop.subscribe();
        let _ = stop.wait_for(|stop| *stop).await;
    }
}

/// Gives a transcode slot back however the conversion ends.
struct Slot {
    slots: Arc<Slots>,
    hw: bool,
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.slots.release(self.hw);
    }
}

impl Conversions {
    pub fn new(
        ff: Arc<Ffmpeg>,
        prober: Arc<Prober>,
        slots: Arc<Slots>,
        shader_cache: Option<PathBuf>,
        dir: PathBuf,
    ) -> Arc<Self> {
        // What a crash left half made is of no use: it starts again when asked.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|e| e == "part") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Arc::new(Self {
            ff,
            prober,
            slots,
            shader_cache,
            dir,
            items: Mutex::new(HashMap::new()),
            turn: Arc::new(Semaphore::new(1)),
        })
    }

    /// Starts converting `req`, or finds the conversion (or the file) made
    /// for it already. A failed one is tried again.
    pub async fn create(self: &Arc<Self>, req: JobRequest) -> ApiResult<ConversionStatus> {
        if !self.ff.caps.ffmpeg_available {
            return Err(ApiError::Failed("ffmpeg is not available on the transcoder host".into()));
        }
        let meta = match tokio::fs::metadata(&req.input).await {
            Ok(meta) if meta.is_file() => meta,
            Ok(_) => return Err(ApiError::BadRequest(format!("{} is not a file", req.input))),
            Err(_) => return Err(ApiError::NotFound(format!("input not found: {}", req.input))),
        };
        let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
        let id = conversion_id(&req, meta.len(), modified.map_or(0, |d| d.as_millis()));
        let duration_ms = req.media.as_ref().and_then(|m| m.duration_ms).unwrap_or(0);
        let conversion = {
            let mut items = self.items.lock();
            if let Some(found) = items.get(&id).filter(|c| c.status.lock().state != ConversionState::Failed) {
                return Ok(found.status.lock().clone());
            }
            if let Some(status) = self.finished(&id, duration_ms) {
                items.insert(id.clone(), Conversion::new(status.clone()));
                return Ok(status);
            }
            let conversion = Conversion::new(ConversionStatus { id: id.clone(), duration_ms, ..Default::default() });
            items.insert(id.clone(), conversion.clone());
            conversion
        };
        tracing::info!(conversion = %id, input = %req.input, "conversion queued");
        let status = conversion.status.lock().clone();
        let this = self.clone();
        tokio::spawn(async move {
            let result = this.convert(&conversion, req).await;
            conversion.update(|s| match result {
                Ok(size) => {
                    tracing::info!(conversion = %s.id, size, "conversion done");
                    s.state = ConversionState::Done;
                    s.progress = 1.0;
                    s.size = Some(size);
                }
                Err(err) => {
                    tracing::warn!(conversion = %s.id, "conversion failed: {err:#}");
                    s.state = ConversionState::Failed;
                    s.error = Some(format!("{err:#}"));
                }
            });
        });
        Ok(status)
    }

    /// Conversion `id` as it stands; one made before a restart is known by its file.
    pub fn status(&self, id: &str) -> Option<ConversionStatus> {
        if let Some(found) = self.items.lock().get(id) {
            return Some(found.status.lock().clone());
        }
        self.finished(id, 0)
    }

    /// The file conversion `id` made, once whole.
    pub fn file(&self, id: &str) -> Option<PathBuf> {
        let path = self.path(id)?;
        if !path.is_file() {
            return None;
        }
        touch(&path);
        Some(path)
    }

    /// Stops conversion `id` and deletes what it made: whether there was one.
    pub async fn remove(&self, id: &str) -> bool {
        let Some(path) = self.path(id) else { return false };
        let known = self.items.lock().remove(id);
        if let Some(conversion) = &known {
            conversion.stop.send_replace(true);
        }
        let had_file = tokio::fs::remove_file(&path).await.is_ok();
        // FFmpeg may still hold the half-made file a moment.
        let part = part_of(&path);
        for _ in 0..20 {
            if tokio::fs::remove_file(&part).await.is_ok() || !part.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        known.is_some() || had_file
    }

    /// Stops every conversion: the transcoder is going.
    pub fn shutdown(&self) {
        for conversion in self.items.lock().values() {
            conversion.stop.send_replace(true);
        }
    }

    /// Where conversion `id`'s file goes; `None` for an id that isn't one.
    fn path(&self, id: &str) -> Option<PathBuf> {
        (id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())).then(|| self.dir.join(format!("{id}.mkv")))
    }

    /// The status of a conversion whose file is there already.
    fn finished(&self, id: &str, duration_ms: i64) -> Option<ConversionStatus> {
        let size = std::fs::metadata(self.path(id)?).ok().filter(|m| m.is_file())?.len();
        Some(ConversionStatus {
            id: id.to_owned(),
            state: ConversionState::Done,
            progress: 1.0,
            duration_ms,
            size: Some(size),
            ..Default::default()
        })
    }

    /// Makes the file: its turn, then a slot, then FFmpeg. The file's size.
    async fn convert(&self, conversion: &Arc<Conversion>, mut req: JobRequest) -> anyhow::Result<u64> {
        let _turn = tokio::select! {
            turn = self.turn.clone().acquire_owned() => turn?,
            () = conversion.stopped() => bail!("removed"),
        };
        let id = conversion.status.lock().id.clone();
        let input = PathBuf::from(&req.input);
        let media: Arc<MediaInfo> = match req
            .media
            .clone()
            .filter(|m| m.duration_ms.is_some_and(|d| d > 0 && d <= MAX_DURATION_MS) && !m.streams.is_empty())
            .filter(|m| m.start_ms.is_some())
        {
            Some(media) => Arc::new(media),
            None => self.prober.probe(&input).await.context("probe failed")?,
        };
        // Made at full speed, from the start.
        req.options.throttle = false;
        req.start_ms = None;
        // One file is one segment: a copy needs no keyframes but the first.
        let first = [media.start_ms.unwrap_or(0)];
        let mut plan = Plan::build(&req, &media, &self.ff, Some(&first)).map_err(|err| anyhow!(err))?;
        if plan.segment_edits().is_some() {
            // A remux's edits (Dolby Vision, HDR10+) are made to segments as
            // they are written: a file can't have them, so its video is made anew.
            if let Some(video) = req.video.as_mut() {
                video.mode = VideoMode::Transcode;
            }
            plan = Plan::build(&req, &media, &self.ff, Some(&first)).map_err(|err| anyhow!(err))?;
        }
        let duration_ms = media.duration_ms.unwrap_or(0).max(1);
        conversion.update(|s| {
            s.state = ConversionState::Running;
            s.duration_ms = duration_ms;
            s.video = plan.video_plan(&media, None);
            s.audio = plan.audio_plan(&media);
        });

        // A transcode slot, as a job's process takes: waited for while every one is in use.
        let planned = plan.video.as_ref().filter(|v| !v.copy).and_then(|v| v.encoder.as_ref());
        let want_hw = planned.is_some_and(|e| e.hardware);
        let hw = loop {
            if let Some(hw) = self.slots.acquire(want_hw) {
                break hw;
            }
            tokio::select! {
                () = tokio::time::sleep(SLOT_WAIT) => {}
                () = conversion.stopped() => bail!("removed"),
            }
        };
        let _slot = Slot { slots: self.slots.clone(), hw };
        let fallback = if want_hw && !hw {
            let main10 = plan.video.as_ref().is_some_and(|v| v.hdr.is_some());
            let software = planned.and_then(|e| self.ff.software_encoder_for(e.codec, main10));
            Some(software.ok_or_else(|| anyhow!("no encoder is free: every hardware session is in use"))?)
        } else {
            None
        };
        let preset = plan
            .video
            .as_ref()
            .filter(|v| !v.copy)
            .map(|v| if v.preset == AUTO { SOFTWARE_PRESET } else { v.preset.as_str() });

        tokio::fs::create_dir_all(&self.dir).await?;
        let path = self.dir.join(format!("{id}.mkv"));
        let part = part_of(&path);
        let ctx = ArgsContext {
            input: &input,
            encoder: fallback,
            relative_split_times: self.ff.segment_times_relative,
            preset,
            shader_cache: self.shader_cache.as_deref(),
        };
        let args = plan.file_args(&ctx, &part);
        tracing::debug!(conversion = %id, "ffmpeg {}", args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "));
        let mut cmd = self.ff.command();
        cmd.args(&args).current_dir(&self.dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        let mut child = cmd.spawn().context("failed to start ffmpeg")?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("ffmpeg's error output is unavailable"))?;
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let reader = tokio::spawn(read_progress(stderr, duration_ms, tail.clone(), conversion.clone()));
        let (status, removed) = tokio::select! {
            status = child.wait() => (status?, false),
            () = conversion.stopped() => {
                let _ = child.start_kill();
                (child.wait().await?, true)
            }
        };
        let _ = reader.await;
        if removed {
            let _ = tokio::fs::remove_file(&part).await;
            bail!("removed");
        }
        if !status.success() {
            let _ = tokio::fs::remove_file(&part).await;
            let lines: Vec<String> = tail.lock().iter().cloned().collect();
            if lines.is_empty() {
                bail!("ffmpeg exited with {status}");
            }
            bail!("ffmpeg exited with {status}: {}", lines.join(" / "));
        }
        tokio::fs::rename(&part, &path).await.context("couldn't keep the file")?;
        Ok(tokio::fs::metadata(&path).await?.len())
    }
}

/// The file being made for `path`.
fn part_of(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    PathBuf::from(part)
}

/// What names a conversion: the input as it is now (path, size and time)
/// and what the request makes of it, not who asked or where a playback
/// would have started.
fn conversion_id(req: &JobRequest, size: u64, modified_ms: u128) -> String {
    let what = serde_json::json!({
        "input": req.input,
        "size": size,
        "modified": modified_ms.to_string(),
        "video": req.video,
        "audio": req.audio,
        "subtitle": req.subtitle,
    });
    let digest = Sha256::digest(what.to_string().as_bytes());
    hex::encode(&digest[..16])
}

/// Reads FFmpeg's `-progress` lines into how far `conversion` has come,
/// and its other lines (errors) into `tail`.
async fn read_progress(
    stderr: tokio::process::ChildStderr,
    duration_ms: i64,
    tail: Arc<Mutex<VecDeque<String>>>,
    conversion: Arc<Conversion>,
) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == "out_time_us" {
                if let Ok(us) = value.trim().parse::<f64>() {
                    let progress = progress_of(us, duration_ms);
                    conversion.update(|s| s.progress = s.progress.max(progress));
                }
                continue;
            }
            // The other `-progress` keys: one word each.
            if !key.contains(' ') {
                continue;
            }
        }
        let line = line.trim();
        if !line.is_empty() {
            let mut tail = tail.lock();
            if tail.len() == TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(line.to_owned());
        }
    }
}

/// `out_time_us` as a share of the media, 0 to 1.
fn progress_of(out_time_us: f64, duration_ms: i64) -> f64 {
    if duration_ms <= 0 || out_time_us <= 0.0 {
        return 0.0;
    }
    (out_time_us / 1000.0 / duration_ms as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pley_transcoder_api::{AudioRequest, VideoRequest};

    fn request(max_height: u32) -> JobRequest {
        JobRequest {
            input: "/media/movie.mkv".into(),
            label: Some("Movie".into()),
            owner: Some("anna".into()),
            video: Some(VideoRequest { max_height: Some(max_height), ..Default::default() }),
            audio: Some(AudioRequest::default()),
            start_ms: Some(60_000),
            ..Default::default()
        }
    }

    #[test]
    fn a_conversion_is_named_by_its_input_and_what_it_makes() {
        let id = conversion_id(&request(1080), 100, 5);
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
        // Who asked and where a playback would start make no difference.
        let mut other = request(1080);
        other.owner = Some("ben".into());
        other.label = None;
        other.start_ms = None;
        assert_eq!(conversion_id(&other, 100, 5), id);
        // Another size, another file, another time or another target do.
        assert_ne!(conversion_id(&request(720), 100, 5), id);
        assert_ne!(conversion_id(&request(1080), 101, 5), id);
        assert_ne!(conversion_id(&request(1080), 100, 6), id);
    }

    fn has_ffmpeg() -> bool {
        let ok =
            |tool: &str| std::process::Command::new(tool).arg("-version").output().is_ok_and(|o| o.status.success());
        let found = ok("ffmpeg") && ok("ffprobe");
        if !found {
            eprintln!("ffmpeg not found: skipped");
        }
        found
    }

    /// A conversion made by FFmpeg (skipped without it on the PATH): a 4 s
    /// clip of H.264 and E-AC-3 5.1 (sound many phones don't decode), its
    /// video copied and its sound made AAC stereo, into Matroska.
    #[tokio::test]
    async fn a_conversion_makes_a_file_of_the_streams_asked_for() {
        use pley_transcoder_api::{
            AudioCodec, AudioMode, AudioRequest, Capabilities, EncoderInfo, HwAccel, Limits, VideoCodec, VideoRequest,
        };
        if !has_ffmpeg() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.mkv");
        let made = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "lavfi", "-i", "testsrc2=duration=4:size=320x180:rate=24"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=4"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "eac3", "-ac", "6"])
            .arg(&source)
            .status()
            .unwrap();
        assert!(made.success());

        let ff = Arc::new(Ffmpeg {
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            caps: Capabilities {
                ffmpeg_available: true,
                ffprobe_available: true,
                encoders: vec![EncoderInfo {
                    name: "libx264".into(),
                    codec: VideoCodec::H264,
                    hardware: false,
                    accel: HwAccel::None,
                    main10: false,
                }],
                ..Default::default()
            },
            segment_times_relative: true,
        });
        let metrics = Arc::new(crate::metrics::Metrics::default());
        let prober = Arc::new(Prober::new(ff.clone(), metrics, 1));
        let slots = Arc::new(Slots::new(&Limits { max_jobs: 2, max_hw_jobs: 0 }));
        let conversions = Conversions::new(ff, prober, slots.clone(), None, dir.path().join("conversions"));
        let req = JobRequest {
            input: source.to_string_lossy().into_owned(),
            video: Some(VideoRequest { mode: VideoMode::Copy, ..Default::default() }),
            audio: Some(AudioRequest {
                mode: AudioMode::Transcode,
                codec: AudioCodec::Aac,
                channels: Some(2),
                ..Default::default()
            }),
            ..Default::default()
        };

        let created = conversions.create(req.clone()).await.unwrap();
        let id = created.id.clone();
        let status = loop {
            let status = conversions.status(&id).unwrap();
            if matches!(status.state, ConversionState::Done | ConversionState::Failed) {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert_eq!(status.state, ConversionState::Done, "{:?}", status.error);
        assert_eq!(status.progress, 1.0);
        assert_eq!(status.video.as_ref().map(|v| v.mode), Some(VideoMode::Copy));
        assert_eq!(slots.counts(), (0, 0), "its slot is given back");

        let file = conversions.file(&id).unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().len(), status.size.unwrap());
        let out = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-show_entries", "stream=codec_name,channels", "-of", "csv=p=0"])
            .arg(&file)
            .output()
            .unwrap();
        let streams = String::from_utf8_lossy(&out.stdout).replace("\r", "");
        assert_eq!(streams.trim(), "h264\naac,2", "{streams}");

        // Asked again: the file made.
        let again = conversions.create(req.clone()).await.unwrap();
        assert_eq!((again.id, again.state), (id.clone(), ConversionState::Done));
        assert!(conversions.remove(&id).await);
        assert!(conversions.file(&id).is_none());

        // Its video made anew, smaller: another conversion of the same file.
        let smaller = JobRequest {
            video: Some(VideoRequest { mode: VideoMode::Transcode, max_height: Some(90), ..Default::default() }),
            ..req
        };
        let created = conversions.create(smaller).await.unwrap();
        assert_ne!(created.id, id);
        let status = loop {
            let status = conversions.status(&created.id).unwrap();
            if matches!(status.state, ConversionState::Done | ConversionState::Failed) {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert_eq!(status.state, ConversionState::Done, "{:?}", status.error);
        let out = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v",
                "-show_entries",
                "stream=codec_name,height",
                "-of",
                "csv=p=0",
            ])
            .arg(conversions.file(&created.id).unwrap())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "h264,90");
    }

    #[test]
    fn progress_is_a_share_of_the_media() {
        assert_eq!(progress_of(30_000_000.0, 60_000), 0.5);
        assert_eq!(progress_of(90_000_000.0, 60_000), 1.0);
        assert_eq!(progress_of(-1.0, 60_000), 0.0);
        assert_eq!(progress_of(1.0, 0), 0.0);
    }
}
