-- One row per play that reached the end (where an item counts as watched).
-- Marking an item watched by hand is not a play and is not recorded.
CREATE TABLE play_history (
    id        INTEGER PRIMARY KEY,
    user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id   INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    viewed_at INTEGER NOT NULL,
    device    TEXT                -- "Chrome on Windows", a Plex client's name
);
CREATE INDEX play_history_user ON play_history (user_id, viewed_at);
CREATE INDEX play_history_viewed ON play_history (viewed_at);
CREATE INDEX play_history_item ON play_history (item_id);
