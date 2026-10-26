//! Seek-bar previews ("trickplay", Plex's video preview thumbnails) and the
//! chapter images cut from them. The transcoder makes the sprite sheets; they
//! are kept in the file's media folder ([`crate::store`]), so they follow the
//! file when it is moved or renamed. Files are done one at a time in the
//! background: first the titles people open (unless the server is busy
//! transcoding, a playback is starting, or the title itself is playing),
//! then whole libraries after scans
//! when the setting allows, only while nothing plays. A playback that starts
//! while previews are being made stops them; they are made again later.
//!
//! Chapter images and Plex's BIF index (`index-sd.bif`) are cut from the
//! sprite sheets, so they cost no extra decoding of the video.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use image::codecs::jpeg::JpegEncoder;
use parking_lot::Mutex;
use pley_transcoder_api::TrickplayInfo;
use tokio_util::sync::CancellationToken;

use crate::catalog::{self, FileRow};
use crate::db::Db;
use crate::playback::{QUIET_BEFORE_BACKGROUND_SECS, Sessions};
use crate::settings::SettingsStore;
use crate::store::Store;
use crate::transcoder::Transcoder;
use crate::util::now;

/// Previews wait while this many transcodes run.
const BUSY_TRANSCODES: usize = 2;
/// How often a waiting worker looks again.
const IDLE_LOOK: Duration = Duration::from_secs(15);
/// Quality of chapter images and BIF frames cut from the sheets.
const JPEG_QUALITY: u8 = 82;
/// A chapter's image is taken this far into it (or 20 % of it, if shorter),
/// past black frames and fades at the cut.
const CHAPTER_OFFSET_MS: i64 = 4_000;

pub struct TrickplayService {
    db: Db,
    transcoder: Arc<Transcoder>,
    settings: Arc<SettingsStore>,
    sessions: Arc<Sessions>,
    store: Arc<Store>,
    /// Files waiting: the ones people opened first, then the libraries'.
    queues: Mutex<Queues>,
    /// A file was queued.
    queued: tokio::sync::Notify,
    pending: Mutex<HashSet<i64>>,
    /// File versions that could not be done (by fingerprint); retried after a restart.
    failed: Mutex<HashMap<i64, String>>,
    /// One chapter-image or BIF job per file at a time.
    cutting: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
    /// Cancelled when the server shuts down: the worker stops.
    stopping: CancellationToken,
}

#[derive(Default)]
struct Queues {
    opened: VecDeque<i64>,
    library: VecDeque<i64>,
}

/// What `TrickplayInfo` says about a tile: its sheet and position.
fn tile(info: &TrickplayInfo, n: u32) -> (u32, u32, u32) {
    let per_sheet = (info.columns * info.rows).max(1);
    let in_sheet = n % per_sheet;
    (n / per_sheet, (in_sheet % info.columns) * info.width, (in_sheet / info.columns) * info.height)
}

/// The tile shown at `ms` (the last one past the end).
fn tile_at(info: &TrickplayInfo, ms: i64) -> u32 {
    let n = (ms.max(0) / info.interval_ms.max(1)) as u32;
    n.min(info.count.saturating_sub(1))
}

fn encode_jpeg(img: &image::RgbImage) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(img.as_raw().len() / 8);
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(img)?;
    Ok(out)
}

impl TrickplayService {
    pub fn new(
        db: Db,
        transcoder: Arc<Transcoder>,
        settings: Arc<SettingsStore>,
        sessions: Arc<Sessions>,
        store: Arc<Store>,
        legacy_cache: PathBuf,
        stopping: CancellationToken,
    ) -> Arc<Self> {
        let service = Arc::new(Self {
            db,
            transcoder,
            settings,
            sessions,
            store,
            queues: Mutex::default(),
            queued: tokio::sync::Notify::new(),
            pending: Mutex::default(),
            failed: Mutex::default(),
            cutting: Mutex::default(),
            stopping,
        });
        // Earlier versions kept previews in the cache, keyed by file id.
        tokio::spawn(async move {
            let _ = tokio::fs::remove_dir_all(legacy_cache).await;
        });
        tokio::spawn(service.clone().worker());
        service
    }

    fn dir(&self, file: &FileRow) -> Option<PathBuf> {
        Some(self.store.media_dir(file.fingerprint.as_deref()?)?.join("trickplay"))
    }

    /// The previews of this file, if they exist.
    pub fn index(&self, file: &FileRow) -> Option<TrickplayInfo> {
        let bytes = std::fs::read(self.dir(file)?.join("index.json")).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn sheet(&self, file: &FileRow, n: u32) -> Option<PathBuf> {
        Some(self.dir(file)?.join(format!("{n}.jpg"))).filter(|p| p.is_file())
    }

    /// Files waiting for previews.
    pub fn pending(&self) -> usize {
        self.pending.lock().len()
    }

    /// Queue the file unless its previews exist, are queued or failed.
    pub fn request(&self, file: &FileRow) {
        if !self.settings.get().seek_previews || file.video_codec.is_none() || self.index(file).is_some() {
            return;
        }
        self.enqueue(file.id, file.fingerprint.as_deref(), true);
    }

    /// Queue a file without previews unless it is queued or failed: one
    /// someone opened (`opened`) goes before the libraries' backlog, also
    /// when it was already in it.
    fn enqueue(&self, file_id: i64, fingerprint: Option<&str>, opened: bool) {
        if fingerprint.is_some() && self.failed.lock().get(&file_id).map(String::as_str) == fingerprint {
            return;
        }
        let new = self.pending.lock().insert(file_id);
        let mut queues = self.queues.lock();
        if opened {
            if !new {
                if queues.opened.contains(&file_id) {
                    return;
                }
                queues.library.retain(|id| *id != file_id);
            }
            queues.opened.push_back(file_id);
        } else if new {
            queues.library.push_back(file_id);
        } else {
            return;
        }
        drop(queues);
        self.queued.notify_one();
    }

    /// Queue every video file without previews, newest first (when previews
    /// are made for whole libraries).
    pub async fn request_all(&self) {
        let settings = self.settings.get();
        if !settings.seek_previews || !settings.previews_in_background {
            return;
        }
        let files = match catalog::video_file_ids(&self.db).await {
            Ok(files) => files,
            Err(err) => {
                tracing::warn!("listing files for seek previews: {err}");
                return;
            }
        };
        // Which ones have previews is a question for the disk (one look per
        // file): asked off the async workers.
        let store = self.store.clone();
        let missing = tokio::task::spawn_blocking(move || {
            let done =
                |fp: &str| store.media_dir(fp).is_some_and(|dir| dir.join("trickplay").join("index.json").is_file());
            files.into_iter().filter(|(_, fp)| !fp.as_deref().is_some_and(done)).collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        let before = self.pending();
        for (id, fingerprint) in &missing {
            self.enqueue(*id, fingerprint.as_deref(), false);
        }
        let queued = self.pending().saturating_sub(before);
        if queued > 0 {
            tracing::info!("making seek previews for {queued} files in the background");
        }
    }

    /// Makes what is queued until the server shuts down, which drops the file
    /// in progress (its half-made previews are cleared next time): it would
    /// only fail without the database and the transcoder.
    async fn worker(self: Arc<Self>) {
        let stopping = self.stopping.clone();
        tokio::select! {
            () = stopping.cancelled() => {}
            () = self.work() => {}
        }
    }

    async fn work(&self) {
        loop {
            // A playback that is starting gets the disk and the network to
            // itself: even the titles people opened wait (their player asks
            // again every 20 s, so they still show up during the play).
            let starting = self.sessions.starting_for();
            let playing = self.playing_files();
            let next = {
                let mut queues = self.queues.lock();
                // A title being played waits for its session to end: reading
                // the whole file again would compete with the play's own reads
                // (a 4K remux on a network share needs all of it).
                let opened = if starting.is_none() {
                    let at = queues.opened.iter().position(|id| !playing.contains(id));
                    at.and_then(|at| queues.opened.remove(at))
                } else {
                    None
                };
                opened.map(|id| (id, true)).or_else(|| {
                    // A library's previews read whole files: only while nothing plays.
                    if self.sessions.quiet_for() >= QUIET_BEFORE_BACKGROUND_SECS {
                        queues.library.pop_front().map(|id| (id, false))
                    } else {
                        None
                    }
                })
            };
            let Some((file_id, opened)) = next else {
                // Until something is queued, a playback starts or ends, or a
                // look again (the start is over, or playback may have ended).
                let wait = starting.map_or(IDLE_LOOK, |left| left.min(IDLE_LOOK));
                tokio::select! {
                    _ = tokio::time::timeout(wait, self.queued.notified()) => {}
                    () = self.sessions.changed() => {}
                }
                continue;
            };
            while self.sessions.active_transcodes() >= BUSY_TRANSCODES {
                tokio::time::sleep(IDLE_LOOK).await;
            }
            if self.sessions.starting_for().is_some() {
                self.requeue(file_id, opened);
                continue;
            }
            // A playback starting meanwhile stops this one (dropping the
            // request stops the transcoder's FFmpeg): it is queued again, as
            // it was, and never counts as failed.
            let starts = self.sessions.starts();
            tokio::select! {
                result = self.make(file_id) => {
                    if let Err(err) = result {
                        tracing::warn!("seek previews for file {file_id} failed: {err:#}");
                    }
                    self.pending.lock().remove(&file_id);
                }
                () = self.sessions.started_since(starts) => {
                    tracing::debug!("seek previews for file {file_id} put off: a playback is starting");
                    self.requeue(file_id, opened);
                }
            }
        }
    }

    /// The files sessions are playing now (paused ones included, until they go quiet).
    fn playing_files(&self) -> HashSet<i64> {
        let now = now();
        self.sessions.list().iter().filter(|s| s.is_active(now)).map(|s| s.file_id).collect()
    }

    /// Put a file whose previews were put off back at the head of its queue.
    fn requeue(&self, file_id: i64, opened: bool) {
        let mut queues = self.queues.lock();
        if !opened && queues.opened.contains(&file_id) {
            // Someone opened it meanwhile: it is queued already.
            return;
        }
        let queue = if opened { &mut queues.opened } else { &mut queues.library };
        queue.retain(|id| *id != file_id);
        queue.push_front(file_id);
    }

    async fn make(&self, file_id: i64) -> anyhow::Result<()> {
        let Some(mut file) = catalog::file(&self.db, file_id).await? else { return Ok(()) };
        let fingerprint = self.store.fingerprint_of(file.id, &file.path, file.fingerprint.as_deref()).await?;
        file.fingerprint = Some(fingerprint.clone());
        if self.index(&file).is_some() {
            return Ok(());
        }
        if !self.transcoder.is_available() && !self.transcoder.wait_ready(Duration::from_secs(60)).await {
            anyhow::bail!("the transcoder is not available");
        }
        let info = match self.transcoder.trickplay(&file.path).await {
            Ok(info) => info,
            Err(err) => {
                self.failed.lock().insert(file.id, fingerprint);
                return Err(err.into());
            }
        };
        let target = self.dir(&file).context("no media folder")?;
        let part = target.with_extension("part");
        let _ = tokio::fs::remove_dir_all(&part).await;
        tokio::fs::create_dir_all(&part).await.with_context(|| format!("creating {}", part.display()))?;
        for n in 0..info.sheets {
            let bytes = self.transcoder.trickplay_sheet(&info.id, n).await?;
            tokio::fs::write(part.join(format!("{n}.jpg")), bytes).await?;
        }
        tokio::fs::write(part.join("index.json"), serde_json::to_vec(&info)?).await?;
        let _ = tokio::fs::remove_dir_all(&target).await;
        tokio::fs::rename(&part, &target).await?;
        // Chapter images come from the same sheets; make them while they are warm.
        if file.media_info().is_some_and(|m| m.chapters.len() > 1) {
            if let Err(err) = self.chapter_images(&file).await {
                tracing::debug!("chapter images for file {file_id}: {err:#}");
            }
        }
        Ok(())
    }

    fn cut_lock(&self, file_id: i64) -> Arc<tokio::sync::Mutex<()>> {
        self.cutting.lock().entry(file_id).or_default().clone()
    }

    fn chapters_dir(&self, file: &FileRow) -> Option<PathBuf> {
        Some(self.store.media_dir(file.fingerprint.as_deref()?)?.join("chapters"))
    }

    /// Whether the file's chapters have images (or can get them right away).
    pub fn has_chapter_images(&self, file: &FileRow) -> bool {
        self.index(file).is_some()
    }

    /// The image of chapter `index`, cut from the previews when missing.
    pub async fn chapter_image(&self, file: &FileRow, index: usize) -> anyhow::Result<Option<PathBuf>> {
        let Some(dir) = self.chapters_dir(file) else { return Ok(None) };
        let path = dir.join(format!("{index}.jpg"));
        if !path.is_file() {
            self.chapter_images(file).await?;
        }
        Ok(Some(path).filter(|p| p.is_file()))
    }

    /// Cut an image for every chapter out of the sprite sheets.
    async fn chapter_images(&self, file: &FileRow) -> anyhow::Result<()> {
        let lock = self.cut_lock(file.id);
        let _guard = lock.lock().await;
        let (Some(info), Some(dir), Some(sheets)) = (self.index(file), self.chapters_dir(file), self.dir(file)) else {
            return Ok(());
        };
        let chapters = file.media_info().map(|m| m.chapters).unwrap_or_default();
        if chapters.is_empty() || dir.join(format!("{}.jpg", chapters.len() - 1)).is_file() {
            return Ok(());
        }
        let tiles: Vec<u32> = chapters
            .iter()
            .map(|c| {
                let length = (c.end_ms - c.start_ms).max(0);
                tile_at(&info, c.start_ms + CHAPTER_OFFSET_MS.min(length / 5))
            })
            .collect();
        crate::util::blocking(move || {
            std::fs::create_dir_all(&dir)?;
            let mut decoded: HashMap<u32, image::RgbImage> = HashMap::new();
            for (index, n) in tiles.into_iter().enumerate() {
                let (sheet, x, y) = tile(&info, n);
                if let std::collections::hash_map::Entry::Vacant(slot) = decoded.entry(sheet) {
                    let img = image::open(sheets.join(format!("{sheet}.jpg")))?.to_rgb8();
                    slot.insert(img);
                }
                let img = &decoded[&sheet];
                let crop = image::imageops::crop_imm(img, x, y, info.width, info.height).to_image();
                let tmp = dir.join(format!("{index}.jpg.tmp"));
                std::fs::write(&tmp, encode_jpeg(&crop)?)?;
                std::fs::rename(&tmp, dir.join(format!("{index}.jpg")))?;
            }
            Ok(())
        })
        .await
    }

    /// Plex's BIF index of the previews (`index-sd.bif`), made once.
    pub async fn bif(&self, file: &FileRow) -> anyhow::Result<Option<PathBuf>> {
        let (Some(info), Some(sheets)) = (self.index(file), self.dir(file)) else { return Ok(None) };
        let path = sheets.join("index-sd.bif");
        if path.is_file() {
            return Ok(Some(path));
        }
        let lock = self.cut_lock(file.id);
        let _guard = lock.lock().await;
        if path.is_file() {
            return Ok(Some(path));
        }
        let target = path.clone();
        crate::util::blocking(move || {
            let mut frames = Vec::with_capacity(info.count as usize);
            for sheet in 0..info.sheets {
                let img = image::open(sheets.join(format!("{sheet}.jpg")))?.to_rgb8();
                let per_sheet = info.columns * info.rows;
                for k in 0..per_sheet {
                    let n = sheet * per_sheet + k;
                    if n >= info.count {
                        break;
                    }
                    let (_, x, y) = tile(&info, n);
                    let crop = image::imageops::crop_imm(&img, x, y, info.width, info.height).to_image();
                    frames.push(encode_jpeg(&crop)?);
                }
            }
            let bytes = bif_bytes(&frames, info.interval_ms.clamp(1, i64::from(u32::MAX)) as u32);
            let tmp = target.with_extension("bif.tmp");
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, &target)?;
            Ok(())
        })
        .await?;
        Ok(Some(path))
    }

    /// The preview frame at `ms`, out of the BIF index.
    pub async fn frame_at(&self, file: &FileRow, ms: i64) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(path) = self.bif(file).await? else { return Ok(None) };
        let bytes = tokio::fs::read(&path).await?;
        Ok(bif_frame(&bytes, ms).map(<[u8]>::to_vec))
    }
}

const BIF_MAGIC: [u8; 8] = [0x89, b'B', b'I', b'F', 0x0d, 0x0a, 0x1a, 0x0a];

/// A BIF file (Roku's format, used by Plex): a 64-byte header, an index of
/// (frame number, offset) pairs ending with 0xffffffff, then the JPEGs. Frame
/// `n` shows the video at `n Ã— interval_ms`.
fn bif_bytes(frames: &[Vec<u8>], interval_ms: u32) -> Vec<u8> {
    let index_len = (frames.len() + 1) * 8;
    let mut out = Vec::with_capacity(64 + index_len + frames.iter().map(Vec::len).sum::<usize>());
    out.extend_from_slice(&BIF_MAGIC);
    out.extend_from_slice(&0u32.to_le_bytes()); // version
    out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
    out.extend_from_slice(&interval_ms.to_le_bytes());
    out.resize(64, 0);
    let mut offset = (64 + index_len) as u32;
    for (n, frame) in frames.iter().enumerate() {
        out.extend_from_slice(&(n as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += frame.len() as u32;
    }
    out.extend_from_slice(&u32::MAX.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    for frame in frames {
        out.extend_from_slice(frame);
    }
    out
}

/// The frame of a BIF file shown at `ms`.
fn bif_frame(bif: &[u8], ms: i64) -> Option<&[u8]> {
    if bif.len() < 64 || bif[..8] != BIF_MAGIC {
        return None;
    }
    let word = |at: usize| bif.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let count = word(12)? as usize;
    let interval = word(16)?.max(1) as i64;
    if count == 0 {
        return None;
    }
    let n = ((ms.max(0) / interval) as usize).min(count - 1);
    let start = word(64 + n * 8 + 4)? as usize;
    let end = word(64 + (n + 1) * 8 + 4)? as usize;
    bif.get(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> TrickplayInfo {
        TrickplayInfo {
            id: "x".into(),
            interval_ms: 10_000,
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            count: 150,
            sheets: 2,
        }
    }

    #[test]
    fn tiles_map_to_sheets() {
        let info = info();
        assert_eq!(tile(&info, 0), (0, 0, 0));
        assert_eq!(tile(&info, 13), (0, 3 * 320, 180));
        assert_eq!(tile(&info, 104), (1, 4 * 320, 0));
        assert_eq!(tile_at(&info, 25_000), 2);
        assert_eq!(tile_at(&info, 99_999_999), 149, "past the end: the last tile");
    }

    #[test]
    fn bif_index_finds_frames() {
        let frames = vec![b"first".to_vec(), b"second".to_vec(), b"third".to_vec()];
        let bif = bif_bytes(&frames, 5_000);
        assert_eq!(&bif[..8], &BIF_MAGIC);
        assert_eq!(bif_frame(&bif, 0), Some(&b"first"[..]));
        assert_eq!(bif_frame(&bif, 7_400), Some(&b"second"[..]));
        assert_eq!(bif_frame(&bif, 60_000), Some(&b"third"[..]));
        assert_eq!(bif_frame(b"not a bif", 0), None);
    }
}
