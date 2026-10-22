// SPDX-License-Identifier: LicenseRef-Restricted
//! Audio fingerprints of parts of a file, computed by FFmpeg's `chromaprint`
//! muxer. The server compares them across the episodes of a season to find
//! the intro and credits they share. Background work: few at a time, at low
//! process priority, so playback never waits for it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use pley_transcoder_api::{Fingerprint, FingerprintRequest};
use tokio::sync::Semaphore;

use crate::error::{ApiError, ApiResult};
use crate::extract::run_capture;
use crate::ffmpeg::Ffmpeg;
use crate::util::{background_priority, input_arg, secs};

/// Chromaprint's default algorithm works on 11025 Hz audio in frames that
/// advance by 1365 samples: one value per 0.1238 s.
pub const ITEM_SECONDS: f64 = 1365.0 / 11025.0;
const MAX_DURATION_MS: i64 = 30 * 60_000;
const TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub struct Fingerprinter {
    ff: Arc<Ffmpeg>,
    limiter: Semaphore,
}

impl Fingerprinter {
    pub fn new(ff: Arc<Ffmpeg>, concurrency: usize) -> Self {
        Self { ff, limiter: Semaphore::new(concurrency.max(1)) }
    }

    pub async fn fingerprint(&self, req: &FingerprintRequest) -> ApiResult<Fingerprint> {
        if !self.ff.caps.chromaprint {
            return Err(ApiError::BadRequest("this FFmpeg build has no chromaprint muxer".into()));
        }
        let path = Path::new(&req.path);
        if !path.is_file() {
            return Err(ApiError::NotFound(format!("file not found: {}", req.path)));
        }
        let start_ms = req.start_ms.max(0);
        let duration_ms = req.duration_ms.clamp(1_000, MAX_DURATION_MS);
        let map = match req.stream_index {
            Some(index) => format!("0:{index}"),
            None => "0:a:0".to_owned(),
        };

        let mut cmd = self.ff.command();
        cmd.args(["-loglevel", "error"]);
        if start_ms > 0 {
            cmd.args(["-ss", &secs(start_ms)]);
        }
        cmd.args(["-t", &secs(duration_ms)]).arg("-i").arg(input_arg(path));
        // Mono 11025 Hz is what Chromaprint analyses anyway; resampling here
        // is cheaper than letting it resample a 48 kHz surround track.
        cmd.args(["-map", &map, "-vn", "-sn", "-dn", "-ac", "1", "-ar", "11025"]);
        cmd.args(["-f", "chromaprint", "-fp_format", "raw", "-"]);
        background_priority(&mut cmd);

        let _permit = self.limiter.acquire().await.map_err(|e| ApiError::Internal(e.into()))?;
        let (raw, _) = run_capture(cmd, TIMEOUT).await?;
        Ok(Fingerprint { start_ms, item_seconds: ITEM_SECONDS, data: parse_raw(&raw) })
    }
}

/// `fp_format=raw` writes the values as native-endian (little-endian on every
/// platform FFmpeg runs on) 32-bit integers.
fn parse_raw(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_little_endian_values() {
        assert_eq!(parse_raw(&[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 9]), vec![1, u32::MAX]);
        assert!((ITEM_SECONDS - 0.1238).abs() < 0.0001);
    }
}
