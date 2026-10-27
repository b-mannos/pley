# pley

pley is a self-hosted media server. It organizes your
movies, TV shows and music and plays them in any browser: directly when the
browser can, transcoded on the fly when it can't, with hardware encoding and
HDR10, HLG and Dolby Vision tone mapping. The server is written in Rust, the
web app in React and TypeScript.

## Features

**Libraries**

- Movies, TV shows and music, scanned incrementally and rescanned when files
  change. A drive that is offline isn't mistaken for deleted media.
- File names are read the way media servers name them: `Movie (2019)`, release names,
  `{tmdb-123}` hints, editions (`{edition-Director's Cut}`), `S01E02`, `1x02`,
  date-based and anime episodes, and extras (`Trailers/` folders,
  `Movie (2019)-trailer.mkv`). Looser episode names work too: `04 - Title` or
  `104 - Title` in a `Season 1` folder, `Season 1 Episode 4`, and a title
  alone, which TMDB numbers by its title; no two files ever merge into one
  episode for lack of a number.

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
  publishers are off until an administrator allows them. On a TV, pley's
  app shows a code to type on pley's `/link` page. See
  [docs/API.md](docs/API.md#apps-oauth-21).
- First-run setup with a one-time code from the server's log, which the
  tray icon fills in. Passwords have at least 8 characters and can't be
  common ones. See [SECURITY.md](SECURITY.md).

**Administration**

- Metadata editing with locked fields, Fix Match and a choice of artwork.
- Settings with a Test button for each API key and service, a live dashboard
  (playback, transcoder, scans) and daily database backups.

**Compatibility API** on port 32400, for `python-plexapi`, Tautulli and
Kometa: browsing with their filters, editing, collections, playlists, play
queues, sessions and notifications. See
[docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

**Web app** for phones, tablets and desktops, which installs as an app from
the browser.

**Apps**: pley for Windows, macOS and Linux, which finds servers on the
local network by itself (or takes an address) and shows the server's web
app; and pley for Android, a native app for phones, tablets, Android TV,
Google TV and Fire TV, with its own player, which plays more files as they
are than a browser, music in the background, and a remote-first interface
on TVs. It signs in through the server's own page or, on a TV, with a code;
installing it is in [docs/ANDROID.md](docs/ANDROID.md). Building and
installing them is in [docs/TV_PLATFORMS.md](docs/TV_PLATFORMS.md).

**Offline**: on the home network everything works without the internet.
Item pages download a file's original, which the Android app (phones and
tablets) and the desktop app keep to play while the server is away; the
Android app sends what was watched once it is back.

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
| `apps/android` | pley for Android phones, tablets and TVs (Kotlin, Compose, Media3) |
| `apps/kodi` | pley for Kodi, an add-on (Python, GPL-2.0-or-later) |
| `apps/roku` | pley for Roku (BrightScript, SceneGraph) |
| `apps/tizen`, `apps/webos` | pley for Samsung TV and pley for LG TV |
| `packages/tv` | The TV app both share (TypeScript, Preact) |
| `packages/transcoder-api` | Protocol types shared by server and transcoder |
| `deploy/` | Docker, systemd and launchd setups ([guide](deploy/README.md)) |
| `docs/` | [HTTP API](docs/API.md), [compatibility API](docs/COMPATIBILITY.md), [transcoder](docs/TRANSCODER.md), [storage](docs/STORAGE.md), [Samsung and LG TVs](docs/TV_PLATFORMS.md) |
| `scripts/` | Release build, transcoder source package, license check, the web and desktop icons (`render-icons.mjs`) |

## Requirements

- Rust 1.95 or newer.
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
(`http://localhost:7539/setup?code=…`; on a desktop the tray icon opens it).
Open it, create the administrator account and add a library. Nobody without
the code can claim a new server.

A lost password is reset with `pley reset-password <user> <new-password>
[--admin]`, which also turns that account's two-step sign-in off.

To run it on a home server instead, `deploy/docker/setup.sh` builds and
starts it in Docker, locked down, for a reverse proxy in front of it
([guide](deploy/docker/README.md)). On Windows, the release's
`pley-server-…-setup.exe` installs it as a locked-down Windows service
([guide](deploy/windows/README.md)).

## Configuration

Options come from command-line flags, `PLEY_*` environment variables or
`pley.toml` in the data folder ([example](pley.example.toml)):

| Flag | Default | |
|---|---|---|
| `--bind` | `0.0.0.0:7539` | Listen address (7539 spells PLEY on a keypad); several separated by commas |
| `--plex-bind` | `0.0.0.0:32400` | Compatibility API; `off` turns it off; several as with `--bind` |
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
- The Android app builds with the Android SDK and JDK 17 or newer (on
  Windows, Android Studio's: `JAVA_HOME=C:\Program Files\Android\Android
  Studio\jbr`). In `apps/android`, `./gradlew testDebugUnitTest lintDebug
  assembleDebug assembleStoreRelease` tests, lints and builds every module,
  as CI does, then `pwsh ../../scripts/check-android.ps1` checks the
  release's libraries, native code and size. Builds come in two flavors,
  `store` and `direct`: `adb install
  app/build/outputs/apk/store/debug/app-store-debug.apk` installs the debug
  app. [docs/ANDROID.md](docs/ANDROID.md) has the modules and the rest.
- `npm run tv:check` type-checks and tests the Samsung and LG TV apps;
  `npm run tizen` and `npm run webos` package them with Samsung's and LG's
  command-line tools ([docs/TV_PLATFORMS.md](docs/TV_PLATFORMS.md)).
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
The release also carries the server's Windows installers, the Android app (an
APK to sideload and a bundle for Google Play, signed with the key from the
repository's secrets), the desktop app's installers, the LG TV app, the Kodi
add-on and the Roku app.

Every app has one version, pley's, in [`VERSION`](VERSION):
`node scripts/version.mjs set 0.3.0` writes it into every manifest, and
`node scripts/version.mjs check` (which CI runs) finds any that disagree. A
release's tag is that version (`v0.3.0`). Builds record the commit they
were made from, which every app shows beside its version, as
"0.2.0 (1a2b3c4d5)": `pley --version`, the web app's Account page and
dashboard, and the apps' Settings.

## Status

Version 0.2.
