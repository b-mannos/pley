-- Subtitles people add to a media file (uploaded, or downloaded from
-- OpenSubtitles): kept in the data folder as `subtitles/<file_id>/<id>.<ext>`,
-- never next to the media. `idx` is the track's index for every client,
-- from 9000 up, and given once among the files of the file's title
-- (embedded tracks are below 1000, subtitle files next to the video from
-- 1000 to 8999). Ids are never given again, so neither is a file name or a
-- cached conversion.
CREATE TABLE added_subtitles (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id   INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    idx       INTEGER NOT NULL,
    codec     TEXT    NOT NULL,              -- subrip, webvtt, ass or ssa
    language  TEXT,
    title     TEXT,
    forced    INTEGER NOT NULL DEFAULT 0,
    sdh       INTEGER NOT NULL DEFAULT 0,
    source    TEXT    NOT NULL,              -- 'upload', or 'opensubtitles:<file id>'
    added_by  INTEGER REFERENCES users(id) ON DELETE SET NULL,
    added_at  INTEGER NOT NULL,
    UNIQUE (file_id, idx)
);
