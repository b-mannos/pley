-- Live TV (IPTV): where channels come from, the channels, their guide, and
-- what each user marked.

-- A source of channels: an M3U playlist (`url`, the guide often named in
-- it) or an Xtream Codes account (`url` its server, with `username` and
-- `password`). `max_connections` is how many channels the provider lets
-- it play at once (0: no limit): each channel watched takes one, however
-- many watch it. `account` is what an Xtream server last said of the
-- account (JSON: its status, expiry and connections).
CREATE TABLE live_sources (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT    NOT NULL,
    kind            TEXT    NOT NULL CHECK (kind IN ('m3u', 'xtream')),
    url             TEXT    NOT NULL,
    username        TEXT    NOT NULL DEFAULT '',
    password        TEXT    NOT NULL DEFAULT '',
    guide_url       TEXT    NOT NULL DEFAULT '',
    user_agent      TEXT    NOT NULL DEFAULT '',
    max_connections INTEGER NOT NULL DEFAULT 1,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    channels_at     INTEGER,                    -- the last list read
    channels_error  TEXT,                       -- why the last try failed
    guide_at        INTEGER,
    guide_error     TEXT,
    account         TEXT
);

-- A channel as its source lists it, `key` its id there (an Xtream stream
-- id, else what the playlist names it by). What administrators set (hidden,
-- a name, a number, a guide channel of their own) stays across refreshes.
-- A channel the source stops listing is kept, out of sight, for 14 days
-- (`missing_since`): favourites survive a provider's bad day.
-- `matched_guide_id`: the guide's channel of the same name, for channels
-- that name none.
CREATE TABLE live_channels (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id        INTEGER NOT NULL REFERENCES live_sources(id) ON DELETE CASCADE,
    key              TEXT    NOT NULL,
    name             TEXT    NOT NULL,
    number           TEXT,
    group_name       TEXT    NOT NULL DEFAULT '',
    logo_url         TEXT    NOT NULL DEFAULT '',
    guide_id         TEXT    NOT NULL DEFAULT '',
    matched_guide_id TEXT,
    stream_url       TEXT    NOT NULL,
    user_agent       TEXT,                      -- the playlist's own for it (#EXTVLCOPT)
    position         INTEGER NOT NULL,
    missing_since    INTEGER,
    hidden           INTEGER NOT NULL DEFAULT 0,
    custom_name      TEXT,
    custom_number    TEXT,
    custom_guide_id  TEXT,
    UNIQUE (source_id, key)
);

-- The guide: programmes by the guide's channel id, from a little before
-- now to as far ahead as the guide goes (two weeks at most).
CREATE TABLE live_programs (
    source_id   INTEGER NOT NULL REFERENCES live_sources(id) ON DELETE CASCADE,
    guide_id    TEXT    NOT NULL,
    start       INTEGER NOT NULL,
    stop        INTEGER NOT NULL,
    title       TEXT    NOT NULL,
    subtitle    TEXT,
    description TEXT,
    categories  TEXT,                           -- JSON array
    image       TEXT,
    season      INTEGER,
    episode     INTEGER,
    is_new      INTEGER NOT NULL DEFAULT 0,
    is_live     INTEGER NOT NULL DEFAULT 0,
    rating      TEXT,
    PRIMARY KEY (source_id, guide_id, start)
) WITHOUT ROWID;
CREATE INDEX live_programs_stop ON live_programs (stop);

CREATE TABLE live_favorites (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id INTEGER NOT NULL REFERENCES live_channels(id) ON DELETE CASCADE,
    added_at   INTEGER NOT NULL,
    PRIMARY KEY (user_id, channel_id)
) WITHOUT ROWID;

-- The channels each user watched last, for "Recently watched".
CREATE TABLE live_watched (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id INTEGER NOT NULL REFERENCES live_channels(id) ON DELETE CASCADE,
    watched_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, channel_id)
) WITHOUT ROWID;
