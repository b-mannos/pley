//! SQLite access. Writes go through a single-connection pool so they never
//! contend for the write lock; reads use a separate read-only pool (WAL mode
//! lets them run concurrently with the writer).
//!
//! Whoever holds the writer holds up every other write (progress reports,
//! logins, settings), and those fail after the 60 s acquire timeout: work out
//! what to write first (reading through `read`, computing in
//! `spawn_blocking`), then write it in short transactions, as the scanner
//! does.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};

/// The schema, brought up to date when the database is opened.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

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
        backup_before_upgrade(&write, path).await?;
        MIGRATOR.run(&write).await?;

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

/// Copies an existing database to `backups/` before migrations change it:
/// nothing else could undo a migration that went wrong. pley doesn't upgrade
/// a database it could not copy.
async fn backup_before_upgrade(write: &SqlitePool, path: &Path) -> anyhow::Result<()> {
    let migrated: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(write)
    .await?;
    if !migrated {
        // A new database: nothing to keep.
        return Ok(());
    }
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success").fetch_all(write).await?;
    let pending = MIGRATOR.iter().any(|m| m.migration_type.is_up_migration() && !applied.contains(&m.version));
    if applied.is_empty() || !pending {
        return Ok(());
    }
    let dir = path.parent().unwrap_or(Path::new("")).join("backups");
    // Logged by write_checked; the name says why it was made.
    crate::backup::write_checked(write, &dir, crate::backup::BEFORE_UPGRADE)
        .await
        .context("could not back up the database before upgrading it")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::ConnectOptions;

    use super::*;

    async fn versions(path: &Path) -> Vec<i64> {
        let mut conn = SqliteConnectOptions::new().filename(path).read_only(true).connect().await.unwrap();
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&mut conn).await.unwrap()
    }

    #[tokio::test]
    async fn databases_are_backed_up_before_an_upgrade_and_only_then() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pley.db");
        let backups = dir.path().join("backups");
        // A database made by an older version: every migration but the last.
        let older = dir.path().join("older");
        std::fs::create_dir(&older).unwrap();
        let mut files: Vec<_> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        for file in &files[..files.len() - 1] {
            std::fs::copy(file, older.join(file.file_name().unwrap())).unwrap();
        }
        let options = SqliteConnectOptions::new().filename(&path).create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await.unwrap();
        Migrator::new(older.as_path()).await.unwrap().run(&pool).await.unwrap();
        sqlx::query("INSERT INTO settings (key, value) VALUES ('probe', 'before')").execute(&pool).await.unwrap();
        pool.close().await;

        let db = Db::open(&path).await.unwrap();
        let latest: Vec<i64> = MIGRATOR.iter().map(|m| m.version).collect();
        assert_eq!(versions(&path).await, latest, "upgraded");
        let copies: Vec<_> = std::fs::read_dir(&backups).unwrap().flatten().map(|e| e.path()).collect();
        assert_eq!(copies.len(), 1, "{copies:?}");
        assert!(copies[0].to_string_lossy().ends_with("-before-upgrade.db"), "{copies:?}");
        assert_eq!(versions(&copies[0]).await, latest[..latest.len() - 1], "the copy is the old database");
        let mut copy = SqliteConnectOptions::new().filename(&copies[0]).read_only(true).connect().await.unwrap();
        let probe: String =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'probe'").fetch_one(&mut copy).await.unwrap();
        assert_eq!(probe, "before");
        db.close().await;

        // Up to date: no copy. A new database: none either.
        Db::open(&path).await.unwrap().close().await;
        Db::open(&dir.path().join("new.db")).await.unwrap().close().await;
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);
    }
}
