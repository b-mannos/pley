-- A library's id is never given to another library: open pages, Plex apps
-- and an order being saved may still hold a deleted library's id. SQLite
-- gives a new row the highest id there is plus one, so the id of a deleted
-- newest library would come back; new libraries count on from the highest
-- id ever given instead.
CREATE TABLE library_ids (last INTEGER NOT NULL);
INSERT INTO library_ids (last) SELECT COALESCE(MAX(id), 0) FROM libraries;

CREATE TRIGGER library_ids_given AFTER INSERT ON libraries BEGIN
    UPDATE library_ids SET last = MAX(last, NEW.id);
END;
