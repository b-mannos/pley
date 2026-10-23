//! Artwork delivery: resizes images to a few width buckets (cached on disk so
//! each size is only computed once) and generates missing artwork on demand
//! through the transcoder (video frame grabs, embedded cover art).

use std::io::{BufRead, Cursor, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageReader, ImageResult, Limits};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use crate::error::AppError;

/// Pictures come from users and the internet, and a small file can claim
/// huge dimensions: decoding stops at these limits instead of allocating.
const MAX_DIMENSION: u32 = 12_000;
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

/// Uploaded pictures decoded at the same time (each can take a few hundred MB).
pub static UPLOAD_DECODES: Semaphore = Semaphore::const_new(2);

fn limited<R: BufRead + Seek>(mut reader: ImageReader<R>) -> ImageResult<DynamicImage> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    reader.decode()
}

/// Decode a picture (any supported format) within the size limits.
pub fn decode(bytes: &[u8]) -> ImageResult<DynamicImage> {
    limited(ImageReader::new(Cursor::new(bytes)).with_guessed_format()?)
}

const BUCKETS: [u32; 8] = [160, 240, 320, 480, 640, 960, 1280, 1920];
const JPEG_QUALITY: u8 = 82;

pub struct Images {
    cache_dir: PathBuf,
    limiter: Semaphore,
    /// Per-item locks so concurrent requests don't generate the same artwork twice.
    generating: DashMap<i64, Arc<tokio::sync::Mutex<()>>>,
}

pub struct ImageBytes {
    pub bytes: Bytes,
    pub content_type: &'static str,
    pub etag: String,
}

impl Images {
    pub fn new(cache_dir: PathBuf) -> Self {
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
        Self { cache_dir, limiter: Semaphore::new(workers), generating: DashMap::new() }
    }

    pub fn generation_lock(&self, item_id: i64) -> Arc<tokio::sync::Mutex<()>> {
        self.generating.entry(item_id).or_default().clone()
    }

    pub fn release_generation_lock(&self, item_id: i64) {
        self.generating.remove_if(&item_id, |_, lock| Arc::strong_count(lock) <= 1);
    }

    /// Serve `source`, resized to the bucket at or above `width` when smaller
    /// than the original.
    pub async fn load(&self, source: &Path, width: Option<u32>) -> Result<ImageBytes, AppError> {
        let meta = tokio::fs::metadata(source).await.map_err(|_| AppError::not_found("image not found"))?;
        let mtime = meta.modified().ok().map(crate::util::system_time_secs).unwrap_or(0);
        let bucket = width.map(|w| *BUCKETS.iter().find(|b| **b >= w).unwrap_or(&BUCKETS[BUCKETS.len() - 1]));
        let mut hasher = Sha256::new();
        hasher.update(source.to_string_lossy().as_bytes());
        hasher.update(meta.len().to_le_bytes());
        hasher.update(mtime.to_le_bytes());
        hasher.update(bucket.unwrap_or(0).to_le_bytes());
        let key = hex::encode(&hasher.finalize()[..16]);
        let etag = format!("\"{key}\"");

        let Some(bucket) = bucket else {
            let bytes = tokio::fs::read(source).await?;
            return Ok(ImageBytes { content_type: content_type_of(source, &bytes), bytes: bytes.into(), etag });
        };

        let cached = self.cache_dir.join(&key[..2]).join(format!("{key}.jpg"));
        if let Ok(bytes) = tokio::fs::read(&cached).await {
            return Ok(ImageBytes { bytes: bytes.into(), content_type: "image/jpeg", etag });
        }

        let _permit = self.limiter.acquire().await.map_err(|e| AppError::Internal(e.into()))?;
        let src = source.to_path_buf();
        let out = cached.clone();
        let result = crate::util::blocking(move || resize_to_file(&src, bucket, &out)).await;
        match result {
            Ok(Some(bytes)) => Ok(ImageBytes { bytes: bytes.into(), content_type: "image/jpeg", etag }),
            // Already small enough (or not decodable): serve the original.
            Ok(None) | Err(_) => {
                let bytes = tokio::fs::read(source).await?;
                Ok(ImageBytes { content_type: content_type_of(source, &bytes), bytes: bytes.into(), etag })
            }
        }
    }
}

/// Resize to `width` (keeping aspect ratio) and write a JPEG. Returns `None`
/// when the source is not wider than the target.
fn resize_to_file(source: &Path, width: u32, out: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let reader = ImageReader::open(source)?.with_guessed_format()?;
    let (w, h) = reader.into_dimensions()?;
    if w <= width {
        return Ok(None);
    }
    let img = limited(ImageReader::open(source)?.with_guessed_format()?)?;
    let height = ((h as f64) * (width as f64) / (w as f64)).round().max(1.0) as u32;
    let resized = img.resize_exact(width, height, image::imageops::FilterType::Triangle).to_rgb8();
    let mut buf = Vec::with_capacity((width * height / 4) as usize);
    JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY).encode_image(&resized)?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = out.with_extension("tmp");
    std::fs::write(&tmp, &buf)?;
    std::fs::rename(&tmp, out)?;
    Ok(Some(buf))
}

pub fn content_type_of(path: &Path, bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "image/png";
    }
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return "image/jpeg";
    }
    match path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resizes_and_caches() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("poster.png");
        image::RgbImage::from_pixel(1000, 1500, image::Rgb([200, 100, 50])).save(&src).unwrap();
        let images = Images::new(dir.path().join("cache"));

        let small = images.load(&src, Some(300)).await.unwrap();
        assert_eq!(small.content_type, "image/jpeg");
        let decoded = image::load_from_memory(&small.bytes).unwrap();
        assert_eq!(decoded.width(), 320);
        assert_eq!(decoded.height(), 480);

        // Second call is served from the cache with the same ETag.
        let again = images.load(&src, Some(300)).await.unwrap();
        assert_eq!(again.etag, small.etag);

        // Wider than the source: original bytes.
        let original = images.load(&src, Some(1920)).await.unwrap();
        assert_eq!(original.content_type, "image/png");
    }
}
