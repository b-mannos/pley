-- A short id of each Live TV source (a playlist or an Xtream connection)
-- that addresses name it by: `/live/{uid}/{channel}` says which source a
-- channel comes from, where two sources' channels share numbers.
ALTER TABLE live_sources ADD COLUMN uid TEXT NOT NULL DEFAULT '';
UPDATE live_sources SET uid = lower(hex(randomblob(3))) WHERE uid = '';
CREATE UNIQUE INDEX live_sources_uid ON live_sources (uid) WHERE uid != '';
