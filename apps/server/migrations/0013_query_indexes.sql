-- Libraries sorted (and hubs picked) by year, rating or release date walk an
-- index instead of sorting every item.
CREATE INDEX items_library_kind_year    ON items(library_id, kind, year);
CREATE INDEX items_library_kind_rating  ON items(library_id, kind, rating);
CREATE INDEX items_library_kind_release ON items(library_id, kind, release_date);

-- Genres as rows, kept in step with `items.genres` (a JSON array) by the
-- triggers below: "More Like This", genre filters and the genre list look
-- them up instead of parsing every item's JSON.
CREATE TABLE item_genres (
    genre      TEXT    NOT NULL,
    library_id INTEGER NOT NULL,
    kind       TEXT    NOT NULL,
    item_id    INTEGER NOT NULL,
    PRIMARY KEY (genre, library_id, kind, item_id)
) WITHOUT ROWID;
CREATE INDEX item_genres_item ON item_genres(item_id);

INSERT OR IGNORE INTO item_genres (genre, library_id, kind, item_id)
SELECT g.value, i.library_id, i.kind, i.id
FROM items i, json_each(i.genres) g
WHERE i.genres IS NOT NULL AND json_valid(i.genres) AND g.type = 'text';

CREATE TRIGGER item_genres_insert AFTER INSERT ON items
WHEN NEW.genres IS NOT NULL AND json_valid(NEW.genres)
BEGIN
    INSERT OR IGNORE INTO item_genres (genre, library_id, kind, item_id)
    SELECT value, NEW.library_id, NEW.kind, NEW.id FROM json_each(NEW.genres) WHERE type = 'text';
END;

CREATE TRIGGER item_genres_update AFTER UPDATE OF genres, library_id, kind ON items
BEGIN
    DELETE FROM item_genres WHERE item_id = NEW.id;
    INSERT OR IGNORE INTO item_genres (genre, library_id, kind, item_id)
    SELECT value, NEW.library_id, NEW.kind, NEW.id
    FROM json_each(CASE WHEN json_valid(NEW.genres) THEN NEW.genres ELSE '[]' END)
    WHERE type = 'text';
END;

CREATE TRIGGER item_genres_delete AFTER DELETE ON items
BEGIN
    DELETE FROM item_genres WHERE item_id = OLD.id;
END;

-- Search for people by any word of their name (like items_fts), instead of
-- a LIKE over every name.
CREATE VIRTUAL TABLE people_fts USING fts5(
    name,
    content = 'people', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
INSERT INTO people_fts(rowid, name) SELECT id, name FROM people;
CREATE TRIGGER people_fts_insert AFTER INSERT ON people BEGIN
    INSERT INTO people_fts(rowid, name) VALUES (new.id, new.name);
END;
CREATE TRIGGER people_fts_delete AFTER DELETE ON people BEGIN
    INSERT INTO people_fts(people_fts, rowid, name) VALUES ('delete', old.id, old.name);
END;
CREATE TRIGGER people_fts_update AFTER UPDATE OF name ON people BEGIN
    INSERT INTO people_fts(people_fts, rowid, name) VALUES ('delete', old.id, old.name);
    INSERT INTO people_fts(rowid, name) VALUES (new.id, new.name);
END;

-- Versions of what library listings show, bumped by every change (even two
-- in the same second): a library's items, and a user's watch state and
-- ratings. Listing ETags are made from them (see api/libraries.rs).
CREATE TABLE listing_versions (
    scope   TEXT    NOT NULL CHECK (scope IN ('library', 'user')),
    id      INTEGER NOT NULL,
    version INTEGER NOT NULL,
    PRIMARY KEY (scope, id)
) WITHOUT ROWID;

CREATE TRIGGER listing_items_insert AFTER INSERT ON items BEGIN
    INSERT INTO listing_versions (scope, id, version) VALUES ('library', NEW.library_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
END;
CREATE TRIGGER listing_items_update AFTER UPDATE ON items BEGIN
    INSERT INTO listing_versions (scope, id, version) VALUES ('library', NEW.library_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
    INSERT INTO listing_versions (scope, id, version) VALUES ('library', OLD.library_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
END;
CREATE TRIGGER listing_items_delete AFTER DELETE ON items BEGIN
    INSERT INTO listing_versions (scope, id, version) VALUES ('library', OLD.library_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
END;
CREATE TRIGGER listing_state_insert AFTER INSERT ON user_item_state BEGIN
    INSERT INTO listing_versions (scope, id, version) VALUES ('user', NEW.user_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
END;
CREATE TRIGGER listing_state_update AFTER UPDATE ON user_item_state BEGIN
    INSERT INTO listing_versions (scope, id, version) VALUES ('user', NEW.user_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
END;
CREATE TRIGGER listing_state_delete AFTER DELETE ON user_item_state BEGIN
    INSERT INTO listing_versions (scope, id, version) VALUES ('user', OLD.user_id, 1)
    ON CONFLICT (scope, id) DO UPDATE SET version = version + 1;
END;
