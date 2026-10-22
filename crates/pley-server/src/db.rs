//! SQLite access. Writes go through a single-connection pool so they never
//! contend for the write lock; reads use a separate read-only pool (WAL mode
//! lets them run concurrently with the writer).

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};

#[derive(Clone)]
pub struct Db {
    pub read: SqlitePool,
    pub write: SqlitePool,
}

impl Db {
    pub async fn open(path: &Path) -> anyhow::Result<Self> {
        let base = SqliteConnectOptions::from_str("sqlite://")?
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(30))
            .foreign_keys(true)
            .pragma("cache_size", "-32768")
            .pragma("mmap_size", "268435456")
            .pragma("temp_store", "memory");

        let write = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(60))
            .connect_with(base.clone())
            .await?;
        sqlx::migrate!("./migrations").run(&write).await?;

        let readers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(4, 16) as u32;
        let read = SqlitePoolOptions::new()
            .max_connections(readers)
            .acquire_timeout(Duration::from_secs(30))
            .connect_with(base.read_only(true))
            .await?;
        Ok(Self { read, write })
    }

    /// Refresh query planner statistics (cheap; run after large scans).
    pub async fn optimize(&self) {
        let _ = sqlx::query("PRAGMA optimize").execute(&self.write).await;
    }

    pub async fn close(&self) {
        self.read.close().await;
        self.write.close().await;
    }
}
