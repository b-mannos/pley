-- Collections (per library, shared) and playlists (per user, private).

CREATE TABLE collections (
    id         INTEGER PRIMARY KEY,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title      TEXT    NOT NULL,
    sort_title TEXT    NOT NULL,
    summary    TEXT,
    -- Set for automatic collections (TMDB "belongs_to_collection").
    tmdb_id    INTEGER,
    -- Absolute paths of downloaded artwork.
    poster     TEXT,
    backdrop   TEXT,
    -- How items are ordered: release, title or custom (position).
    sort       TEXT    NOT NULL DEFAULT 'release' CHECK (sort IN ('release', 'title', 'custom')),
    -- An admin changed the title or summary: TMDB refreshes leave them alone.
    edited     INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX collections_tmdb ON collections(library_id, tmdb_id) WHERE tmdb_id IS NOT NULL;

CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    item_id       INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (collection_id, item_id)
) WITHOUT ROWID;
CREATE INDEX collection_items_item ON collection_items(item_id);

CREATE TABLE playlists (
    id         INTEGER PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title      TEXT    NOT NULL,
    kind       TEXT    NOT NULL CHECK (kind IN ('video', 'audio')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX playlists_user ON playlists(user_id);

-- One row per entry: the same item may appear twice.
CREATE TABLE playlist_items (
    id          INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    item_id     INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    added_at    INTEGER NOT NULL
);
CREATE INDEX playlist_items_order ON playlist_items(playlist_id, position);
CREATE INDEX playlist_items_item ON playlist_items(item_id);
