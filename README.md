# pley

pley is a self-hosted media server in the spirit of Plex. It organizes your
movies, TV shows and music and plays them in any browser: directly when the
browser can, transcoded on the fly when it can't, with hardware encoding and
HDR10, HLG and Dolby Vision tone mapping. The server is written in Rust, the
web app in React and TypeScript.

## Features

**Libraries**

- Movies, TV shows and music, scanned incrementally and rescanned when files
  change. A drive that is offline isn't mistaken for deleted media.
- File names are read the way Plex names them: `Movie (2019)`, release names,
  `{tmdb-123}` hints, editions (`{edition-Director's Cut}`), `S01E02`, `1x02`,
  date-based and anime episodes, and extras (`Trailers/` folders,
  `Movie (2019)-trailer.mkv`).

**Metadata**

- Movies from TMDB (free API key). TV shows from TMDB, TheTVDB or TVmaze,
  which needs no key. Music from MusicBrainz and the Cover Art Archive, no key.
- Local artwork and hand edits always win. Cast and crew get their own pages.

**Playback**

- Direct play, remux or transcode, chosen for each device, with the reason
  shown. One quality menu: the original, or a lower quality that transcodes.
- Hardware encoding (NVENC, Quick Sync, AMF, VAAPI, VideoToolbox) and GPU tone
  mapping with libplacebo, including Dolby Vision profile 5.
- Subtitles (text tracks as WebVTT, image tracks burned in), audio tracks and
  default languages per user. A choice carries over to the next episode.
- Resume, Up Next, Skip Intro and Skip Credits (from chapters, and by
  comparing the audio of episodes), seek-bar previews and chapter pictures.

**Watching**

- Progress, history, star ratings, Continue Watching and Next Up for each
  user.
- Recently Added, Recently Released, Top Rated, More Like This and full-text
  search.
- Collections (from TMDB, curated, or smart) and playlists (ordered or smart).

**Accounts**

- Administrators, users and managed profiles for children, each with their
  own libraries and age limit.
- A profile switcher with optional PINs, invitation links, and a list of
  signed-in devices that can be signed out remotely.
- Optional two-step sign-in with an authenticator app (scan a QR code), with
  recovery codes for a lost phone.
- Apps sign in on pley's own page and get only what you allow (browsing,
  playing, playlists), never administration: OAuth 2.1 with client metadata
  documents. pley's own apps work without internet access; apps by other
  publishers are off until an administrator allows them. See
  [docs/API.md](docs/API.md#apps-oauth-21).
- First-run setup with a one-time code from the server's log, which the
  tray icon fills in. Passwords have at least 8 characters and can't be
  common ones. See [SECURITY.md](SECURITY.md).

**Administration**

- Metadata editing with locked fields, Fix Match and a choice of artwork.
- Settings with a Test button for each API key and service, a live dashboard
  (playback, transcoder, scans) and daily database backups.

**Plex-compatible API** on port 32400, for python-plexapi, Tautulli and
Kometa: browsing with Plex's filters, editing, collections, playlists, play
queues, sessions and notifications. See [docs/PLEX.md](docs/PLEX.md).

**Web app** for phones, tablets and desktops, which installs as an app from
the browser.

**Apps**: pley for Windows, macOS and Linux, and pley for Android. Both find
servers on the local network by themselves (or take an address) and show the
server's web app; the Android app plays videos in its own player, which
plays more files as they are than a browser.

**Performance**: single binaries; SQLite in WAL mode with one writer;
listings filtered, sorted and paged in SQL; compressed responses; pictures
resized once and cached; transcodes throttled to about twice real time.

## Architecture

```
 browser ── HTTP ──▶ pley (server)                               pley-transcoder
   React UI            libraries · scanner · metadata · auth      ffprobe/ffmpeg · HLS jobs · tone mapping
   hls.js              sessions · hubs · search · /api            thumbnails · subtitles · keyframes · /metrics
                       SQLite (WAL)          ── HTTP/JSON ──▶      spawned & supervised by the server
```

The transcoder is a separate program and the server talks to it only over
HTTP, so the two can have different licenses
([LICENSING.md](LICENSING.md)).

| Path | What |
|---|---|
| `apps/server` | The media server (crate `pley-server`, binary `pley`) |
| `apps/transcoder` | The FFmpeg engine (crate and binary `pley-transcoder`) |
| `apps/web` | The web app (React, TypeScript, Vite, Tailwind), embedded into the server |
| `apps/desktop` | pley for Windows, macOS and Linux (Tauri) |
| `apps/android` | pley for Android (Kotlin, Compose, Media3) |
| `packages/transcoder-api` | Protocol types shared by server and transcoder |
| `deploy/` | Docker, systemd and launchd setups ([guide](deploy/README.md)) |
| `docs/` | [HTTP API](docs/API.md), [Plex API](docs/PLEX.md), [transcoder](docs/TRANSCODER.md), [storage](docs/STORAGE.md) |
| `scripts/` | Release build, transcoder source package, license check |

## Requirements

- Rust 1.85 or newer.
- Node.js 22 or newer to build the web app (22.22+ or 24.15+ to run its
  tests).
- FFmpeg and ffprobe on `PATH` or configured; 6.1 or newer throttles
  transcodes. For hardware transcoding and GPU tone mapping, use a build with
  NVENC, QSV, AMF or VAAPI and libplacebo, such as gyan.dev's "full" build on
  Windows or jellyfin-ffmpeg on Linux.

## Quick start

```powershell
npm install; npm run build                       # build the web app (embedded into the server)
cargo build                                      # build pley and pley-transcoder
.\target\debug\pley.exe --data-dir .\data        # http://localhost:7539
```

pley prints a link with a one-time setup code when it starts
(`http://localhost:7539/setup#code=…`; on a desktop the tray icon opens it).
Open it, create the administrator account and add a library. Nobody without
the code can claim a new server.

A lost password is reset with `pley reset-password <user> <new-password>
[--admin]`, which also turns that account's two-step sign-in off.

## Configuration

Options come from command-line flags, `PLEY_*` environment variables or
`pley.toml` in the data folder ([example](pley.example.toml)):

| Flag | Default | |
|---|---|---|
| `--bind` | `0.0.0.0:7539` | Listen address (7539 spells PLEY on a keypad) |
| `--plex-bind` | `0.0.0.0:32400` | Plex-compatible API; `off` turns it off |
| `--data-dir` | `%LOCALAPPDATA%\pley`, `~/.local/share/pley` | Database, caches and artwork |
| `--ffmpeg`, `--ffprobe` | from `PATH` | Passed to the transcoder |
| `--transcoder-url` | | Use a transcoder running elsewhere instead of starting one |
| `--web-dir` | | Serve the web app from a folder instead of the embedded copy |
| `--log` | `info` | Log filter, e.g. `info,pley=debug`; also written to `logs/pley.log` |
| `--no-tray` | | Never show the tray icon |
| `--no-discovery` | | Don't answer the apps looking for servers on the local network |

Everything else (server name, metadata sources and keys, hardware
acceleration, tone mapping, transcode limits, scanning) is set in the web app
under **Settings → Server**.

pley stops cleanly, giving playback a few seconds to finish, on Ctrl+C,
`SIGTERM`, the tray's Quit and, on Windows, `taskkill` without `/F`, closing
its console window or signing out.

## Metadata sources

| Library | Source | Key |
|---|---|---|
| Movies | [TMDB](https://www.themoviedb.org) | Free API key |
| TV shows | TMDB, [TheTVDB](https://thetvdb.com) or [TVmaze](https://www.tvmaze.com) | TMDB's key; TheTVDB's key (and subscriber PIN for user-supported keys); none for TVmaze |
| Music | [MusicBrainz](https://musicbrainz.org) and the [Cover Art Archive](https://coverartarchive.org) | None |

- *Automatic* uses TMDB for TV when it has a key, then TheTVDB, else TVmaze.
  Changing the source fetches every show again.
- Without a TMDB key, movies keep their file names and local artwork: no
  keyless source has movie artwork.
- TVmaze is in English and has no content ratings. Its specials are numbered
  as season 0 in air date order.
- MusicBrainz only fills in what the tags lack and allows one request a
  second, so the first look-up of a big music library takes a while.

This product uses the TMDB API but is not endorsed or certified by TMDB. TV
data from TVmaze is licensed
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). TV metadata
from TheTVDB; please consider adding missing information or subscribing.
Music data from MusicBrainz is public domain (CC0); covers from the Cover Art
Archive belong to their owners.

## Development

- `npm run dev` serves the web app with hot reload on `http://localhost:5173`,
  proxying `/api` to the server on port 7539. Debug builds of the server read
  `apps/web/dist` from disk, so `npm run build` shows up without recompiling.
- `cargo test --workspace` runs the Rust tests, including end-to-end tests of
  both APIs. `cargo test -p pley-server --test providers -- --ignored` also
  checks the real TVmaze and MusicBrainz (and TheTVDB with
  `PLEY_TEST_TVDB_KEY` and, if needed, `PLEY_TEST_TVDB_PIN`).
- `npm run check` runs the web app's type check, ESLint and tests (Vitest),
  and the desktop app's.
- `npm run desktop` runs the desktop app (Rust, and on Linux WebKitGTK:
  `libwebkit2gtk-4.1-dev`); `npm run build -w apps/desktop` makes its
  installers.
- The Android app builds with the Android SDK and JDK 17 or newer:
  `./gradlew testDebugUnitTest assembleDebug` in `apps/android`, then
  `adb install app/build/outputs/apk/debug/app-debug.apk`.
- `cargo clippy --workspace --all-targets` and `cargo fmt --all` lint and
  format the Rust code; `scripts/check-licenses.ps1` checks the license
  boundary.

[GitHub Actions](.github/workflows/ci.yml) runs all of this on Linux, macOS and
Windows for every push and pull request, plus a build with the oldest
supported Rust, the license check and the Docker build. Pushing a tag such as
`v0.2.0` runs [`release.yml`](.github/workflows/release.yml): archives for
Linux, Windows and macOS (x86_64 and arm64) with checksums and build
provenance, a draft GitHub release, and the Docker image on GitHub Container
Registry. `scripts/build.ps1` (or `build.sh`) builds an archive locally.

## Status

Version 0.1.

## Trademarks

pley is an independent project, not affiliated with or endorsed by Plex Inc.
"Plex" is a trademark of Plex Inc., used here only to describe API
compatibility.
