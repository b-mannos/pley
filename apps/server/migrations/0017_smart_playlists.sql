-- Smart playlists hold no entries: they play what a section listing query
-- (Plex's filter syntax, as for smart collections) finds in their library,
-- shows and albums as their episodes and tracks, as their owner may see it.
ALTER TABLE playlists ADD COLUMN library_id INTEGER REFERENCES libraries(id) ON DELETE CASCADE;
ALTER TABLE playlists ADD COLUMN smart_filter TEXT;
