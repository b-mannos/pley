-- Audio and subtitle choices remembered per user: for a whole show (applied
-- to every episode) or for a single movie. Stored as track properties, not
-- indexes, so they carry over to files with a different track layout.
CREATE TABLE stream_preferences (
    user_id           INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id           INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    audio_language    TEXT,
    audio_title       TEXT,
    subtitles         INTEGER NOT NULL DEFAULT 0, -- 0 = off, 1 = on
    subtitle_language TEXT,
    subtitle_title    TEXT,
    subtitle_forced   INTEGER NOT NULL DEFAULT 0,
    updated_at        INTEGER NOT NULL,
    PRIMARY KEY (user_id, item_id)
) WITHOUT ROWID;
