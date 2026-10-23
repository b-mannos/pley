# pley

A fast, self-hosted media server in the spirit of Plex, written in Rust, with a
full web interface. It organizes your movies, TV shows and music, and plays
them in any browser. It direct-plays when it can and otherwise remuxes or
transcodes on the fly, including hardware-accelerated tone mapping of HDR10,
HLG and Dolby Vision.

## Features

- **Libraries.**
  - Movies, TV shows (seasons and episodes) and music (artists, albums,
    tracks).
  - Incremental scans: only new or changed files are analyzed.
  - Automatic rescans through filesystem notifications plus a periodic timer.
  - Unmounted drives are never mistaken for deleted files.
- **Smart file name parsing.**
  - Movies: `Title (Year)` and release names (`Title.2019.1080p.BluRay…`), plus
    `{tmdb-123}` hints. Editions (`{edition-Director's Cut}`) are versions of
    one movie, named in the version menu.
  - Episodes: `S01E02`, `1x02`, multi-episode files, season folders, specials,
    date-based shows and anime numbering.
  - Music: tags first, then folders.
  - Extras: trailers, featurettes, behind-the-scenes and the rest with Plex's
    conventions (`Trailers/`, `Behind The Scenes/`, … folders or
    `Movie (2020)-trailer.mkv`), shown on the movie or show and playable.
- **Metadata** (see [Metadata sources](#metadata-sources)).
  - Movies from TMDB: posters, backdrops, summaries, ratings, certifications,
    genres and studios. It needs a free API key.
  - TV shows from TMDB, TheTVDB or TVmaze, with season and episode details.
    TVmaze needs no key, so shows get metadata out of the box.
  - Music from MusicBrainz, without a key: genres, release dates and album
    covers (from the Cover Art Archive) that the files' tags and folders lack.
  - Local artwork (`poster.jpg`, `fanart.jpg`, `season01-poster.jpg`,
    `cover.jpg`…) always wins over downloaded artwork.
  - Video frame grabs and embedded album art are extracted on demand.
  - Cast and crew: actors with their parts, directors, writers and creators,
    and a page for each person listing what they're in across your libraries.
- **Playback.**
  - The server picks direct play, remux or transcode for each client and
    explains why.
  - Direct play uses HTTP range requests.
  - HLS VOD playlists are seekable anywhere; ffmpeg restarts at the exact
    segment.
  - Remux copies H.264 without re-encoding. Segments line up on source
    keyframes, read from the file's own index (Matroska cues, MP4/MOV sample
    tables) the moment playback starts; other containers are indexed in the
    background.
  - Transcoding auto-detects hardware: NVENC, QSV, AMF, VAAPI or VideoToolbox,
    with libx264 as fallback. Decoding on the GPU is supported too.
  - HDR10, HLG and Dolby Vision are tone-mapped to SDR with libplacebo (GPU),
    OpenCL or zscale. Dolby Vision profile 5 is handled correctly through
    libplacebo.
  - Subtitles: text tracks (embedded and sidecar `.srt`, `.ass`, `.vtt`) are
    served as WebVTT; image subtitles (PGS, VobSub) are burned in.
  - Audio-track selection, quality presets, resume, and next-episode autoplay.
  - Skip Intro and Skip Credits: intros and credits are found from chapter
    names and by comparing the audio of episodes (Chromaprint), with Up Next
    when an episode's credits start and optional automatic intro skipping.
  - Seek-bar previews: thumbnails while hovering the timeline, made from
    keyframes in the background for whole libraries (seconds per film, HDR
    tone-mapped), and served to Plex clients as BIF indexes.
  - Chapters are marked on the seek bar and listed with pictures in a
    Chapters menu.
  - Subtitle and audio choices are remembered per show, so the next episode
    starts with the same language.
- **Watch state.**
  - Per-user progress, watched flags and play counts.
  - Watch history per user (with the device), and everyone's recent plays on
    the dashboard.
  - Star ratings (half stars) per user, and "More Like This" on movies and
    shows (shared collections, directors, cast, genres and period).
  - Continue Watching and Next Up, plus Recently Added, Recently Released and
    Top Rated hubs.
  - Full-text search.
- **Collections and playlists.**
  - Movie collections from TMDB (Harry Potter, Alien…), made automatically,
    plus collections that administrators curate, with their own order.
  - Private playlists for video and music: add a film, an episode or a whole
    show, album or artist, reorder by dragging, play in order or shuffled.
  - Smart collections and smart playlists: a library's filters ("unwatched
    comedies, newest first") saved once, which keep up with the library by
    themselves.
  - All of them respect library access and age limits.
- **Accounts.**
  - Administrators, users and managed profiles (no password, for children),
    each with their own watch state, ratings, playlists and history.
  - Library access and an age limit per user (by content rating).
  - Home profiles with a profile switcher and optional PINs, display names,
    pictures and colors.
  - Invitation links with the access they grant, signed-in devices with
    remote sign-out, password changes, disabled accounts, and login
    throttling.
  - Default audio language and subtitle mode per user (off, forced only,
    with foreign audio, always), over which a title's own choice wins.
- **Admin.**
  - Metadata editing with locked fields, Fix Match against TMDB, and
    choosing or uploading posters, backgrounds and thumbnails.
  - Library management with a folder browser.
  - Runtime settings.
  - A live dashboard: now playing with transcode details, transcoder metrics
    (fps, speed, hardware sessions, segments, disk), scans and system info.
  - Prometheus `/metrics` on the transcoder.
- **Plex-compatible API** on port 32400 for Plex tools such as python-plexapi,
  Tautulli and Kometa: libraries with Plex's filters, sorts and paging,
  metadata with provider guids, markers and "More Like This", editing
  (fields and locks, genres, labels, collections, posters), collections and
  playlists (smart ones too), play queues, watch state, sessions and the
  notification socket, direct play and HLS streams. See
  [docs/PLEX.md](docs/PLEX.md).
- **Web app.** Works on phones, tablets and desktops, and installs as an app
  from the browser.
- **Storage.** Downloaded and uploaded pictures are stored once by content,
  files generated from media (frame grabs, previews, chapter pictures) follow
  their file through moves and renames, unused files are cleaned up, and the
  database is backed up daily. See [docs/STORAGE.md](docs/STORAGE.md).
- **Performance.**
  - Single binaries.
  - SQLite in WAL mode with a dedicated writer connection, and a mimalloc
    allocator.
  - Session cache in memory.
  - Library listings are filtered, sorted and paged in SQL, and the library
    page loads its first screenful on its own. API responses are compressed
    with zstd (or Brotli/gzip for clients without it).
  - Images resized once per size bucket and cached with immutable cache
    headers.
  - Precompressed (Brotli and gzip) UI assets embedded in the server.
  - Transcodes throttled to about 2× realtime after the initial buffer, with
    idle processes stopped automatically.

## Architecture

```
 browser ── HTTP ──▶ pley (server)                               pley-transcoder
   React UI            libraries · scanner · metadata · auth      ffprobe/ffmpeg · HLS jobs · tone mapping
   hls.js              sessions · hubs · search · /api            thumbnails · subtitles · keyframes · /metrics
                       SQLite (WAL)          ── HTTP/JSON ──▶      spawned & supervised by the server
```

The transcoder is a separate program, and the server talks to it only over
HTTP. That keeps the server's source closed while the transcoder can be
published. See [LICENSING.md](LICENSING.md),
[docs/TRANSCODER.md](docs/TRANSCODER.md) and [docs/API.md](docs/API.md).

The repository is a monorepo: a Cargo workspace for the Rust programs and an
npm workspace for the web UI, both rooted at the top level.

| Path | What |
|---|---|
| `apps/server` | The media server (crate `pley-server`, binary `pley`) |
| `apps/transcoder` | The FFmpeg engine (crate and binary `pley-transcoder`) |
| `apps/web` | React + TypeScript + Vite + Tailwind web UI, embedded into the server |
| `packages/transcoder-api` | Protocol types shared by server and transcoder |
| `deploy/` | Docker image and compose file, systemd unit, launchd agent ([guide](deploy/README.md)) |
| `docs/` | HTTP API, transcoder protocol |
| `scripts/` | Release build, license check |
| `.github/` | CI on Linux, macOS and Windows; release builds for x86_64 and arm64; Dependabot |

## Requirements

- Rust 1.85 or newer (edition 2024).
- Node.js 22 or newer, to build the web UI (22.22+ or 24.15+ to run its
  tests).
- FFmpeg and ffprobe on `PATH`, or configured. Version 6.1 or newer is
  recommended for transcode throttling. For hardware transcoding and GPU tone
  mapping, use a build with NVENC/QSV/AMF/VAAPI and libplacebo, such as the
  gyan.dev "full" build on Windows or jellyfin-ffmpeg on Linux.

## Quick start

```powershell
npm install; npm run build                       # build the UI (embedded into the server)
cargo build                                      # builds pley + pley-transcoder
.\target\debug\pley.exe --data-dir .\data        # http://localhost:7539
```

On first launch, open pley on the computer it runs on (or on your home
network) and create the administrator account, then add a library: pick a
type and one or more folders. From anywhere else, for example a server
reached over the internet or through a reverse proxy, setting up also asks
for the one-time code pley prints in its log when it starts, so nobody else
can claim a new server. Passwords need at least 8 characters and can't be
common ones.

To recover a lost password, use
`pley reset-password <user> <new-password> [--admin]`.

### Development

- `npm run dev` runs the UI with hot reload on `http://localhost:5173`,
  proxying `/api` to the server on port 7539.
- Debug builds of the server read `apps/web/dist` from disk, so
  `npm run build` shows up without recompiling Rust.
- Tests: `cargo test --workspace`. They cover parsers, decision logic, ffmpeg
  argument building, plus HTTP API end-to-end tests. Metadata providers are
  tested against a local stand-in; `cargo test -p pley-server --test providers
  -- --ignored` checks the real TVmaze and MusicBrainz (and TheTVDB with
  `PLEY_TEST_TVDB_KEY`, plus `PLEY_TEST_TVDB_PIN` if the key needs one).
- Web UI: `npm test` runs its tests (Vitest, Testing Library, jsdom; next to
  the code as `*.test.ts(x)`), `npm run check` runs the type check, ESLint and
  the tests, as CI does. `npm run coverage -w apps/web` writes a coverage
  report.
- Lint: `cargo clippy --workspace --all-targets`, `cargo fmt --all` and
  `npm run lint` (ESLint with type-aware rules; TypeScript is strict, with
  `noUncheckedIndexedAccess`).
- License boundary: `powershell -File scripts/check-licenses.ps1`.

### Continuous integration

GitHub Actions runs on every push to `main` and every pull request
([`ci.yml`](.github/workflows/ci.yml)):

- **Web UI**: type check, ESLint, tests, production build.
- **Rust** on Linux (x86_64, arm64), Windows and macOS: rustfmt, Clippy with
  warnings as errors, tests. Another job builds with Rust 1.85, the minimum
  supported version.
- **License boundary**: the server never depends on GPL code, third-party
  licenses are allowed by [`deny.toml`](deny.toml).
- **Docker image** builds, and the workflows themselves pass actionlint.

Actions are pinned to commits; Dependabot proposes weekly updates for them,
Cargo, npm and the Docker base images.

### Release

Push a version tag (`git tag v0.2.0 && git push origin v0.2.0`) and
[`release.yml`](.github/workflows/release.yml) runs CI, then:

- builds archives for Linux, Windows (x86_64, arm64) and macOS (Apple
  silicon, Intel), with `SHA256SUMS` and build provenance
  (`gh attestation verify <archive> --repo <owner>/<repo>`),
- drafts a GitHub release with them and generated notes, to read over and
  publish,
- publishes the Docker image for amd64 and arm64 to GitHub Container Registry
  as `ghcr.io/<owner>/<repo>`, tagged `0.2.0`, `0.2` and `latest`
  (pre-releases like `v0.3.0-rc.1` only get their own tag).

To build an archive locally:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build.ps1     # or ./scripts/build.sh
```

This produces `dist/pley-<version>-<os>-<arch>/` containing:

- both binaries
- the license files

## Configuration

Static options come from command-line flags, `PLEY_*` environment variables,
or `pley.toml` in the data directory; see
[pley.example.toml](pley.example.toml). The main ones:

| Flag | Default | |
|---|---|---|
| `--bind` | `0.0.0.0:7539` | listen address (port spells PLEY on a keypad) |
| `--plex-bind` | `0.0.0.0:32400` | Plex-compatible API, `off` to disable |
| `--data-dir` | `%LOCALAPPDATA%\pley` / `~/.local/share/pley` | database, caches, artwork |
| `--ffmpeg`, `--ffprobe` | from `PATH` | passed to the transcoder |
| `--transcoder-url` | (managed) | use an external transcoder instead of spawning one |
| `--web-dir` | (embedded) | serve the UI from a folder |
| `--log` | `info` | e.g. `debug`, `info,pley=debug`; also written to `logs/pley.log` in the data directory |
| `--no-tray` | off | never show the tray icon (it only appears in interactive desktop sessions) |

pley shuts down cleanly (playback in progress gets a few seconds, then the
transcoder stops and the database closes) on Ctrl+C, `SIGTERM` (Docker,
systemd), the tray's Quit, and on Windows also on `taskkill` without `/F`,
when its console window is closed, and when you sign out or shut down.

Runtime settings are edited in the UI under **Settings → Server**:

- server name
- metadata sources: TMDB API key, TV source, TheTVDB key and PIN,
  MusicBrainz, metadata language
- hardware acceleration, hardware decoding, tone mapping
- maximum transcodes, encoder preset, throttling
- scan interval, watching folders for changes

## Metadata sources

| Library | Source | Key |
|---|---|---|
| Movies | [TMDB](https://www.themoviedb.org) | free API key |
| TV shows | TMDB, [TheTVDB](https://thetvdb.com) or [TVmaze](https://www.tvmaze.com) | TMDB: its key. TheTVDB: an API key, plus your subscriber PIN for user-supported keys. TVmaze: none |
| Music | [MusicBrainz](https://musicbrainz.org) and the [Cover Art Archive](https://coverartarchive.org) | none |

- The TV source is chosen under **Settings → Server → Metadata**.
  *Automatic* uses TMDB when it has a key, then TheTVDB when it has one,
  else TVmaze. Changing the source fetches every show again.
- Movies stay with TMDB: no keyless source has movie artwork. Without a key,
  movies keep their file names and local artwork.
- TVmaze has English data and no content ratings. It files specials under
  the season they aired in, without a number; pley numbers them in air date
  order as season 0 (`S00E01`, `S00E02`, …).
- MusicBrainz fills in what the tags lack and never renames artists or
  albums; an album's own cover (a file next to it or one embedded in its
  tracks) wins over the Cover Art Archive's. Files tagged with MusicBrainz
  Picard are matched by their MusicBrainz ids. MusicBrainz allows a request
  a second, so the first look-up of a large music library takes a while.
- Local artwork always wins, and hand edits stay through every refresh.

### Attribution

- This product uses the TMDB API but is not endorsed or certified by TMDB.
- TV data from [TVmaze](https://www.tvmaze.com) is licensed
  [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
- TV metadata from [TheTVDB](https://thetvdb.com). Please consider adding
  missing information or subscribing.
- Music data from [MusicBrainz](https://musicbrainz.org) is public domain
  (CC0); covers come from the [Cover Art Archive](https://coverartarchive.org)
  and belong to their owners.

## Status

Version 0.1, the first full vertical slice.

## Trademarks

pley is an independent project. It is not affiliated with, endorsed by, or
connected to Plex Inc. "Plex" is a trademark of Plex Inc. and is used here only
to describe API compatibility.
