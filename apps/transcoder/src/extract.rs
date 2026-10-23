// SPDX-License-Identifier: LicenseRef-Restricted
//! One-shot extractions: video frame thumbnails, embedded cover art, and
//! subtitles converted to WebVTT (all text tracks of a file in one pass, since
//! extracting any track means reading the whole file).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use pley_transcoder_api::{MediaInfo, MediaStream, ThumbnailRequest, Tonemapper};
use tokio::sync::Semaphore;

use crate::error::{ApiError, ApiResult};
use crate::ffmpeg::Ffmpeg;
use crate::metrics::Metrics;
use crate::plan::{MAX_DURATION_MS, TextSource, ZSCALE_TONEMAP, untagged_hdr_fix};
use crate::probe::Prober;
use crate::util::{MEDIA_INPUT_OPTIONS, file_key, input_arg, secs};

pub struct Extractor {
    ff: Arc<Ffmpeg>,
    prober: Arc<Prober>,
    metrics: Arc<Metrics>,
    subs_dir: PathBuf,
    limiter: Semaphore,
    file_locks: FileLocks,
}

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
            file_locks: Mutex::default(),
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
    pub async fn subtitle(&self, path: &str, stream: Option<i64>) -> ApiResult<String> {
        let path = existing(path)?;
        Metrics::inc(&self.metrics.subtitle_extractions);
        match stream {
            None => self.convert_file(&path).await,
            Some(index) => self.embedded(&path, index).await,
        }
    }

    async fn convert_file(&self, path: &Path) -> ApiResult<String> {
        let raw = tokio::fs::read(path).await?;
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if ext == "vtt" {
            if let Ok(text) = String::from_utf8(raw.clone()) {
                let text = text.trim_start_matches('\u{feff}').to_owned();
                return Ok(if text.starts_with("WEBVTT") { text } else { format!("WEBVTT\n\n{text}") });
            }
        }
        let key = file_key(path)?;
        let cached = self.subs_dir.join(format!("{key}_file.vtt"));
        if let Ok(text) = tokio::fs::read_to_string(&cached).await {
            return Ok(text);
        }
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error"]);
        if let Some(charset) = guess_charset(&raw) {
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
        let _ = tokio::fs::write(&cached, &text).await;
        Ok(text)
    }

    async fn embedded(&self, path: &Path, index: i64) -> ApiResult<String> {
        let key = file_key(path)?;
        let target = self.subs_dir.join(format!("{key}_{index}.vtt"));
        if let Ok(text) = tokio::fs::read_to_string(&target).await {
            return Ok(text);
        }
        let entry = FileLock::acquire(&self.file_locks, &key);
        let _guard = entry.lock.lock().await;
        if let Ok(text) = tokio::fs::read_to_string(&target).await {
            return Ok(text);
        }

        let media = self.prober.probe(path).await.map_err(|e| ApiError::Failed(format!("probe failed: {e:#}")))?;
        let text_streams: Vec<i64> = media.streams.iter().filter(|s| s.is_text_subtitle()).map(|s| s.index).collect();
        if !text_streams.contains(&index) {
            return Err(ApiError::BadRequest(format!("stream {index} is not a text subtitle track")));
        }
        // Extract every text track in a single pass over the file.
        let _permit = self.limiter.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        let all = self.extract_vtt(path, &key, &text_streams).await;
        if all.is_err() && text_streams.len() > 1 {
            self.extract_vtt(path, &key, &[index]).await?;
        } else {
            all?;
        }
        Ok(tokio::fs::read_to_string(&target).await?)
    }

    async fn extract_vtt(&self, path: &Path, key: &str, streams: &[i64]) -> ApiResult<()> {
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error", "-y"]).args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(path));
        let mut outputs = Vec::new();
        for index in streams {
            let part = self.subs_dir.join(format!("{key}_{index}.vtt.part"));
            cmd.args(["-map", &format!("0:{index}"), "-c:s", "webvtt", "-f", "webvtt"]).arg(&part);
            outputs.push((part, self.subs_dir.join(format!("{key}_{index}.vtt"))));
        }
        let result = run_capture(cmd, Duration::from_secs(20 * 60)).await;
        for (part, done) in &outputs {
            if result.is_ok() {
                let _ = tokio::fs::rename(part, done).await;
            } else {
                let _ = tokio::fs::remove_file(part).await;
            }
        }
        result.map(|_| ())
    }

    /// Write a subtitle track as ASS into a job directory for libass burn-in.
    pub async fn extract_ass(&self, input: &Path, source: &TextSource, out: &Path) -> ApiResult<()> {
        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error", "-y"]);
        match source {
            TextSource::Embedded(index) => {
                cmd.args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(input)).args(["-map", &format!("0:{index}")]);
            }
            TextSource::File(path) => {
                if let Ok(raw) = tokio::fs::read(path).await {
                    if let Some(charset) = guess_charset(&raw) {
                        cmd.args(["-sub_charenc", charset]);
                    }
                }
                cmd.args(MEDIA_INPUT_OPTIONS).arg("-i").arg(input_arg(path)).args(["-map", "0:s:0"]);
            }
        }
        cmd.args(["-c:s", "ass", "-f", "ass"]).arg(out);
        run_capture(cmd, Duration::from_secs(20 * 60)).await.map(|_| ())
    }
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

fn existing(path: &str) -> ApiResult<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_file() { Ok(path) } else { Err(ApiError::NotFound(format!("file not found: {}", path.display()))) }
}

/// Run ffmpeg, returning stdout. Non-zero exit becomes `ApiError::Failed`.
pub(crate) async fn run_capture(mut cmd: tokio::process::Command, timeout: Duration) -> ApiResult<(Vec<u8>, String)> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn().map_err(|e| ApiError::Internal(anyhow::anyhow!("failed to start ffmpeg: {e}")))?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| ApiError::Timeout("ffmpeg took too long".into()))?
        .map_err(|e| ApiError::Internal(e.into()))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        let message = stderr.lines().last().unwrap_or("ffmpeg failed").to_owned();
        return Err(ApiError::Failed(message));
    }
    Ok((output.stdout, stderr))
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

/// Text subtitle files are frequently not UTF-8. UTF-16 with a BOM is handled by
/// ffmpeg; anything else that is not valid UTF-8 is assumed to be Windows-1252.
fn guess_charset(raw: &[u8]) -> Option<&'static str> {
    if raw.starts_with(&[0xFF, 0xFE]) || raw.starts_with(&[0xFE, 0xFF]) || std::str::from_utf8(raw).is_ok() {
        None
    } else {
        Some("CP1252")
    }
}

type FileLocks = Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

/// Per-file extraction lock, forgotten again once no request uses it (on
/// every path, including errors and cancelled requests).
struct FileLock<'a> {
    locks: &'a FileLocks,
    key: String,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl<'a> FileLock<'a> {
    fn acquire(locks: &'a FileLocks, key: &str) -> Self {
        let lock = locks.lock().entry(key.to_owned()).or_default().clone();
        Self { locks, key: key.to_owned(), lock }
    }
}

impl Drop for FileLock<'_> {
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

    #[test]
    fn charset_guess() {
        assert_eq!(guess_charset("héllo".as_bytes()), None);
        assert_eq!(guess_charset(&[b'h', 0xE9, b'l']), Some("CP1252"));
        assert_eq!(guess_charset(&[0xFF, 0xFE, b'h', 0]), None);
    }
}
