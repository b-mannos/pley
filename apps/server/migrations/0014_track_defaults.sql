-- Each user's default tracks, for titles they haven't picked tracks for
-- (see streams.rs): the audio language to prefer, and when subtitles come
-- on, in which language.
ALTER TABLE users ADD COLUMN audio_language TEXT;     -- ISO 639 code; NULL: the file's default track
ALTER TABLE users ADD COLUMN subtitle_language TEXT;  -- NULL: the audio language
ALTER TABLE users ADD COLUMN subtitle_mode TEXT NOT NULL DEFAULT 'forced'
    CHECK (subtitle_mode IN ('off', 'forced', 'foreign', 'always'));
