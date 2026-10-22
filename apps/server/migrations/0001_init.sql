-- pley schema v1

CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    username      TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT    NOT NULL,
    is_admin      INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL
);

CREATE TABLE sessions (
    token_hash   TEXT    PRIMARY KEY,
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,
    user_agent   TEXT
) WITHOUT ROWID;
CREATE INDEX sessions_user ON sessions(user_id);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE libraries (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    kind       TEXT    NOT NULL CHECK (kind IN ('movies', 'shows', 'music')),
    paths      TEXT    NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    scanned_at INTEGER
);

-- Every browsable thing: movie, show, season, episode, artist, album, track.
CREATE TABLE items (
    id              INTEGER PRIMARY KEY,
    library_id      INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    parent_id       INTEGER REFERENCES items(id) ON DELETE CASCADE,
    grandparent_id  INTEGER REFERENCES items(id) ON DELETE CASCADE,
    kind            TEXT    NOT NULL,
    key             TEXT    NOT NULL,          -- stable identity within the library
    title           TEXT    NOT NULL,
    sort_title      TEXT    NOT NULL,
    original_title  TEXT,
    year            INTEGER,
    idx             INTEGER,                   -- season / episode / track number
    disc            INTEGER,                   -- disc number (tracks)
    summary         TEXT,
    tagline         TEXT,
    rating          REAL,
    content_rating  TEXT,
    genres          TEXT,                      -- JSON array of strings
    studio          TEXT,
    release_date    TEXT,                      -- YYYY-MM-DD
    duration_ms     INTEGER,
    path            TEXT,                      -- folder (show/season/album/artist/movie)
    poster          TEXT,                      -- absolute path of artwork files
    backdrop        TEXT,
    thumb           TEXT,
    tmdb_id         INTEGER,
    imdb_id         TEXT,
    leaf_count      INTEGER,
    child_count     INTEGER,
    added_at        INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    metadata_at     INTEGER,                   -- last successful agent refresh (NULL = never)
    metadata_locked INTEGER NOT NULL DEFAULT 0,
    UNIQUE (library_id, key)
);
CREATE INDEX items_library_kind_sort  ON items(library_id, kind, sort_title);
CREATE INDEX items_library_kind_added ON items(library_id, kind, added_at DESC);
CREATE INDEX items_parent             ON items(parent_id, idx);
CREATE INDEX items_grandparent        ON items(grandparent_id);
CREATE INDEX items_kind_added         ON items(kind, added_at DESC);

CREATE TABLE media_files (
    id            INTEGER PRIMARY KEY,
    item_id       INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path          TEXT    NOT NULL UNIQUE,
    size          INTEGER NOT NULL,
    mtime         INTEGER NOT NULL,
    container     TEXT,
    duration_ms   INTEGER,
    bitrate       INTEGER,
    width         INTEGER,
    height        INTEGER,
    video_codec   TEXT,
    audio_codec   TEXT,
    media_info    TEXT,                        -- JSON MediaInfo from the transcoder (streams, tags, chapters)
    external_subs TEXT    NOT NULL DEFAULT '[]', -- JSON [{path, language, forced, codec}]
    probed        INTEGER NOT NULL DEFAULT 0,
    added_at      INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX media_files_item    ON media_files(item_id);
CREATE INDEX media_files_library ON media_files(library_id);

CREATE TABLE user_item_state (
    user_id        INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id        INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position_ms    INTEGER NOT NULL DEFAULT 0,
    watched        INTEGER NOT NULL DEFAULT 0,
    play_count     INTEGER NOT NULL DEFAULT 0,
    last_played_at INTEGER,
    updated_at     INTEGER NOT NULL,
    PRIMARY KEY (user_id, item_id)
) WITHOUT ROWID;
CREATE INDEX user_item_state_item   ON user_item_state(item_id);
CREATE INDEX user_item_state_recent ON user_item_state(user_id, last_played_at DESC);

-- Full text search over titles (external content table kept in sync by triggers).
CREATE VIRTUAL TABLE items_fts USING fts5(
    title, original_title,
    content = 'items', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER items_fts_insert AFTER INSERT ON items BEGIN
    INSERT INTO items_fts(rowid, title, original_title) VALUES (new.id, new.title, new.original_title);
END;
CREATE TRIGGER items_fts_delete AFTER DELETE ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, title, original_title) VALUES ('delete', old.id, old.title, old.original_title);
END;
CREATE TRIGGER items_fts_update AFTER UPDATE OF title, original_title ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, title, original_title) VALUES ('delete', old.id, old.title, old.original_title);
    INSERT INTO items_fts(rowid, title, original_title) VALUES (new.id, new.title, new.original_title);
END;
