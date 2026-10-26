-- A title's, a file's or a profile's id is never given to another one, as
-- a library's isn't (0021): apps keep them (a phone's downloads, Plex apps'
-- ratingKeys, remembered pages), and SQLite gives a new row the highest id
-- there is plus one, so the id of a deleted newest row came back for the
-- next one (a download of a deleted film played for the episode added
-- after it). New rows count on from the highest id ever given instead.
CREATE TABLE item_ids (last INTEGER NOT NULL);
INSERT INTO item_ids (last) SELECT COALESCE(MAX(id), 0) FROM items;

CREATE TRIGGER item_ids_given AFTER INSERT ON items BEGIN
    UPDATE item_ids SET last = MAX(last, NEW.id);
END;

CREATE TABLE file_ids (last INTEGER NOT NULL);
INSERT INTO file_ids (last) SELECT COALESCE(MAX(id), 0) FROM media_files;

CREATE TRIGGER file_ids_given AFTER INSERT ON media_files BEGIN
    UPDATE file_ids SET last = MAX(last, NEW.id);
END;

CREATE TABLE user_ids (last INTEGER NOT NULL);
INSERT INTO user_ids (last) SELECT COALESCE(MAX(id), 0) FROM users;

CREATE TRIGGER user_ids_given AFTER INSERT ON users BEGIN
    UPDATE user_ids SET last = MAX(last, NEW.id);
END;
