-- Account management: profiles, home switching with PINs, per-user library
-- access, age limits and invitations.

ALTER TABLE users ADD COLUMN display_name TEXT;
ALTER TABLE users ADD COLUMN avatar_color TEXT;
-- File name of an uploaded picture in <data>/avatars.
ALTER TABLE users ADD COLUMN avatar TEXT;
-- Optional PIN protecting the profile when switching to it.
ALTER TABLE users ADD COLUMN pin_hash TEXT;
-- Shown in the "Who's watching?" switcher of other home members.
ALTER TABLE users ADD COLUMN home INTEGER NOT NULL DEFAULT 1;
-- Managed profiles have no password and are only reachable by switching.
ALTER TABLE users ADD COLUMN managed INTEGER NOT NULL DEFAULT 0;
-- 0 = only the libraries listed in user_libraries.
ALTER TABLE users ADD COLUMN all_libraries INTEGER NOT NULL DEFAULT 1;
-- Highest age rating shown (12 = up to PG-13 / TV-PG / FSK 12); NULL = no limit.
ALTER TABLE users ADD COLUMN max_age INTEGER;
ALTER TABLE users ADD COLUMN disabled INTEGER NOT NULL DEFAULT 0;

CREATE TABLE user_libraries (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, library_id)
) WITHOUT ROWID;

-- Minimum age for the item's own content rating (episodes and seasons
-- inherit their show's); NULL = unrated.
ALTER TABLE items ADD COLUMN rating_age INTEGER;

CREATE TABLE invites (
    code          TEXT    PRIMARY KEY,
    created_by    INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at    INTEGER NOT NULL,
    expires_at    INTEGER,
    uses_left     INTEGER NOT NULL DEFAULT 1,
    all_libraries INTEGER NOT NULL DEFAULT 1,
    library_ids   TEXT    NOT NULL DEFAULT '[]',
    max_age       INTEGER,
    note          TEXT
) WITHOUT ROWID;

-- Where a session was last used from, for the devices list.
ALTER TABLE sessions ADD COLUMN last_ip TEXT;
