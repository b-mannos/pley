//! Seek-bar previews ("trickplay"): sprite sheets made by the transcoder,
//! copied into the cache once and served from there. They are made in the
//! background, one file at a time, when a title is opened, and wait while the
//! server is busy transcoding.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use parking_lot::Mutex;
use pley_transcoder_api::TrickplayInfo;
use tokio::sync::mpsc;

use crate::catalog::{self, FileRow};
use crate::db::Db;
use crate::playback::Sessions;
use crate::settings::SettingsStore;
use crate::transcoder::Transcoder;

/// Previews wait while this many transcodes run.
const BUSY_TRANSCODES: usize = 2;

pub struct TrickplayService {
    db: Db,
    transcoder: Arc<Transcoder>,
    settings: Arc<SettingsStore>,
    sessions: Arc<Sessions>,
    dir: PathBuf,
    queue: mpsc::UnboundedSender<i64>,
    pending: Mutex<HashSet<i64>>,
    /// File versions that could not be done; retried after a restart.
    failed: Mutex<HashMap<i64, String>>,
}

/// Cache folder name of a file version.
fn key(file: &FileRow) -> String {
    format!("{}-{}-{}", file.id, file.size, file.mtime)
}

impl TrickplayService {
    pub fn new(
        db: Db,
        transcoder: Arc<Transcoder>,
        settings: Arc<SettingsStore>,
        sessions: Arc<Sessions>,
        dir: PathBuf,
    ) -> Arc<Self> {
        let (queue, rx) = mpsc::unbounded_channel();
        let service = Arc::new(Self {
            db,
            transcoder,
            settings,
            sessions,
            dir,
            queue,
            pending: Mutex::default(),
            failed: Mutex::default(),
        });
        tokio::spawn(service.clone().worker(rx));
        service
    }

    /// The previews of this file version, if they exist.
    pub fn index(&self, file: &FileRow) -> Option<TrickplayInfo> {
        let bytes = std::fs::read(self.dir.join(key(file)).join("index.json")).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn sheet(&self, file: &FileRow, n: u32) -> Option<PathBuf> {
        Some(self.dir.join(key(file)).join(format!("{n}.jpg"))).filter(|p| p.is_file())
    }

    /// Queue the file unless its previews exist, are queued or failed.
    pub fn request(&self, file: &FileRow) {
        if !self.settings.get().seek_previews || file.video_codec.is_none() || self.index(file).is_some() {
            return;
        }
        if self.failed.lock().get(&file.id) == Some(&key(file)) {
            return;
        }
        if self.pending.lock().insert(file.id) {
            let _ = self.queue.send(file.id);
        }
    }

    async fn worker(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<i64>) {
        if let Err(err) = self.prune().await {
            tracing::debug!("could not prune seek previews: {err:#}");
        }
        while let Some(file_id) = rx.recv().await {
            while self.sessions.active_transcodes() >= BUSY_TRANSCODES {
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
            if let Err(err) = self.make(file_id).await {
                tracing::warn!("seek previews for file {file_id} failed: {err:#}");
            }
            self.pending.lock().remove(&file_id);
        }
    }

    async fn make(&self, file_id: i64) -> anyhow::Result<()> {
        let Some(file) = catalog::file(&self.db, file_id).await? else { return Ok(()) };
        if self.index(&file).is_some() {
            return Ok(());
        }
        if !self.transcoder.is_available() && !self.transcoder.wait_ready(Duration::from_secs(60)).await {
            anyhow::bail!("the transcoder is not available");
        }
        let info = match self.transcoder.trickplay(&file.path).await {
            Ok(info) => info,
            Err(err) => {
                self.failed.lock().insert(file.id, key(&file));
                return Err(err.into());
            }
        };
        let target = self.dir.join(key(&file));
        let part = self.dir.join(format!("{}.part", key(&file)));
        let _ = tokio::fs::remove_dir_all(&part).await;
        tokio::fs::create_dir_all(&part).await.with_context(|| format!("creating {}", part.display()))?;
        for n in 0..info.sheets {
            let bytes = self.transcoder.trickplay_sheet(&info.id, n).await?;
            tokio::fs::write(part.join(format!("{n}.jpg")), bytes).await?;
        }
        tokio::fs::write(part.join("index.json"), serde_json::to_vec(&info)?).await?;
        let _ = tokio::fs::remove_dir_all(&target).await;
        tokio::fs::rename(&part, &target).await?;
        // Previews of older versions of the file are useless now.
        remove_other_versions(&self.dir, file.id, &key(&file)).await;
        Ok(())
    }

    /// Delete previews of files that left the library.
    async fn prune(&self) -> anyhow::Result<()> {
        let ids: HashSet<i64> =
            sqlx::query_scalar("SELECT id FROM media_files").fetch_all(&self.db.read).await?.into_iter().collect();
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else { return Ok(()) };
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name.split('-').next().and_then(|i| i.parse::<i64>().ok());
            if id.is_none_or(|id| !ids.contains(&id)) || name.ends_with(".part") {
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
        }
        Ok(())
    }
}

async fn remove_other_versions(dir: &Path, file_id: i64, keep: &str) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else { return };
    let prefix = format!("{file_id}-");
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name != keep {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}
