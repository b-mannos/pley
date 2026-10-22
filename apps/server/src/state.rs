use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::auth::AuthService;
use crate::config::Config;
use crate::db::Db;
use crate::events::EventBus;
use crate::images::Images;
use crate::library::scanner::Scanner;
use crate::library::watcher::LibraryWatcher;
use crate::markers::MarkerService;
use crate::trickplay::TrickplayService;
use crate::metadata::MetadataAgent;
use crate::playback::Sessions;
use crate::settings::SettingsStore;
use crate::transcoder::Transcoder;

/// Shared application state handed to every request handler.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

pub struct Inner {
    pub config: Config,
    pub db: Db,
    pub auth: AuthService,
    pub settings: Arc<SettingsStore>,
    pub events: EventBus,
    pub transcoder: Arc<Transcoder>,
    pub metadata: Arc<MetadataAgent>,
    pub scanner: Arc<Scanner>,
    pub watcher: Arc<LibraryWatcher>,
    pub images: Images,
    pub sessions: Arc<Sessions>,
    pub markers: Arc<MarkerService>,
    pub trickplay: Arc<TrickplayService>,
    pub started_at: Instant,
}

impl Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        &self.inner
    }
}

impl AppState {
    pub async fn init(config: Config) -> anyhow::Result<Self> {
        for dir in [config.data_dir.clone(), config.cache_dir(), config.metadata_dir(), config.transcode_dir.clone()] {
            std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!("cannot create {}: {e}", dir.display()))?;
        }
        let db = Db::open(&config.db_path()).await?;
        crate::ratings::backfill(&db).await?;
        let settings = Arc::new(SettingsStore::load(&db).await?);
        let events = EventBus::new();
        let transcoder = Transcoder::new(&config);
        let metadata = MetadataAgent::new(db.clone(), settings.clone(), config.metadata_dir());
        let scanner = Scanner::new(db.clone(), transcoder.clone(), events.clone(), metadata.clone(), &config);
        let watcher = LibraryWatcher::new(db.clone(), scanner.clone(), settings.clone());
        let images = Images::new(config.cache_dir().join("images"));
        let sessions = Arc::new(Sessions::default());
        let markers = MarkerService::new(
            db.clone(),
            transcoder.clone(),
            settings.clone(),
            sessions.clone(),
            events.clone(),
            config.cache_dir().join("fingerprints"),
        );
        let trickplay = TrickplayService::new(
            db.clone(),
            transcoder.clone(),
            settings.clone(),
            sessions.clone(),
            config.cache_dir().join("trickplay"),
        );
        Ok(Self {
            inner: Arc::new(Inner {
                auth: AuthService::new(db.clone()),
                sessions,
                markers,
                trickplay,
                started_at: Instant::now(),
                config,
                db,
                settings,
                events,
                transcoder,
                metadata,
                scanner,
                watcher,
                images,
            }),
        })
    }

    pub fn start_background_tasks(&self) {
        self.transcoder.start();
        self.sessions.spawn_reaper(self.transcoder.clone(), self.events.clone());
        self.markers.start();
        let watcher = self.watcher.clone();
        tokio::spawn(async move { watcher.reload().await });
        let state = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(3600));
            loop {
                tick.tick().await;
                state.auth.purge_expired().await;
            }
        });
    }

    pub async fn shutdown(&self) {
        // External transcoders keep running: delete our jobs there. A managed
        // one stops all of its jobs itself when it shuts down.
        if !self.transcoder.is_managed() {
            let jobs: Vec<String> = self.sessions.list().iter().filter_map(|s| s.job_id()).collect();
            let deletes = futures::future::join_all(jobs.iter().map(|job| self.transcoder.delete_job(job)));
            let _ = tokio::time::timeout(Duration::from_secs(5), deletes).await;
        }
        self.transcoder.shutdown().await;
        self.db.close().await;
    }
}
