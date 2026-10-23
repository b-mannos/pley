-- Smart collections hold no items: they show what a section listing query
-- (Plex's filter syntax, the query string of /library/sections/{id}/all,
-- e.g. `type=1&genre=Comedy&sort=year:desc`) finds, as each user may see it.
ALTER TABLE collections ADD COLUMN smart_filter TEXT;
