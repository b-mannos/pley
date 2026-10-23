-- Metadata providers besides TMDB: TVmaze and TheTVDB for TV shows,
-- MusicBrainz for music. The id an item got from each provider is kept so
-- later refreshes go straight to the same entry.
ALTER TABLE items ADD COLUMN tvdb_id INTEGER;          -- shows: TheTVDB series
ALTER TABLE items ADD COLUMN tvmaze_id INTEGER;        -- shows: TVmaze show
ALTER TABLE items ADD COLUMN musicbrainz_id TEXT;      -- artists: artist MBID, albums: release group MBID

-- People credited by TVmaze or TheTVDB are known by that provider's id, like
-- TMDB's by tmdb_id. Each person row has exactly one of the three ids.
ALTER TABLE people ADD COLUMN tvmaze_id INTEGER;
ALTER TABLE people ADD COLUMN tvdb_id INTEGER;
CREATE UNIQUE INDEX people_tvmaze ON people (tvmaze_id);
CREATE UNIQUE INDEX people_tvdb ON people (tvdb_id);
