-- An app grant's id is never given to another one, as a title's isn't
-- (0026): a playback a TV started keeps its grant's id, for its keyed media
-- addresses, and once that app was signed out the next grant took the same
-- id, so the addresses worked again. New grants count on from the highest
-- id ever given instead.
CREATE TABLE grant_ids (last INTEGER NOT NULL);
INSERT INTO grant_ids (last) SELECT COALESCE(MAX(id), 0) FROM app_grants;

CREATE TRIGGER grant_ids_given AFTER INSERT ON app_grants BEGIN
    UPDATE grant_ids SET last = MAX(last, NEW.id);
END;
