//! Automatic rescans: debounced filesystem notifications plus a periodic timer
//! (for network shares where notifications are unreliable). A library whose
//! last scan failed is tried again after a growing delay
//! ([`super::scanner::ScanFailures`]).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::list_libraries;
use super::scanner::Scanner;
use crate::db::Db;
use crate::settings::SettingsStore;

const QUIET_PERIOD: Duration = Duration::from_secs(10);

pub struct LibraryWatcher {
    db: Db,
    scanner: Arc<Scanner>,
    settings: Arc<SettingsStore>,
    watcher: Mutex<Option<RecommendedWatcher>>,
    roots: Arc<Mutex<Vec<(PathBuf, i64)>>>,
    tx: mpsc::UnboundedSender<PathBuf>,
}

impl LibraryWatcher {
    pub fn new(db: Db, scanner: Arc<Scanner>, settings: Arc<SettingsStore>) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let watcher = Arc::new(Self { db, scanner, settings, watcher: Mutex::new(None), roots: Arc::default(), tx });
        tokio::spawn(watcher.clone().debounce(rx));
        tokio::spawn(watcher.clone().periodic());
        watcher
    }

    /// Re-read the library list and (re)arm filesystem watches.
    pub async fn reload(&self) {
        let libraries = match list_libraries(&self.db).await {
            Ok(libs) => libs,
            Err(err) => {
                tracing::warn!("cannot load libraries for watching: {err}");
                return;
            }
        };
        let roots: Vec<(PathBuf, i64)> =
            libraries.iter().flat_map(|l| l.paths().into_iter().map(move |p| (p, l.id))).collect();
        *self.roots.lock() = roots.clone();

        if !self.settings.get().watch_libraries {
            *self.watcher.lock() = None;
            return;
        }
        let tx = self.tx.clone();
        let created = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, notify::EventKind::Access(_)) {
                    return;
                }
                for path in event.paths {
                    let _ = tx.send(path);
                }
            }
        });
        let mut watcher = match created {
            Ok(w) => w,
            Err(err) => {
                tracing::warn!("file watching unavailable: {err}");
                return;
            }
        };
        for (root, _) in &roots {
            if let Err(err) = watcher.watch(root, RecursiveMode::Recursive) {
                tracing::warn!("cannot watch {}: {err}", root.display());
            }
        }
        *self.watcher.lock() = Some(watcher);
    }

    async fn debounce(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<PathBuf>) {
        let mut pending: HashMap<i64, Instant> = HashMap::new();
        loop {
            tokio::select! {
                path = rx.recv() => {
                    let Some(path) = path else { break };
                    let library = self.roots.lock().iter().find(|(root, _)| path.starts_with(root)).map(|(_, id)| *id);
                    if let Some(id) = library {
                        pending.insert(id, Instant::now());
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
            let due: Vec<i64> =
                pending.iter().filter(|(_, at)| at.elapsed() >= QUIET_PERIOD).map(|(id, _)| *id).collect();
            let interval = self.settings.get().scan_interval_minutes as i64 * 60;
            for id in due {
                // Try again once the running scan finished, or once a library
                // whose last scan failed has waited.
                if self.scanner.is_scanning(id) || self.scanner.backing_off(id, interval) {
                    pending.insert(id, Instant::now());
                    continue;
                }
                pending.remove(&id);
                tracing::info!("changes detected in library {id}, rescanning");
                self.scanner.enqueue(id, false);
            }
        }
    }

    async fn periodic(self: Arc<Self>) {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            let interval = self.settings.get().scan_interval_minutes as i64 * 60;
            if interval == 0 {
                continue;
            }
            let Ok(libraries) = list_libraries(&self.db).await else { continue };
            for lib in libraries {
                // A failed scan leaves `scanned_at` as it was: the scanner
                // spaces out the next tries.
                if self.scanner.due(&lib, interval) && !self.scanner.is_scanning(lib.id) {
                    self.scanner.enqueue(lib.id, false);
                }
            }
        }
    }
}
