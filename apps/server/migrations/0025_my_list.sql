-- Titles a user took out of Continue Watching: a movie, or a show for all of
-- its episodes. The row goes when they play that movie or an episode of that
-- show again (a progress report or a watched mark), and the title comes back
-- as it would have. Hiding never touches progress or watched marks.
CREATE TABLE continue_watching_hidden (
    user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id   INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    hidden_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, item_id)
) WITHOUT ROWID;
-- Each user's My List (watchlist): movies and shows, newest added first. A
-- movie leaves it once it is watched; a show stays until it is removed.
-- Added in the same second, the later row (rowid) comes first.
CREATE TABLE watchlist (
    user_id  INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id  INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, item_id)
);
CREATE INDEX watchlist_user_added ON watchlist(user_id, added_at DESC);
