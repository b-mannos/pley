# Storage

What pley keeps on disk, where, and how it is cleaned up. Everything lives in
the data folder (`--data-dir`, see the README) except your media and its local
artwork, which pley never moves or changes.

```
<data>/
  pley.toml                  the config file (unless --config names another):
                             Settings → Network saves where pley listens there
  pley.db                    the database (SQLite, WAL)
  pley.lock                  locked while pley runs: one server per data folder
  plex-machine-id            the compatibility API's server id, kept so tools
                             know the server again
  secret.key                 the key that encrypts authenticator secrets
                             (two-step sign-in): keep it with the database
  metadata/blobs/ab/<sha256>.<ext>
                             pictures that are metadata
  media/ab/<fingerprint>/    files generated from one media file
    thumb.jpg                frame grab (movies and episodes without a still)
    trickplay/               seek-bar previews (sprite sheets and index-sd.bif files)
    chapters/<n>.jpg         chapter pictures, cut from the previews
  avatars/                   profile pictures
  backups/pley-<date>.db     the last 3 daily database backups, and the copy
                             made before the last upgrade
  cache/                     resized images, subtitles, fingerprints; safe to
                             delete, old files are removed (see Clean-up)
  transcode/                 the managed transcoder's work folder (stream
                             segments, its own caches, converted downloads;
                             `transcode_dir` in pley.toml puts it elsewhere).
                             The transcoder prunes it: docs/TRANSCODER.md
  logs/pley.log              the server log, then pley.log.1 to .3 (10 MB each)
```

## Pictures (`metadata/blobs`)

Downloaded artwork, uploads, embedded album covers and people's photos are
stored once, named by the SHA-256 of their content. The database refers to them
as `blob:<sha256>.<ext>`:

- The same picture is stored once, however many items use it.
- No absolute paths in the database: the data folder can be moved or mounted
  elsewhere (Docker) without breaking anything.
- Downloads remember their URL (`remote_images`). Metadata providers change an
  image's URL when the image changes, so a refresh that finds the same URL
  reuses the stored copy instead of downloading it again.
- Pictures are only downloaded over HTTPS from the providers' image servers.

Local artwork next to the media (`poster.jpg`, `fanart.jpg`, `season01-poster.jpg`,
`cover.jpg`, …) is referenced by its path and always wins over downloaded
artwork.

## Generated media files (`media/`)

Frame grabs, seek-bar previews and chapter images are made from a media file
and kept in a folder named after the file's **fingerprint**: the OpenSubtitles
hash (the file size plus a checksum of its first and last 64 KiB). They follow
the file when it is moved or renamed, and two copies of the same file share
them. Fingerprints are computed in the background after scans; a file that
changes gets a new one. When they cannot be saved (a full disk, a read-only
database) the background run stops after a few failures and the next scan
tries again.

## Database backups

Once a day (and on start when the last backup is older than a day) pley
writes a copy of the database to `backups/pley-YYYY-MM-DD-HHMM.db` with
SQLite's `VACUUM INTO`, which reads a consistent snapshot while the server
keeps running. The copy is checked before it counts, and the last three are
kept. `POST /api/system/backups` makes one on demand; `GET /api/system/backups`
lists them with the result of the check of the live database that runs at
every start.

Before a new version changes the database (it brings the schema up to date
when it starts), pley writes a checked copy to
`backups/pley-YYYY-MM-DD-HHMM-before-upgrade.db`, and doesn't start when it
can't (a full disk, say). The copy from the last upgrade is kept until the
next one, apart from the daily backups.

To restore one: stop pley, move `pley.db`, `pley.db-wal` and `pley.db-shm`
aside, copy the backup to `pley.db`, and start pley again.

To move pley to another computer, copy the whole data folder. A database
without its `secret.key` can't check codes from authenticator apps: people
with two-step sign-in then sign in with a recovery code and set it up again
(or an administrator turns it off for them).

Never copy `pley.db` while pley is running: most of the latest changes live in
`pley.db-wal` until SQLite checkpoints them, so the copy would be stale or
inconsistent. Use a backup instead.

## Clean-up

Pictures and media folders that nothing uses any more (a removed movie, a
replaced poster, a deleted file) are removed after scans that removed items,
once a day, and on demand (`POST /api/system/storage/clean`). A picture is
never removed between being stored and being used:

- Files younger than an hour are left alone.
- So are pictures and media folders handed out in the last hour, however old
  their files: a picture stored again or found again by its URL (a refresh,
  a re-upload), a media folder asked for (a file moved back). This is checked
  right before each removal, so it holds while a clean-up runs.
- The URLs of downloads nothing uses are forgotten with their pictures, after
  the same hour.

The same clean-ups remove old files from `cache/`. Cached files are named
after what they were made from (a path, a size, a modification time), so the
files of media that changed or went are never asked for again; the age limits
keep the cache from growing for ever:

| Folder | What | Kept for |
|---|---|---|
| `cache/images` | resized pictures | 30 days |
| `cache/covers` | albums known to have no embedded cover | 30 days |
| `cache/subtitles` | subtitles converted to WebVTT, and picture subtitles copied into Matroska (`.mks`) | 90 days |
| `cache/fingerprints` | audio fingerprints (intro and credits detection) | 90 days |

A file that is needed again later is simply made again. Subtitles are written
to a temporary file first, so a crash or a full disk never leaves one cut
short.

`GET /api/system/storage` reports the space used.

Earlier versions kept pictures as files under `metadata/items`,
`metadata/collections` and `metadata/people` with absolute paths in the
database. They are moved into the blob store on the first start of a version
with this layout.
