//! Where pley keeps the files it downloads or makes (docs/STORAGE.md).
//!
//! - **Blobs** (`<data>/metadata/blobs/ab/abcd….jpg`): pictures that are
//!   metadata — downloaded artwork, uploads, embedded album covers, people's
//!   photos. Stored once, named by the SHA-256 of their content, and
//!   referenced from the database as `blob:<name>`: the same picture is never
//!   stored twice and moving the data folder breaks nothing. Downloads
//!   remember their URL ([`Store::remote`]), so refreshes don't fetch them
//!   again.
//! - **Media folders** (`<data>/media/ab/<fingerprint>/`): files generated
//!   from a media file — frame grabs, seek-bar previews, chapter images. Keyed
//!   by the file's fingerprint (its OpenSubtitles hash), so they follow the
//!   file when it is moved or renamed.
//! - Local artwork next to the media (`poster.jpg`, …) stays where it is and
//!   is referenced by its path.
//!
//! [`Store::clean`] removes whatever nothing references any more.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::db::Db;
use crate::util::{now, path_str};

const BLOB_PREFIX: &str = "blob:";
/// Largest picture accepted from the internet.
const MAX_REMOTE_BYTES: usize = 25 * 1024 * 1024;
/// Files younger than this are never cleaned up: their database row may not
/// be committed yet.
const GRACE: Duration = Duration::from_secs(3600);
/// Bytes hashed at each end of a media file for its fingerprint.
const FINGERPRINT_CHUNK: u64 = 64 * 1024;

/// Hosts pictures may be downloaded from (metadata providers' image servers).
const IMAGE_HOSTS: &[&str] = &["image.tmdb.org", "static.tvmaze.com", "artworks.thetvdb.com", "coverartarchive.org"];

/// Whether a stored reference is a blob (as opposed to a local file path).
pub fn is_blob(reference: &str) -> bool {
    reference.starts_with(BLOB_PREFIX)
}

/// `<64 hex>.<ext>`: nothing else is ever turned into a path.
fn valid_blob_name(name: &str) -> bool {
    let Some((hash, ext)) = name.split_once('.') else { return false };
    hash.len() == 64
        && hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && matches!(ext, "jpg" | "png" | "webp" | "gif")
}

/// Extension for picture bytes, `None` when they are not a picture pley serves.
pub fn picture_ext(bytes: &[u8]) -> Option<&'static str> {
    match image::guess_format(bytes).ok()? {
        image::ImageFormat::Jpeg => Some("jpg"),
        image::ImageFormat::Png => Some("png"),
        image::ImageFormat::WebP => Some("webp"),
        image::ImageFormat::Gif => Some("gif"),
        _ => None,
    }
}

/// Whether pley may download a picture from `url`: https on a known image host.
pub fn allowed_image_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    IMAGE_HOSTS.contains(&host)
}

/// The OpenSubtitles hash of a file: its size plus the sum of the 64-bit
/// little-endian words of its first and last 64 KiB, as 16 hex digits.
pub fn fingerprint(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hash = size;
    let mut add = |buf: &[u8]| {
        for word in buf.chunks(8) {
            let mut bytes = [0u8; 8];
            bytes[..word.len()].copy_from_slice(word);
            hash = hash.wrapping_add(u64::from_le_bytes(bytes));
        }
    };
    let chunk = FINGERPRINT_CHUNK.min(size) as usize;
    let mut buf = vec![0u8; chunk];
    file.read_exact(&mut buf)?;
    add(&buf);
    file.seek(SeekFrom::Start(size - chunk as u64))?;
    file.read_exact(&mut buf)?;
    add(&buf);
    Ok(format!("{hash:016x}"))
}

fn valid_fingerprint(fp: &str) -> bool {
    fp.len() == 16 && fp.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Usage {
    pub blobs: u64,
    pub blob_bytes: u64,
    pub media_folders: u64,
    pub media_bytes: u64,
    pub cache_bytes: u64,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct CleanReport {
    pub blobs_removed: u64,
    pub media_folders_removed: u64,
    pub bytes_freed: u64,
}

pub struct Store {
    db: Db,
    blobs: PathBuf,
    media: PathBuf,
    cache: PathBuf,
    http: reqwest::Client,
    cleaning: tokio::sync::Mutex<()>,
}

impl Store {
    pub fn new(db: Db, metadata_dir: &Path, media_dir: &Path, cache_dir: &Path) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("pley/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client");
        Self {
            db,
            blobs: metadata_dir.join("blobs"),
            media: media_dir.to_path_buf(),
            cache: cache_dir.to_path_buf(),
            http,
            cleaning: tokio::sync::Mutex::new(()),
        }
    }

    /// The file behind a reference: a blob in the store, or a local file.
    pub fn path(&self, reference: &str) -> Option<PathBuf> {
        match reference.strip_prefix(BLOB_PREFIX) {
            Some(name) => valid_blob_name(name).then(|| self.blobs.join(&name[..2]).join(name)),
            None => Some(PathBuf::from(reference)),
        }
    }

    /// Whether the file behind a reference is there.
    pub fn exists(&self, reference: &str) -> bool {
        self.path(reference).is_some_and(|p| p.is_file())
    }

    /// Store a picture; returns its reference. Storing the same bytes twice
    /// keeps one copy.
    pub async fn put(&self, bytes: &[u8], ext: &str) -> std::io::Result<String> {
        let hash = hex::encode(Sha256::digest(bytes));
        let name = format!("{hash}.{ext}");
        let dir = self.blobs.join(&hash[..2]);
        let path = dir.join(&name);
        if !path.is_file() {
            tokio::fs::create_dir_all(&dir).await?;
            let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
            tokio::fs::write(&tmp, bytes).await?;
            if let Err(err) = tokio::fs::rename(&tmp, &path).await {
                let _ = tokio::fs::remove_file(&tmp).await;
                // Someone else stored the same picture meanwhile.
                if !path.is_file() {
                    return Err(err);
                }
            }
        }
        Ok(format!("{BLOB_PREFIX}{name}"))
    }

    /// A picture from the internet, downloaded the first time and reused
    /// after that (image URLs of metadata providers change with the picture).
    pub async fn remote(&self, url: &str) -> anyhow::Result<String> {
        let known: Option<String> = sqlx::query_scalar("SELECT blob FROM remote_images WHERE url = ?")
            .bind(url)
            .fetch_optional(&self.db.read)
            .await?;
        if let Some(blob) = known.filter(|b| self.exists(b)) {
            return Ok(blob);
        }
        if !allowed_image_url(url) {
            bail!("pictures are not downloaded from {url}");
        }
        let resp = self.http.get(url).send().await?.error_for_status()?;
        if resp.content_length().is_some_and(|n| n as usize > MAX_REMOTE_BYTES) {
            bail!("{url} is too large");
        }
        let bytes = resp.bytes().await?;
        anyhow::ensure!(bytes.len() <= MAX_REMOTE_BYTES, "{url} is too large");
        let ext = picture_ext(&bytes).with_context(|| format!("{url} is not a picture"))?;
        let blob = self.put(&bytes, ext).await?;
        sqlx::query("INSERT OR REPLACE INTO remote_images (url, blob, fetched_at) VALUES (?, ?, ?)")
            .bind(url)
            .bind(&blob)
            .bind(now())
            .execute(&self.db.write)
            .await?;
        Ok(blob)
    }

    /// Folder of files generated from the media file with this fingerprint.
    pub fn media_dir(&self, fingerprint: &str) -> Option<PathBuf> {
        valid_fingerprint(fingerprint).then(|| self.media.join(&fingerprint[..2]).join(fingerprint))
    }

    /// The fingerprint of a media file, computed (and saved) when missing.
    pub async fn fingerprint_of(&self, file_id: i64, path: &str, known: Option<&str>) -> anyhow::Result<String> {
        if let Some(fp) = known.filter(|f| valid_fingerprint(f)) {
            return Ok(fp.to_owned());
        }
        let owned = PathBuf::from(path);
        let fp = crate::util::blocking(move || Ok(fingerprint(&owned)?)).await?;
        sqlx::query("UPDATE media_files SET fingerprint = ? WHERE id = ?")
            .bind(&fp)
            .bind(file_id)
            .execute(&self.db.write)
            .await?;
        Ok(fp)
    }

    /// Fingerprint media files scanned since the last run, one at a time.
    pub async fn fingerprint_pending(&self) {
        loop {
            let batch: Vec<(i64, String, i64, i64)> = match sqlx::query_as(
                "SELECT id, path, size, mtime FROM media_files WHERE fingerprint IS NULL ORDER BY id LIMIT 200",
            )
            .fetch_all(&self.db.read)
            .await
            {
                Ok(batch) => batch,
                Err(err) => {
                    tracing::warn!("fingerprinting media files: {err}");
                    return;
                }
            };
            if batch.is_empty() {
                return;
            }
            let mut done = 0;
            for (id, path, size, mtime) in batch {
                let owned = PathBuf::from(&path);
                // Unreadable files (offline drives) get an empty marker so the
                // loop moves on; the next scan that sees them changed resets it.
                let fp = crate::util::blocking(move || Ok(fingerprint(&owned)?)).await.unwrap_or_default();
                done += 1;
                let _ = sqlx::query("UPDATE media_files SET fingerprint = ? WHERE id = ? AND size = ? AND mtime = ?")
                    .bind(&fp)
                    .bind(id)
                    .bind(size)
                    .bind(mtime)
                    .execute(&self.db.write)
                    .await;
            }
            tracing::debug!("fingerprinted {done} media files");
        }
    }

    /// Every blob the database still uses.
    async fn referenced_blobs(&self) -> sqlx::Result<HashSet<String>> {
        let queries = [
            "SELECT poster FROM items WHERE poster LIKE 'blob:%'
             UNION SELECT backdrop FROM items WHERE backdrop LIKE 'blob:%'
             UNION SELECT thumb FROM items WHERE thumb LIKE 'blob:%'",
            "SELECT value FROM item_overrides WHERE value LIKE 'blob:%'",
            "SELECT poster FROM collections WHERE poster LIKE 'blob:%'
             UNION SELECT backdrop FROM collections WHERE backdrop LIKE 'blob:%'",
            // People's photos are only known by URL.
            "SELECT r.blob FROM remote_images r JOIN people p ON r.url = p.photo_url",
        ];
        let mut out = HashSet::new();
        for sql in queries {
            let rows: Vec<String> = sqlx::query_scalar(sql).fetch_all(&self.db.read).await?;
            out.extend(rows);
        }
        Ok(out)
    }

    /// Remove blobs and media folders nothing references any more.
    pub async fn clean(&self) -> anyhow::Result<CleanReport> {
        self.clean_older_than(GRACE).await
    }

    /// [`Self::clean`], sparing files younger than `grace`.
    pub async fn clean_older_than(&self, grace: Duration) -> anyhow::Result<CleanReport> {
        let _running = self.cleaning.lock().await;
        let mut report = CleanReport::default();
        let referenced = self.referenced_blobs().await?;
        // Downloads nothing uses any more are forgotten with their pictures.
        let remote: Vec<(String, String)> =
            sqlx::query_as("SELECT url, blob FROM remote_images").fetch_all(&self.db.read).await?;
        for (url, blob) in remote {
            if !referenced.contains(&blob) {
                sqlx::query("DELETE FROM remote_images WHERE url = ?").bind(url).execute(&self.db.write).await?;
            }
        }
        let names: HashSet<String> =
            referenced.iter().filter_map(|r| r.strip_prefix(BLOB_PREFIX)).map(str::to_owned).collect();
        let blobs = self.blobs.clone();
        let (count, bytes) =
            crate::util::blocking(move || Ok(sweep(&blobs, grace, |name| names.contains(name)))).await?;
        report.blobs_removed = count;
        report.bytes_freed += bytes;

        let fingerprints: HashSet<String> =
            sqlx::query_scalar("SELECT DISTINCT fingerprint FROM media_files WHERE fingerprint IS NOT NULL")
                .fetch_all(&self.db.read)
                .await?
                .into_iter()
                .collect();
        let media = self.media.clone();
        let (count, bytes) =
            crate::util::blocking(move || Ok(sweep(&media, grace, |name| fingerprints.contains(name)))).await?;
        report.media_folders_removed = count;
        report.bytes_freed += bytes;
        if report.blobs_removed + report.media_folders_removed > 0 {
            tracing::info!(
                "storage cleaned: {} pictures and {} media folders removed, {} MB freed",
                report.blobs_removed,
                report.media_folders_removed,
                report.bytes_freed / 1_000_000
            );
        }
        Ok(report)
    }

    /// Space used by blobs, media folders and the cache.
    pub async fn usage(&self) -> anyhow::Result<Usage> {
        let (blobs, media, cache) = (self.blobs.clone(), self.media.clone(), self.cache.clone());
        crate::util::blocking(move || {
            let (blob_count, blob_bytes) = tree_size(&blobs);
            let media_folders = std::fs::read_dir(&media)
                .map(|shards| {
                    shards.flatten().map(|s| std::fs::read_dir(s.path()).map(|d| d.count() as u64).unwrap_or(0)).sum()
                })
                .unwrap_or(0);
            Ok(Usage {
                blobs: blob_count,
                blob_bytes,
                media_folders,
                media_bytes: tree_size(&media).1,
                cache_bytes: tree_size(&cache).1,
            })
        })
        .await
    }

    /// Move pictures kept by earlier versions (absolute paths under
    /// `metadata/items`, `metadata/collections` and `metadata/people`) into
    /// the blob store. Runs at every start; only does work once.
    pub async fn migrate_legacy(&self, metadata_dir: &Path) -> anyhow::Result<()> {
        let prefix = path_str(metadata_dir);
        let like = format!("{}%", like_escape(&prefix));
        let mut moved = 0u64;
        // An override and the item it applies to name the same file.
        let mut done: HashMap<String, Option<String>> = HashMap::new();
        for (table, key, columns) in
            [("items", "id", &["poster", "backdrop", "thumb"][..]), ("collections", "id", &["poster", "backdrop"][..])]
        {
            for column in columns {
                let rows: Vec<(i64, String)> =
                    sqlx::query_as(&format!("SELECT {key}, {column} FROM {table} WHERE {column} LIKE ? ESCAPE '^'"))
                        .bind(&like)
                        .fetch_all(&self.db.read)
                        .await?;
                for (id, old) in rows {
                    let new = self.import_once(&old, metadata_dir, &mut done, &mut moved).await;
                    sqlx::query(&format!("UPDATE {table} SET {column} = ? WHERE {key} = ? AND {column} = ?"))
                        .bind(new)
                        .bind(id)
                        .bind(&old)
                        .execute(&self.db.write)
                        .await?;
                }
            }
        }
        let overrides: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT item_id, field, value FROM item_overrides WHERE value LIKE ? ESCAPE '^'")
                .bind(&like)
                .fetch_all(&self.db.read)
                .await?;
        for (item, field, old) in overrides {
            let new = self.import_once(&old, metadata_dir, &mut done, &mut moved).await;
            sqlx::query("UPDATE item_overrides SET value = ? WHERE item_id = ? AND field = ?")
                .bind(new)
                .bind(item)
                .bind(field)
                .execute(&self.db.write)
                .await?;
        }
        // People's photos were files named after their TMDB path.
        let people_dir = metadata_dir.join("people");
        if people_dir.is_dir() {
            let people: Vec<(String,)> = sqlx::query_as("SELECT photo_url FROM people WHERE photo_url IS NOT NULL")
                .fetch_all(&self.db.read)
                .await?;
            for (url,) in people {
                let Some(file) = url.rsplit('/').next().map(|name| people_dir.join(name)) else { continue };
                if !file.is_file() {
                    continue;
                }
                if let Some(blob) = self.import(&file, metadata_dir).await {
                    moved += 1;
                    sqlx::query("INSERT OR IGNORE INTO remote_images (url, blob, fetched_at) VALUES (?, ?, ?)")
                        .bind(&url)
                        .bind(&blob)
                        .bind(now())
                        .execute(&self.db.write)
                        .await?;
                }
            }
        }
        for dir in ["items", "collections", "people"] {
            let dir = metadata_dir.join(dir);
            if dir.is_dir() {
                let _ = crate::util::blocking(move || {
                    remove_old_layout(&dir);
                    Ok(())
                })
                .await;
            }
        }
        if moved > 0 {
            tracing::info!("moved {moved} pictures into the blob store");
        }
        Ok(())
    }

    async fn import_once(
        &self,
        old: &str,
        metadata_dir: &Path,
        done: &mut HashMap<String, Option<String>>,
        moved: &mut u64,
    ) -> Option<String> {
        if let Some(new) = done.get(old) {
            return new.clone();
        }
        let new = self.import(Path::new(old), metadata_dir).await;
        *moved += u64::from(new.is_some());
        done.insert(old.to_owned(), new.clone());
        new
    }

    /// Move a file kept under `metadata_dir` into the blob store.
    async fn import(&self, path: &Path, metadata_dir: &Path) -> Option<String> {
        if !path.starts_with(metadata_dir) {
            return None;
        }
        let bytes = tokio::fs::read(path).await.ok()?;
        let ext = picture_ext(&bytes)?;
        let blob = self.put(&bytes, ext).await.ok()?;
        let _ = tokio::fs::remove_file(path).await;
        Some(blob)
    }
}

/// Escapes `LIKE` wildcards with `^` (backslashes are common in Windows
/// paths): use with `ESCAPE '^'`.
pub(crate) fn like_escape(value: &str) -> String {
    value.replace('^', "^^").replace('%', "^%").replace('_', "^_")
}

fn old_enough(path: &Path, grace: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_none_or(|age| age >= grace)
}

/// Remove entries of a two-level store (`ab/<name>`) that `keep` rejects.
fn sweep(root: &Path, grace: Duration, keep: impl Fn(&str) -> bool) -> (u64, u64) {
    let (mut count, mut bytes) = (0, 0);
    let Ok(shards) = std::fs::read_dir(root) else { return (0, 0) };
    for shard in shards.flatten() {
        let Ok(entries) = std::fs::read_dir(shard.path()) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if keep(&name) || !old_enough(&path, grace) {
                continue;
            }
            let size = if path.is_dir() { tree_size(&path).1 } else { entry.metadata().map(|m| m.len()).unwrap_or(0) };
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path).is_ok()
            } else {
                std::fs::remove_file(&path).is_ok()
            };
            if removed {
                count += 1;
                bytes += size;
            }
        }
        let _ = std::fs::remove_dir(shard.path()); // only when empty
    }
    (count, bytes)
}

/// Files and bytes under a folder.
fn tree_size(root: &Path) -> (u64, u64) {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .fold((0, 0), |(n, b), e| (n + 1, b + e.metadata().map(|m| m.len()).unwrap_or(0)))
}

/// Remove the old per-item folders. Nothing refers to their files any more:
/// pictures were moved into the store (references to files that could not
/// be moved were cleared), and frame grabs are made again on demand.
fn remove_old_layout(dir: &Path) {
    if let Err(err) = std::fs::remove_dir_all(dir) {
        tracing::debug!("removing {}: {err}", dir.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opensubtitles_hash_of_small_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.bin");
        // 8 bytes: size 8 + the word, counted at both ends.
        std::fs::write(&file, 1u64.to_le_bytes()).unwrap();
        assert_eq!(fingerprint(&file).unwrap(), format!("{:016x}", 8u64 + 1 + 1));
    }

    #[test]
    fn opensubtitles_hash_reference_value() {
        // Reference: http://www.opensubtitles.org/addons/avi/breakdance.avi
        // isn't shipped; check the algorithm on known synthetic data instead.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("b.bin");
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&file, &data).unwrap();
        let mut expected = data.len() as u64;
        for chunk in [&data[..65536], &data[data.len() - 65536..]] {
            for word in chunk.chunks(8) {
                expected = expected.wrapping_add(u64::from_le_bytes(word.try_into().unwrap()));
            }
        }
        assert_eq!(fingerprint(&file).unwrap(), format!("{expected:016x}"));
    }

    #[test]
    fn only_plain_blob_names_become_paths() {
        let hash = "a".repeat(64);
        assert!(valid_blob_name(&format!("{hash}.jpg")));
        assert!(!valid_blob_name(&format!("{hash}.exe")));
        assert!(!valid_blob_name("../../etc/passwd.jpg"));
        assert!(!valid_blob_name(&format!("{}.jpg", "A".repeat(64))));
        assert!(allowed_image_url("https://image.tmdb.org/t/p/w780/x.jpg"));
        assert!(!allowed_image_url("http://image.tmdb.org/t/p/w780/x.jpg"));
        assert!(!allowed_image_url("https://image.tmdb.org.evil.com/x.jpg"));
        assert!(!allowed_image_url("https://localhost/x.jpg"));
    }

    #[tokio::test]
    async fn same_picture_is_stored_once() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("pley.db")).await.unwrap();
        let store = Store::new(db, &dir.path().join("metadata"), &dir.path().join("media"), &dir.path().join("cache"));
        let a = store.put(b"\xff\xd8\xff\xe0 picture", "jpg").await.unwrap();
        let b = store.put(b"\xff\xd8\xff\xe0 picture", "jpg").await.unwrap();
        assert_eq!(a, b);
        assert!(is_blob(&a));
        assert!(store.exists(&a));
        let path = store.path(&a).unwrap();
        assert!(path.starts_with(dir.path().join("metadata").join("blobs")));
        assert_eq!(store.path("C:/media/poster.jpg").unwrap(), PathBuf::from("C:/media/poster.jpg"));
    }
}
