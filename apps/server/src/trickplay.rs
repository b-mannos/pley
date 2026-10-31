//! Seek-bar previews ("trickplay", Plex's video preview thumbnails) and the
//! chapter images cut from them. The transcoder makes the sprite sheets; they
//! are kept in the file's media folder ([`crate::store`]), so they follow the
//! file when it is moved or renamed. Files are done one at a time in the
//! background: first the titles people open (unless something is being
//! transcoded, a playback is starting, or the title itself is playing),
//! then whole libraries after scans when the setting allows, only while
//! nothing plays: what is likely to be watched soon first, the rest at the
//! hours [`crate::schedule`] gives library work. A playback that starts
//! while previews are being made stops them; they are made again later. A
//! download being converted meanwhile pauses them (the transcoder holds its
//! FFmpeg until the conversion is done).
//!
//! Chapter images and Plex's BIF index are cut from the sprite sheets, so
//! they cost no extra decoding of the video. The BIF index, larger than the
//! sheets it is cut from, is kept in the cache, for the titles Plex apps
//! played lately, rather than beside them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use image::codecs::jpeg::JpegEncoder;
use parking_lot::Mutex;
use pley_transcoder_api::TrickplayInfo;
use tokio_util::sync::CancellationToken;

use crate::catalog::{self, FileRow};
use crate::db::Db;
use crate::playback::Sessions;
use crate::schedule::{Class, Scheduler, Soon};
use crate::settings::SettingsStore;
use crate::store::Store;
use crate::transcoder::{Transcoder, TranscoderError};
use crate::util::now;

/// How often a waiting worker looks again.
const IDLE_LOOK: Duration = Duration::from_secs(15);
/// Quality of chapter images and BIF frames cut from the sheets.
const JPEG_QUALITY: u8 = 82;
/// A chapter's image is taken this far into it (or 20 % of it, if shorter),
/// past black frames and fades at the cut.
const CHAPTER_OFFSET_MS: i64 = 4_000;
/// The settings row saying the BIF indexes earlier versions kept beside the
/// previews are gone.
const OLD_BIFS_REMOVED: &str = "old_bifs_removed";

pub struct TrickplayService {
    db: Db,
    transcoder: Arc<Transcoder>,
    settings: Arc<SettingsStore>,
    sessions: Arc<Sessions>,
    schedule: Arc<Scheduler>,
    store: Arc<Store>,
    /// Plex's BIF indexes (`<data>/cache/bif`).
    bif_cache: PathBuf,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Db,
        transcoder: Arc<Transcoder>,
        settings: Arc<SettingsStore>,
        sessions: Arc<Sessions>,
        schedule: Arc<Scheduler>,
        store: Arc<Store>,
        cache: PathBuf,
        stopping: CancellationToken,
    ) -> Arc<Self> {
        let service = Arc::new(Self {
            db,
            transcoder,
            settings,
            sessions,
            schedule,
            store,
            bif_cache: cache.join("bif"),
            queues: Mutex::default(),
            queued: tokio::sync::Notify::new(),
            pending: Mutex::default(),
            failed: Mutex::default(),
            cutting: Mutex::default(),
            stopping,
        });
        // Earlier versions kept previews in the cache, keyed by file id.
        let legacy_cache = cache.join("trickplay");
        tokio::spawn(async move {
            let _ = tokio::fs::remove_dir_all(legacy_cache).await;
        });
        let (db, store) = (service.db.clone(), service.store.clone());
        tokio::spawn(async move { remove_old_bifs(&db, store).await });
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
            let (soon, later) = self.library_turn().await;
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
                    // A library's previews read whole files: what is likely
                    // to be watched soon first.
                    let soon = soon?;
                    let at = queues.library.iter().position(|id| soon.files.contains(id));
                    queues.library.remove(at.or(later.then_some(0))?).map(|id| (id, false))
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
            // Transcodes come first, a playback's or a download's.
            self.transcoder.not_transcoding().await;
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

    /// Whether the libraries' previews may be made now (see
    /// [`Scheduler::turn`]): those of what is likely to be watched soon
    /// (given then), and the rest.
    async fn library_turn(&self) -> (Option<Arc<Soon>>, bool) {
        if self.queues.lock().library.is_empty() || self.schedule.turn(Class::Soon).is_err() {
            return (None, false);
        }
        (Some(self.schedule.soon().await), self.schedule.turn(Class::Later).is_ok())
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
                // Not a request that timed out while transcodes held it:
                // only what the transcoder couldn't make.
                if matches!(err, TranscoderError::Api { .. }) {
                    self.failed.lock().insert(file.id, fingerprint);
                }
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
        if file.media_info().is_some_and(|m| m.chapters.len() > 1)
            && let Err(err) = self.chapter_images(&file).await
        {
            tracing::debug!("chapter images for file {file_id}: {err:#}");
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

    /// Plex's BIF index of the previews: the one in the cache, or else one
    /// cut from the sheets and kept there. It is named by the previews it is
    /// cut from, so previews made again otherwise get another one.
    pub async fn bif(&self, file: &FileRow) -> anyhow::Result<Option<Vec<u8>>> {
        let (Some(info), Some(sheets), Some(fingerprint)) =
            (self.index(file), self.dir(file), file.fingerprint.as_deref())
        else {
            return Ok(None);
        };
        let path = self
            .bif_cache
            .join(format!("{fingerprint}-{}-{}x{}-{}.bif", info.interval_ms, info.width, info.height, info.count));
        // Read rather than looked for: the cache's clean-up may remove it in between.
        if let Ok(bytes) = tokio::fs::read(&path).await {
            return Ok(Some(bytes));
        }
        let lock = self.cut_lock(file.id);
        let _guard = lock.lock().await;
        if let Ok(bytes) = tokio::fs::read(&path).await {
            return Ok(Some(bytes));
        }
        let dir = self.bif_cache.clone();
        let bytes = crate::util::blocking(move || {
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
            // Not kept (a full disk) costs only another cut next time.
            if let Err(err) = save(&dir, &path, &bytes) {
                tracing::debug!("could not keep {}: {err}", path.display());
            }
            Ok(bytes)
        })
        .await?;
        Ok(Some(bytes))
    }

    /// The preview frame at `ms`, out of the BIF index.
    pub async fn frame_at(&self, file: &FileRow, ms: i64) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(bif) = self.bif(file).await? else { return Ok(None) };
        Ok(bif_frame(&bif, ms).map(<[u8]>::to_vec))
    }
}

/// Earlier versions kept the BIF index beside the previews, for good: those
/// go, once. A Plex app that asks again gets one cut into the cache.
async fn remove_old_bifs(db: &Db, store: Arc<Store>) {
    let done: sqlx::Result<Option<String>> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(OLD_BIFS_REMOVED)
        .fetch_optional(&db.read)
        .await;
    if !matches!(done, Ok(None)) {
        return;
    }
    // Only video files have previews.
    let fingerprints: Vec<String> = match sqlx::query_scalar(
        "SELECT DISTINCT fingerprint FROM media_files
         WHERE video_codec IS NOT NULL AND fingerprint IS NOT NULL AND fingerprint != ''",
    )
    .fetch_all(&db.read)
    .await
    {
        Ok(fingerprints) => fingerprints,
        Err(err) => {
            tracing::warn!("listing the media folders that may hold old BIF indexes: {err}");
            return;
        }
    };
    let removed = crate::util::blocking(move || {
        let (mut count, mut bytes) = (0u64, 0u64);
        for fingerprint in &fingerprints {
            let Some(dir) = store.media_dir(fingerprint) else { continue };
            let path = dir.join("trickplay").join("index-sd.bif");
            if let Ok(meta) = std::fs::metadata(&path)
                && std::fs::remove_file(&path).is_ok()
            {
                count += 1;
                bytes += meta.len();
            }
        }
        Ok((count, bytes))
    })
    .await;
    let Ok((count, bytes)) = removed else { return };
    if let Err(err) = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?, '1')")
        .bind(OLD_BIFS_REMOVED)
        .execute(&db.write)
        .await
    {
        tracing::warn!("saving that the old BIF indexes are gone: {err}");
    }
    if count > 0 {
        tracing::info!(
            "removed {count} BIF indexes kept beside seek previews ({} MB freed): they are kept in the cache now",
            bytes / 1_000_000
        );
    }
}

/// Write `bytes` to `path` whole, through a temporary file of its own in
/// `dir`: a file played from two libraries may be cut by both at once.
fn save(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let saved = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if saved.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    saved
}

const BIF_MAGIC: [u8; 8] = [0x89, b'B', b'I', b'F', 0x0d, 0x0a, 0x1a, 0x0a];

/// A BIF file (Roku's format, used by Plex): a 64-byte header, an index of
/// (frame number, offset) pairs ending with 0xffffffff, then the JPEGs. Frame
/// `n` shows the video at `n × interval_ms`.
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

    #[tokio::test]
    async fn old_bif_indexes_go_once() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("pley.db")).await.unwrap();
        for sql in [
            "INSERT INTO libraries (id, name, kind, paths, created_at) VALUES (1, 'Movies', 'movies', '[]', 0)",
            "INSERT INTO items (id, library_id, kind, key, title, sort_title, added_at, updated_at)
             VALUES (1, 1, 'movie', 'm', 'M', 'm', 0, 0)",
            "INSERT INTO media_files (item_id, library_id, path, size, mtime, fingerprint, video_codec, added_at, updated_at)
             VALUES (1, 1, '/m/a.mkv', 1, 0, '0123456789abcdef', 'h264', 0, 0)",
        ] {
            sqlx::query(sql).execute(&db.write).await.unwrap();
        }
        let store = Arc::new(Store::new(
            db.clone(),
            &dir.path().join("metadata"),
            &dir.path().join("media"),
            &dir.path().join("cache"),
        ));
        let previews = store.media_dir("0123456789abcdef").unwrap().join("trickplay");
        std::fs::create_dir_all(&previews).unwrap();
        for name in ["0.jpg", "index.json", "index-sd.bif"] {
            std::fs::write(previews.join(name), b"kept").unwrap();
        }

        remove_old_bifs(&db, store.clone()).await;
        assert!(!previews.join("index-sd.bif").exists(), "the BIF index goes");
        assert!(previews.join("0.jpg").is_file() && previews.join("index.json").is_file(), "the previews stay");

        // Done once: what is there afterwards isn't looked for again.
        std::fs::write(previews.join("index-sd.bif"), b"kept").unwrap();
        remove_old_bifs(&db, store).await;
        assert!(previews.join("index-sd.bif").is_file());
    }

    #[test]
    fn kept_files_are_whole() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("bif");
        let path = cache.join("0123456789abcdef-10000-320x180-150.bif");
        save(&cache, &path, b"first").unwrap();
        save(&cache, &path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        let names: Vec<_> = std::fs::read_dir(&cache).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names.len(), 1, "no temporary file left: {names:?}");
    }
}
