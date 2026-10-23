-- Pictures downloaded from the internet, by URL: a refresh that finds the same
-- URL reuses the stored copy instead of downloading it again. The pictures are
-- files in <data>/metadata/blobs named by their content (see docs/STORAGE.md).
CREATE TABLE remote_images (
    url        TEXT PRIMARY KEY,
    blob       TEXT    NOT NULL,      -- "blob:<sha256>.<ext>"
    fetched_at INTEGER NOT NULL
) WITHOUT ROWID;

-- A media file's identity by content: the OpenSubtitles hash (its size plus a
-- checksum of its first and last 64 KiB). Files generated from a media file
-- (frame grabs, seek previews, chapter images) are kept under it, so they
-- survive moves and renames. NULL until computed after a scan.
ALTER TABLE media_files ADD COLUMN fingerprint TEXT;
CREATE INDEX media_files_fingerprint ON media_files (fingerprint);

-- People's photos by full URL (any metadata provider), not just TMDB paths.
ALTER TABLE people ADD COLUMN photo_url TEXT;
UPDATE people SET photo_url = 'https://image.tmdb.org/t/p/h632' || photo_path WHERE photo_path IS NOT NULL;
ALTER TABLE people DROP COLUMN photo_path;
