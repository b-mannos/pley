-- Intro and credits markers (Skip Intro / Skip Credits). They belong to a
-- media file rather than an item because the files of one item can be
-- different cuts.
CREATE TABLE markers (
    file_id    INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    kind       TEXT    NOT NULL CHECK (kind IN ('intro', 'credits')),
    start_ms   INTEGER NOT NULL,
    end_ms     INTEGER NOT NULL,
    source     TEXT    NOT NULL,              -- 'chapter' | 'fingerprint'
    created_at INTEGER NOT NULL,
    PRIMARY KEY (file_id, kind)
) WITHOUT ROWID;

-- Which version of which file was analysed, and by which algorithm version,
-- so changed files and algorithm improvements are analysed again.
CREATE TABLE marker_analysis (
    file_id       INTEGER PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,
    version       INTEGER NOT NULL,
    size          INTEGER NOT NULL,
    mtime         INTEGER NOT NULL,
    fingerprinted INTEGER NOT NULL DEFAULT 0,  -- audio fingerprints were compared
    analyzed_at   INTEGER NOT NULL
);
