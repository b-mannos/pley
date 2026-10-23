-- The order libraries are listed in everywhere (the web app's sidebar and
-- home page, the Plex-compatible API's sections), the same for every user:
-- administrators choose it, new libraries go last. Libraries there already
-- keep the order they were listed in, by name.
ALTER TABLE libraries ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

UPDATE libraries SET position = ranked.position
FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY name COLLATE NOCASE, id) - 1 AS position FROM libraries) AS ranked
WHERE libraries.id = ranked.id;
