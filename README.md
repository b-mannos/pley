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
    `{tmdb-123}` hints.
  - Episodes: `S01E02`, `1x02`, multi-episode files, season folders, specials,
    date-based shows and anime numbering.
  - Music: tags first, then folders.
- **Metadata.**
  - TMDB agent for posters, backdrops, summaries, ratings, certifications,
    genres, studios, and season and episode details. It's optional and needs an
    API key.
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
    keyframes from a background keyframe index.
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
    keyframes in the background (seconds per film, HDR tone-mapped).
  - Chapters are marked on the seek bar and listed in a Chapters menu.
  - Subtitle and audio choices are remembered per show, so the next episode
    starts with the same language.
- **Watch state.**
  - Per-user progress, watched flags and play counts.
  - Continue Watching and Next Up, plus Recently Added, Recently Released and
    Top Rated hubs.
  - Full-text search.
- **Admin.**
  - Metadata editing with locked fields, Fix Match against TMDB, and
    choosing or uploading posters, backgrounds and thumbnails.
  - Users and roles.
  - Library management with a folder browser.
  - Runtime settings.
  - A live dashboard: now playing with transcode details, transcoder metrics
    (fps, speed, hardware sessions, segments, disk), scans and system info.
  - Prometheus `/metrics` on the transcoder.
- **Plex-compatible API** on port 32400 for Plex tools such as python-plexapi
  and Tautulli: libraries, metadata, watch state, sessions, direct play and HLS
  streams. See [docs/PLEX.md](docs/PLEX.md).
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
| `.github/workflows/` | CI on Linux, macOS and Windows; release builds for x86_64 and arm64 |

## Requirements

- Rust 1.85 or newer (edition 2024).
- Node.js 20 or newer, to build the web UI.
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

On first launch the UI asks you to create the administrator account and then
add a library: pick a type and one or more folders.

To recover a lost password, use
`pley reset-password <user> <new-password> [--admin]`.

### Development

- `npm run dev` runs the UI with hot reload on `http://localhost:5173`,
  proxying `/api` to the server on port 7539.
- Debug builds of the server read `apps/web/dist` from disk, so
  `npm run build` shows up without recompiling Rust.
- Tests: `cargo test --workspace`. They cover parsers, decision logic, ffmpeg
  argument building, plus HTTP API end-to-end tests.
- Lint: `cargo clippy --workspace --all-targets`, `cargo fmt --all` and
  `npm run typecheck`.
- License boundary: `powershell -File scripts/check-licenses.ps1`.

### Release

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

Runtime settings are edited in the UI under **Settings → Server**:

- server name, TMDB API key and metadata language
- hardware acceleration, hardware decoding, tone mapping
- maximum transcodes, encoder preset, throttling
- scan interval, watching folders for changes

## Status

Version 0.1, the first full vertical slice.

## Trademarks

pley is an independent project. It is not affiliated with, endorsed by, or
connected to Plex Inc. "Plex" is a trademark of Plex Inc. and is used here only
to describe API compatibility.
