//! Library scanning.
//!
//! A single worker processes scan requests one library at a time. A scan walks
//! the library folders, diffs the files against the database (size + mtime),
//! probes new/changed files through the transcoder with bounded concurrency,
//! writes items in short transactions, removes vanished files (unless their
//! drive is offline), refreshes sidecar artwork/subtitles, recomputes counts and
//! finally lets the metadata agent enrich new items.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use parking_lot::Mutex;
use pley_transcoder_api::{MediaInfo, StreamKind};
use serde::Serialize;
use sqlx::SqliteConnection;
use tokio::sync::mpsc;

use super::parse::{parse_episode, parse_movie, parse_track};
use super::sidecar::{ExternalSub, external_subtitles};
use super::walk::{FoundFile, find_image, walk};
use super::{LibraryKind, LibraryRow, get_library, list_libraries};
use crate::config::Config;
use crate::db::Db;
use crate::events::{EventBus, ScanPhase, ServerEvent};
use crate::metadata::MetadataAgent;
use crate::transcoder::{Transcoder, TranscoderError};
use crate::util::{normalize_key, now, path_str, sort_title};

const PROBE_CONCURRENCY: usize = 6;
const WRITE_BATCH: usize = 64;

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatus {
    pub library_id: i64,
    pub library_name: String,
    pub phase: ScanPhase,
    pub processed: u64,
    pub total: u64,
    pub started_at: i64,
}

#[derive(Debug, Clone, Copy)]
struct ScanRequest {
    library_id: i64,
    force: bool,
}

pub struct Scanner {
    db: Db,
    transcoder: Arc<Transcoder>,
    events: EventBus,
    metadata: Arc<MetadataAgent>,
    metadata_dir: PathBuf,
    queue: mpsc::UnboundedSender<ScanRequest>,
    status: Mutex<HashMap<i64, ScanStatus>>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Totals {
    added: u64,
    updated: u64,
    removed: u64,
}

impl Scanner {
    pub fn new(
        db: Db,
        transcoder: Arc<Transcoder>,
        events: EventBus,
        metadata: Arc<MetadataAgent>,
        config: &Config,
    ) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let scanner = Arc::new(Self {
            db,
            transcoder,
            events,
            metadata,
            metadata_dir: config.metadata_dir(),
            queue: tx,
            status: Mutex::default(),
        });
        tokio::spawn(scanner.clone().worker(rx));
        scanner
    }

    pub fn enqueue(&self, library_id: i64, force: bool) {
        self.status.lock().entry(library_id).or_insert_with(|| ScanStatus {
            library_id,
            library_name: String::new(),
            phase: ScanPhase::Queued,
            processed: 0,
            total: 0,
            started_at: now(),
        });
        let _ = self.queue.send(ScanRequest { library_id, force });
        self.events.send(ServerEvent::ScanProgress { library_id, phase: ScanPhase::Queued, processed: 0, total: 0 });
    }

    pub async fn enqueue_all(&self, force: bool) -> sqlx::Result<()> {
        for lib in list_libraries(&self.db).await? {
            self.enqueue(lib.id, force);
        }
        Ok(())
    }

    pub fn statuses(&self) -> Vec<ScanStatus> {
        let mut list: Vec<_> = self.status.lock().values().cloned().collect();
        list.sort_by_key(|s| s.started_at);
        list
    }

    pub fn is_scanning(&self, library_id: i64) -> bool {
        self.status.lock().contains_key(&library_id)
    }

    async fn worker(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<ScanRequest>) {
        let mut pending: Vec<ScanRequest> = Vec::new();
        loop {
            if pending.is_empty() {
                match rx.recv().await {
                    Some(req) => pending.push(req),
                    None => break,
                }
            }
            while let Ok(req) = rx.try_recv() {
                match pending.iter_mut().find(|p| p.library_id == req.library_id) {
                    Some(existing) => existing.force |= req.force,
                    None => pending.push(req),
                }
            }
            let req = pending.remove(0);
            let started = Instant::now();
            match self.scan(req.library_id, req.force).await {
                Ok(Some(t)) => tracing::info!(
                    "library {} scanned in {:.1}s: {} added, {} updated, {} removed",
                    req.library_id,
                    started.elapsed().as_secs_f32(),
                    t.added,
                    t.updated,
                    t.removed
                ),
                Ok(None) => {}
                Err(err) => tracing::error!("scan of library {} failed: {err:#}", req.library_id),
            }
            if !pending.iter().any(|p| p.library_id == req.library_id) {
                self.status.lock().remove(&req.library_id);
            }
            self.events.send(ServerEvent::ScanProgress {
                library_id: req.library_id,
                phase: ScanPhase::Done,
                processed: 0,
                total: 0,
            });
        }
    }

    fn set_status(&self, lib: &LibraryRow, phase: ScanPhase, processed: u64, total: u64) {
        let mut status = self.status.lock();
        let entry = status.entry(lib.id).or_insert_with(|| ScanStatus {
            library_id: lib.id,
            library_name: lib.name.clone(),
            phase,
            processed,
            total,
            started_at: now(),
        });
        entry.library_name = lib.name.clone();
        entry.phase = phase;
        entry.processed = processed;
        entry.total = total;
        drop(status);
        self.events.send(ServerEvent::ScanProgress { library_id: lib.id, phase, processed, total });
    }

    async fn scan(&self, library_id: i64, force: bool) -> anyhow::Result<Option<Totals>> {
        let Some(lib) = get_library(&self.db, library_id).await? else {
            return Ok(None);
        };
        let kind = lib.kind();
        let roots = lib.paths();
        self.events.send(ServerEvent::ScanStarted { library_id });
        self.set_status(&lib, ScanPhase::Walking, 0, 0);

        let walked = {
            let roots = roots.clone();
            crate::util::blocking(move || Ok(walk(&roots, kind))).await?
        };

        let existing: Vec<(i64, String, i64, i64, bool)> =
            sqlx::query_as("SELECT id, path, size, mtime, probed FROM media_files WHERE library_id = ?")
                .bind(library_id)
                .fetch_all(&self.db.read)
                .await?;
        let existing_by_path: HashMap<&str, (i64, i64, i64, bool)> = existing
            .iter()
            .map(|(id, path, size, mtime, probed)| (path.as_str(), (*id, *size, *mtime, *probed)))
            .collect();

        let mut to_process = Vec::new();
        let mut unchanged = Vec::new();
        let mut seen = HashSet::new();
        for file in &walked.media {
            let key = path_str(&file.path);
            match existing_by_path.get(key.as_str()) {
                Some((id, size, mtime, probed)) if !force && *size == file.size && *mtime == file.mtime && *probed => {
                    unchanged.push((*id, file.path.clone()));
                }
                _ => to_process.push(file.clone()),
            }
            seen.insert(key);
        }
        // A root that suddenly holds no media at all is far more likely an
        // unmounted drive (the mount point exists but is empty) than a library
        // whose every file was deleted: keep its items.
        let mut walked = walked;
        for root in &roots {
            let empty = !walked.media.iter().any(|f| &f.root == root);
            if empty
                && !walked.offline_roots.contains(root)
                && existing.iter().any(|(_, path, ..)| Path::new(path).starts_with(root))
            {
                tracing::warn!("library folder {} is empty; assuming its drive is not mounted", root.display());
                walked.offline_roots.push(root.clone());
            }
        }
        let removed: Vec<i64> = existing
            .iter()
            .filter(|(_, path, ..)| !seen.contains(path) && !walked.maybe_unavailable(Path::new(path)))
            .map(|(id, ..)| *id)
            .collect();

        let mut totals = Totals { removed: removed.len() as u64, ..Default::default() };
        let total = to_process.len() as u64;
        self.set_status(&lib, ScanPhase::Probing, 0, total);
        if !to_process.is_empty() && !self.transcoder.is_available() {
            self.transcoder.wait_ready(Duration::from_secs(90)).await;
        }

        let mut writer = Writer::new(&lib, &walked.sidecars);
        let mut batch: Vec<(FoundFile, ProbeOutcome)> = Vec::with_capacity(WRITE_BATCH);
        let mut processed = 0u64;
        let mut last_emit = Instant::now();
        let mut last_flush = Instant::now();
        let transcoder = self.transcoder.clone();
        let mut results = futures::stream::iter(to_process.into_iter().map(|file| {
            let transcoder = transcoder.clone();
            async move {
                let outcome = match transcoder.probe(&path_str(&file.path)).await {
                    Ok(info) => ProbeOutcome::Ok(Box::new(info)),
                    // ffprobe ran and cannot read the file: don't retry every scan.
                    Err(TranscoderError::Api { code, message, .. }) if code == "ffmpeg_failed" => {
                        tracing::warn!("could not probe {}: {message}", file.path.display());
                        ProbeOutcome::Failed
                    }
                    // Unreachable transcoder, timeout (sleeping NAS), …: try again next scan.
                    Err(err) => {
                        tracing::debug!("probe of {} postponed: {err}", file.path.display());
                        ProbeOutcome::Unavailable
                    }
                };
                (file, outcome)
            }
        }))
        .buffer_unordered(PROBE_CONCURRENCY);

        while let Some(result) = results.next().await {
            batch.push(result);
            processed += 1;
            if batch.len() >= WRITE_BATCH || last_flush.elapsed() > Duration::from_millis(750) {
                self.flush(&mut writer, &mut batch, &existing_by_path, &mut totals).await?;
                last_flush = Instant::now();
            }
            if last_emit.elapsed() > Duration::from_millis(400) {
                self.set_status(&lib, ScanPhase::Probing, processed, total);
                last_emit = Instant::now();
            }
        }
        self.flush(&mut writer, &mut batch, &existing_by_path, &mut totals).await?;
        self.set_status(&lib, ScanPhase::Probing, processed, total);

        let mut conn = self.db.write.acquire().await?;
        for chunk in removed.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!("DELETE FROM media_files WHERE id IN ({placeholders})");
            let mut q = sqlx::query(&sql);
            for id in chunk {
                q = q.bind(id);
            }
            q.execute(&mut *conn).await?;
        }
        cleanup_orphans(&mut conn, library_id).await?;
        refresh_external_subs(&mut conn, &unchanged, &walked.sidecars).await?;
        refresh_artwork(&mut conn, &lib, &walked.sidecars, &self.metadata_dir).await?;
        update_counts(&mut conn, library_id).await?;
        sqlx::query("UPDATE libraries SET scanned_at = ? WHERE id = ?")
            .bind(now())
            .bind(library_id)
            .execute(&mut *conn)
            .await?;
        drop(conn);
        // Scanning rewrites titles of unmatched items and local artwork;
        // hand edits win.
        crate::overrides::apply_library(&self.db, library_id).await?;
        self.events.send(ServerEvent::LibraryChanged { library_id });

        if kind != LibraryKind::Music && self.metadata.enabled() {
            self.set_status(&lib, ScanPhase::Metadata, 0, 0);
            let progress = |done: u64, total: u64| self.set_status(&lib, ScanPhase::Metadata, done, total);
            if let Err(err) = self.metadata.refresh_library(&lib, force, &progress).await {
                tracing::warn!("metadata refresh for library {} failed: {err:#}", lib.id);
            }
            self.events.send(ServerEvent::LibraryChanged { library_id });
        }
        if totals.removed > 0 {
            self.metadata.prune_people().await;
        }

        self.set_status(&lib, ScanPhase::Done, processed, total);
        self.events.send(ServerEvent::ScanFinished {
            library_id,
            added: totals.added,
            updated: totals.updated,
            removed: totals.removed,
        });
        self.db.optimize().await;
        Ok(Some(totals))
    }

    async fn flush(
        &self,
        writer: &mut Writer<'_>,
        batch: &mut Vec<(FoundFile, ProbeOutcome)>,
        existing: &HashMap<&str, (i64, i64, i64, bool)>,
        totals: &mut Totals,
    ) -> anyhow::Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut tx = self.db.write.begin().await?;
        for (file, outcome) in batch.drain(..) {
            let known = existing.get(path_str(&file.path).as_str());
            // A forced rescan while the transcoder is down must not replace
            // good probe data (and music tags) with nothing.
            if matches!(outcome, ProbeOutcome::Unavailable)
                && known.is_some_and(|&(_, size, mtime, _)| size == file.size && mtime == file.mtime)
            {
                continue;
            }
            let is_new = known.is_none();
            match writer.write(&mut tx, &file, outcome).await {
                Ok(()) if is_new => totals.added += 1,
                Ok(()) => totals.updated += 1,
                Err(err) => tracing::warn!("could not save {}: {err:#}", file.path.display()),
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

enum ProbeOutcome {
    Ok(Box<MediaInfo>),
    /// The transcoder could not be reached or timed out: store the file
    /// unprobed (unless it is already known), retry next scan.
    Unavailable,
    /// ffprobe rejected the file: store it as probed so we don't retry every scan.
    Failed,
}

/// Writes items for scanned files, caching container ids for the scan.
struct Writer<'a> {
    library_id: i64,
    kind: LibraryKind,
    sidecars: &'a HashMap<PathBuf, Vec<String>>,
    containers: HashMap<String, i64>,
}

struct NewItem {
    parent_id: Option<i64>,
    grandparent_id: Option<i64>,
    kind: &'static str,
    key: String,
    title: String,
    original_title: Option<String>,
    year: Option<i32>,
    idx: Option<i32>,
    disc: Option<i32>,
    duration_ms: Option<i64>,
    path: Option<String>,
    release_date: Option<String>,
    genres: Option<String>,
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
}

impl NewItem {
    fn new(kind: &'static str, key: String, title: String) -> Self {
        Self {
            parent_id: None,
            grandparent_id: None,
            kind,
            key,
            title,
            original_title: None,
            year: None,
            idx: None,
            disc: None,
            duration_ms: None,
            path: None,
            release_date: None,
            genres: None,
            tmdb_id: None,
            imdb_id: None,
        }
    }
}

impl<'a> Writer<'a> {
    fn new(lib: &LibraryRow, sidecars: &'a HashMap<PathBuf, Vec<String>>) -> Self {
        Self { library_id: lib.id, kind: lib.kind(), sidecars, containers: HashMap::new() }
    }

    async fn write(
        &mut self,
        conn: &mut SqliteConnection,
        file: &FoundFile,
        outcome: ProbeOutcome,
    ) -> anyhow::Result<()> {
        let (info, probed) = match outcome {
            ProbeOutcome::Ok(info) => (Some(*info), true),
            ProbeOutcome::Unavailable => (None, false),
            ProbeOutcome::Failed => (None, true),
        };
        let duration = info.as_ref().and_then(|i| i.duration_ms);
        let item_id = match self.kind {
            LibraryKind::Movies => self.write_movie(conn, file, duration).await?,
            LibraryKind::Shows => self.write_episode(conn, file, duration).await?,
            LibraryKind::Music => self.write_track(conn, file, info.as_ref(), duration).await?,
        };
        let subs =
            if self.kind == LibraryKind::Music { Vec::new() } else { external_subtitles(&file.path, self.sidecars) };
        upsert_file(conn, self.library_id, item_id, file, info.as_ref(), &subs, probed).await
    }

    async fn container(&mut self, conn: &mut SqliteConnection, item: NewItem) -> anyhow::Result<i64> {
        if let Some(id) = self.containers.get(&item.key) {
            return Ok(*id);
        }
        let key = item.key.clone();
        let id = upsert_item(conn, self.library_id, &item).await?;
        self.containers.insert(key, id);
        Ok(id)
    }

    async fn write_movie(
        &mut self,
        conn: &mut SqliteConnection,
        file: &FoundFile,
        duration: Option<i64>,
    ) -> anyhow::Result<i64> {
        let name = parse_movie(&file.path, &file.root);
        let key = match name.tmdb_id {
            Some(id) => format!("movie:tmdb:{id}"),
            None => format!("movie:{}:{}", normalize_key(&name.title), name.year.unwrap_or(0)),
        };
        let mut item = NewItem::new("movie", key, name.title);
        item.year = name.year;
        item.duration_ms = duration;
        item.tmdb_id = name.tmdb_id;
        item.imdb_id = name.imdb_id;
        item.path = file.path.parent().filter(|p| *p != file.root).map(path_str);
        Ok(upsert_item(conn, self.library_id, &item).await?)
    }

    async fn write_episode(
        &mut self,
        conn: &mut SqliteConnection,
        file: &FoundFile,
        duration: Option<i64>,
    ) -> anyhow::Result<i64> {
        let name = parse_episode(&file.path, &file.root);
        let show_key = match name.show_tmdb_id {
            Some(id) => format!("show:tmdb:{id}"),
            None => format!("show:{}:{}", normalize_key(&name.show), name.show_year.unwrap_or(0)),
        };
        let mut show = NewItem::new("show", show_key, name.show.clone());
        show.year = name.show_year;
        show.tmdb_id = name.show_tmdb_id;
        show.path = name.show_dir.as_deref().map(path_str);
        let show_id = self.container(conn, show).await?;

        let season_title = if name.season == 0 { "Specials".to_owned() } else { format!("Season {}", name.season) };
        let mut season = NewItem::new("season", format!("season:{show_id}:{}", name.season), season_title);
        season.parent_id = Some(show_id);
        season.idx = Some(name.season);
        season.path = name.season_dir.as_deref().map(path_str);
        let season_id = self.container(conn, season).await?;

        let title = name.title.clone().unwrap_or_else(|| match name.episode_end {
            Some(end) => format!("Episodes {}-{end}", name.episode),
            None => format!("Episode {}", name.episode),
        });
        let mut episode = NewItem::new("episode", format!("episode:{season_id}:{}", name.episode), title);
        episode.parent_id = Some(season_id);
        episode.grandparent_id = Some(show_id);
        episode.idx = Some(name.episode);
        episode.duration_ms = duration;
        episode.release_date = name.air_date.clone();
        episode.year = name.air_date.as_deref().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok());
        Ok(upsert_item(conn, self.library_id, &episode).await?)
    }

    async fn write_track(
        &mut self,
        conn: &mut SqliteConnection,
        file: &FoundFile,
        info: Option<&MediaInfo>,
        duration: Option<i64>,
    ) -> anyhow::Result<i64> {
        let empty = Default::default();
        let tags = info.map(|i| &i.tags).unwrap_or(&empty);
        let name = parse_track(&file.path, &file.root, tags);

        let mut artist =
            NewItem::new("artist", format!("artist:{}", normalize_key(&name.album_artist)), name.album_artist.clone());
        artist.path = name.artist_dir.as_deref().map(path_str);
        let artist_id = self.container(conn, artist).await?;

        let genres = name.genre.as_ref().map(|g| serde_json::json!([g]).to_string());
        let mut album =
            NewItem::new("album", format!("album:{artist_id}:{}", normalize_key(&name.album)), name.album.clone());
        album.parent_id = Some(artist_id);
        album.year = name.year;
        album.path = Some(path_str(&name.album_dir));
        album.genres = genres.clone();
        let album_id = self.container(conn, album).await?;

        let mut track = NewItem::new("track", format!("track:{}", path_str(&file.path)), name.title.clone());
        track.parent_id = Some(album_id);
        track.grandparent_id = Some(artist_id);
        track.idx = name.track;
        track.disc = name.disc;
        track.year = name.year;
        track.duration_ms = duration;
        track.genres = genres;
        track.original_title = (name.artist != name.album_artist).then(|| name.artist.clone());
        Ok(upsert_item(conn, self.library_id, &track).await?)
    }
}

async fn upsert_item(conn: &mut SqliteConnection, library_id: i64, item: &NewItem) -> sqlx::Result<i64> {
    let now = now();
    sqlx::query_scalar(
        "INSERT INTO items (library_id, parent_id, grandparent_id, kind, key, title, sort_title, original_title, year, idx, disc,
                            duration_ms, path, release_date, genres, tmdb_id, imdb_id, added_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(library_id, key) DO UPDATE SET
            parent_id = excluded.parent_id,
            grandparent_id = excluded.grandparent_id,
            title = CASE WHEN items.metadata_at IS NULL AND items.metadata_locked = 0 THEN excluded.title ELSE items.title END,
            sort_title = CASE WHEN items.metadata_at IS NULL AND items.metadata_locked = 0 THEN excluded.sort_title ELSE items.sort_title END,
            original_title = CASE WHEN items.metadata_at IS NULL AND items.metadata_locked = 0 THEN excluded.original_title ELSE items.original_title END,
            year = CASE WHEN items.metadata_at IS NULL AND items.metadata_locked = 0 THEN COALESCE(excluded.year, items.year) ELSE items.year END,
            idx = excluded.idx,
            disc = excluded.disc,
            duration_ms = COALESCE(excluded.duration_ms, items.duration_ms),
            path = COALESCE(excluded.path, items.path),
            release_date = COALESCE(excluded.release_date, items.release_date),
            genres = COALESCE(excluded.genres, items.genres),
            tmdb_id = COALESCE(items.tmdb_id, excluded.tmdb_id),
            imdb_id = COALESCE(items.imdb_id, excluded.imdb_id),
            updated_at = excluded.updated_at
         RETURNING id",
    )
    .bind(library_id)
    .bind(item.parent_id)
    .bind(item.grandparent_id)
    .bind(item.kind)
    .bind(&item.key)
    .bind(&item.title)
    .bind(sort_title(&item.title))
    .bind(&item.original_title)
    .bind(item.year)
    .bind(item.idx)
    .bind(item.disc)
    .bind(item.duration_ms)
    .bind(&item.path)
    .bind(&item.release_date)
    .bind(&item.genres)
    .bind(item.tmdb_id)
    .bind(&item.imdb_id)
    .bind(now)
    .bind(now)
    .fetch_one(conn)
    .await
}

async fn upsert_file(
    conn: &mut SqliteConnection,
    library_id: i64,
    item_id: i64,
    file: &FoundFile,
    info: Option<&MediaInfo>,
    subs: &[ExternalSub],
    probed: bool,
) -> anyhow::Result<()> {
    let video = info.and_then(|i| i.video());
    let audio = info.and_then(|i| i.default_audio());
    let container = info
        .and_then(|i| i.container.clone())
        .or_else(|| file.path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()));
    let now = now();
    sqlx::query(
        "INSERT INTO media_files (item_id, library_id, path, size, mtime, container, duration_ms, bitrate, width, height,
                                  video_codec, audio_codec, media_info, external_subs, probed, added_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(path) DO UPDATE SET
            item_id = excluded.item_id, library_id = excluded.library_id, size = excluded.size, mtime = excluded.mtime,
            container = excluded.container, duration_ms = excluded.duration_ms, bitrate = excluded.bitrate,
            width = excluded.width, height = excluded.height, video_codec = excluded.video_codec,
            audio_codec = excluded.audio_codec, media_info = excluded.media_info, external_subs = excluded.external_subs,
            probed = excluded.probed, updated_at = excluded.updated_at",
    )
    .bind(item_id)
    .bind(library_id)
    .bind(path_str(&file.path))
    .bind(file.size)
    .bind(file.mtime)
    .bind(container)
    .bind(info.and_then(|i| i.duration_ms))
    .bind(info.and_then(|i| i.bitrate))
    .bind(video.and_then(|v| v.width))
    .bind(video.and_then(|v| v.height))
    .bind(video.map(|v| v.codec.clone()))
    .bind(audio.map(|a| a.codec.clone()))
    .bind(info.map(serde_json::to_string).transpose()?)
    .bind(serde_json::to_string(subs)?)
    .bind(probed)
    .bind(now)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(())
}

async fn cleanup_orphans(conn: &mut SqliteConnection, library_id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM items WHERE library_id = ? AND kind IN ('movie', 'episode', 'track')
         AND NOT EXISTS (SELECT 1 FROM media_files f WHERE f.item_id = items.id)",
    )
    .bind(library_id)
    .execute(&mut *conn)
    .await?;
    for kinds in ["('season', 'album')", "('show', 'artist')"] {
        sqlx::query(&format!(
            "DELETE FROM items WHERE library_id = ? AND kind IN {kinds}
             AND NOT EXISTS (SELECT 1 FROM items c WHERE c.parent_id = items.id)"
        ))
        .bind(library_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn update_counts(conn: &mut SqliteConnection, library_id: i64) -> sqlx::Result<()> {
    let statements = [
        "UPDATE items SET
            leaf_count = (SELECT COUNT(*) FROM items e WHERE e.grandparent_id = items.id AND e.kind = 'episode'),
            child_count = (SELECT COUNT(*) FROM items s WHERE s.parent_id = items.id)
         WHERE library_id = ? AND kind = 'show'",
        "UPDATE items SET leaf_count = (SELECT COUNT(*) FROM items e WHERE e.parent_id = items.id)
         WHERE library_id = ? AND kind = 'season'",
        "UPDATE items SET
            leaf_count = (SELECT COUNT(*) FROM items t WHERE t.grandparent_id = items.id AND t.kind = 'track'),
            child_count = (SELECT COUNT(*) FROM items a WHERE a.parent_id = items.id)
         WHERE library_id = ? AND kind = 'artist'",
        "UPDATE items SET
            leaf_count = (SELECT COUNT(*) FROM items t WHERE t.parent_id = items.id),
            duration_ms = (SELECT SUM(t.duration_ms) FROM items t WHERE t.parent_id = items.id)
         WHERE library_id = ? AND kind = 'album'",
    ];
    for sql in statements {
        sqlx::query(sql).bind(library_id).execute(&mut *conn).await?;
    }
    Ok(())
}

/// Sidecar subtitles can appear without the video changing.
async fn refresh_external_subs(
    conn: &mut SqliteConnection,
    files: &[(i64, PathBuf)],
    sidecars: &HashMap<PathBuf, Vec<String>>,
) -> sqlx::Result<()> {
    for (id, path) in files {
        if path.parent().is_none_or(|dir| !sidecars.contains_key(dir) && !has_subs_dir(dir, sidecars)) {
            continue;
        }
        let subs = serde_json::to_string(&external_subtitles(path, sidecars)).unwrap_or_else(|_| "[]".into());
        sqlx::query("UPDATE media_files SET external_subs = ? WHERE id = ? AND external_subs != ?")
            .bind(&subs)
            .bind(id)
            .bind(&subs)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

fn has_subs_dir(dir: &Path, sidecars: &HashMap<PathBuf, Vec<String>>) -> bool {
    sidecars.contains_key(&dir.join("Subs")) || sidecars.contains_key(&dir.join("subs"))
}

#[derive(sqlx::FromRow)]
struct ArtworkRow {
    id: i64,
    kind: String,
    path: Option<String>,
    idx: Option<i64>,
    poster: Option<String>,
    backdrop: Option<String>,
    thumb: Option<String>,
    parent_path: Option<String>,
    file_path: Option<String>,
}

/// Pick up local artwork (poster.jpg, fanart.jpg, season01-poster.jpg, …).
/// Local files win over downloaded artwork; vanished local files are cleared.
async fn refresh_artwork(
    conn: &mut SqliteConnection,
    lib: &LibraryRow,
    sidecars: &HashMap<PathBuf, Vec<String>>,
    metadata_dir: &Path,
) -> sqlx::Result<()> {
    let rows: Vec<ArtworkRow> = sqlx::query_as(
        "SELECT i.id, i.kind, i.path, i.idx, i.poster, i.backdrop, i.thumb, p.path AS parent_path,
                (SELECT f.path FROM media_files f WHERE f.item_id = i.id ORDER BY f.id LIMIT 1) AS file_path
         FROM items i LEFT JOIN items p ON p.id = i.parent_id
         WHERE i.library_id = ? AND i.kind IN ('movie', 'show', 'season', 'episode', 'artist', 'album')",
    )
    .bind(lib.id)
    .fetch_all(&mut *conn)
    .await?;

    let lookup = |dir: &str, bases: &[String]| -> Option<String> {
        let dir = Path::new(dir);
        sidecars.get(dir).and_then(|files| find_image(files, bases)).map(|name| path_str(&dir.join(name)))
    };
    let strings = |names: &[&str]| names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let now = now();

    for row in rows {
        let file_stem_bases = |suffixes: &[&str]| -> Vec<String> {
            let stem = row
                .file_path
                .as_deref()
                .and_then(|p| Path::new(p).file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            suffixes.iter().map(|s| format!("{stem}{s}")).collect()
        };
        let file_dir = row.file_path.as_deref().and_then(|p| Path::new(p).parent()).map(path_str);
        let (poster, backdrop, thumb) = match row.kind.as_str() {
            "movie" => {
                let dir = file_dir.clone().unwrap_or_default();
                let poster = lookup(&dir, &file_stem_bases(&["-poster", "-cover", ""])).or_else(|| {
                    row.path
                        .as_deref()
                        .and_then(|p| lookup(p, &strings(&["poster", "folder", "cover", "movie", "default"])))
                });
                let backdrop = lookup(&dir, &file_stem_bases(&["-fanart", "-backdrop"])).or_else(|| {
                    row.path.as_deref().and_then(|p| lookup(p, &strings(&["fanart", "backdrop", "background", "art"])))
                });
                (poster, backdrop, None)
            }
            "show" | "artist" => {
                let path = row.path.as_deref().unwrap_or_default();
                let names: &[&str] = if row.kind == "show" {
                    &["poster", "folder", "show", "cover"]
                } else {
                    &["artist", "folder", "poster", "cover"]
                };
                (
                    lookup(path, &strings(names)),
                    lookup(path, &strings(&["fanart", "backdrop", "background", "art"])),
                    None,
                )
            }
            "season" => {
                let n = row.idx.unwrap_or(1);
                let in_show = row.parent_path.as_deref().and_then(|show| {
                    let names = if n == 0 {
                        vec!["season-specials-poster".to_owned(), "season00-poster".to_owned()]
                    } else {
                        vec![format!("season{n:02}-poster"), format!("season{n}-poster")]
                    };
                    lookup(show, &names)
                });
                let in_dir = row.path.as_deref().and_then(|p| lookup(p, &strings(&["poster", "folder", "cover"])));
                (in_dir.or(in_show), None, None)
            }
            "album" => {
                let path = row.path.as_deref().unwrap_or_default();
                (lookup(path, &strings(&["cover", "folder", "front", "album", "poster"])), None, None)
            }
            "episode" => {
                let dir = file_dir.clone().unwrap_or_default();
                (None, None, lookup(&dir, &file_stem_bases(&["-thumb", ""])))
            }
            _ => (None, None, None),
        };

        let decide = |current: &Option<String>, local: Option<String>| -> Option<Option<String>> {
            match (current, local) {
                (Some(cur), Some(new)) if *cur == new => None,
                (_, Some(new)) => Some(Some(new)),
                // Clear local artwork that vanished (downloaded/generated art lives in metadata_dir).
                (Some(cur), None) if !Path::new(cur).starts_with(metadata_dir) && !Path::new(cur).exists() => {
                    Some(None)
                }
                _ => None,
            }
        };
        let poster = decide(&row.poster, poster);
        let backdrop = decide(&row.backdrop, backdrop);
        let thumb = decide(&row.thumb, thumb);
        if poster.is_none() && backdrop.is_none() && thumb.is_none() {
            continue;
        }
        sqlx::query(
            "UPDATE items SET
                poster = CASE WHEN ? THEN ? ELSE poster END,
                backdrop = CASE WHEN ? THEN ? ELSE backdrop END,
                thumb = CASE WHEN ? THEN ? ELSE thumb END,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(poster.is_some())
        .bind(poster.flatten())
        .bind(backdrop.is_some())
        .bind(backdrop.flatten())
        .bind(thumb.is_some())
        .bind(thumb.flatten())
        .bind(now)
        .bind(row.id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Is this file a good candidate for remuxing (copying H.264 into HLS)?
pub fn remux_candidate(container: Option<&str>, streams: &[pley_transcoder_api::MediaStream]) -> bool {
    let video = streams.iter().find(|s| s.kind == StreamKind::Video);
    video.is_some_and(|v| v.codec == "h264" && v.bit_depth.unwrap_or(8) <= 8 && !v.dynamic_range().is_hdr())
        && !matches!(container, Some("mp4" | "m4v" | "mov" | "webm"))
}
