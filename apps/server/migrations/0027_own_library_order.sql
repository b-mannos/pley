-- A user's own order of the libraries they see, once they choose one (the
-- apps' Libraries pages): the libraries listed here first, by position; any
-- others after them (new ones, or ones they were given since), in the order
-- everyone sees them in (libraries.position).
CREATE TABLE library_order (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    position   INTEGER NOT NULL,
    PRIMARY KEY (user_id, library_id)
) WITHOUT ROWID;
