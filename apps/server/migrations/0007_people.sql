-- Cast and crew from TMDB. People are shared between items; a person's photo
-- is downloaded the first time it is shown.
CREATE TABLE people (
    id         INTEGER PRIMARY KEY,
    tmdb_id    INTEGER UNIQUE,
    name       TEXT    NOT NULL,
    photo_path TEXT,                -- TMDB profile image, e.g. "/abc.jpg"
    updated_at INTEGER NOT NULL
);

CREATE TABLE item_people (
    item_id   INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    person_id INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    role      TEXT    NOT NULL CHECK (role IN ('actor', 'creator', 'director', 'writer', 'producer')),
    character TEXT,                 -- actors: the part played
    position  INTEGER NOT NULL,     -- billing order
    PRIMARY KEY (item_id, role, person_id)
) WITHOUT ROWID;
CREATE INDEX item_people_person ON item_people (person_id);

-- When the credits of a movie, show or episode were stored. NULL: never, so
-- items matched before this table existed get theirs fetched once.
ALTER TABLE items ADD COLUMN credits_at INTEGER;
