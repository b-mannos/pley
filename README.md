# Pley

Pley is a self-hosted media server. It organizes your movies, TV shows and
music and plays them on your devices: directly when a device can play the
file, transcoded on the fly when it can't, with hardware encoding and HDR10,
HLG and Dolby Vision tone mapping. The server is written in Rust, the web app
in React and TypeScript.

- [Features](#features)
- [Apps](#apps)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Documentation](#documentation)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Features

**Libraries**

- Movies, TV shows and music, scanned incrementally and rescanned when files
  change. An offline drive isn't mistaken for deleted media.
- Common naming schemes: `Movie (2019)`, release names, `{tmdb-123}` hints,
  editions (`{edition-Director's Cut}`), `S01E02`, `1x02`, date-based and
  anime episodes, and extras (`Trailers/` folders,
  `Movie (2019)-trailer.mkv`). Looser episode names work too: `04 - Title` or
  `104 - Title` in a `Season 1` folder, `Season 1 Episode 4`, or a title
  alone, which TMDB numbers by its title. Two files never merge into one
  episode for lack of a number.
  [deploy/docker/media](deploy/docker/media/README.md) shows a layout.
- Each profile can order its libraries, on every app.

**Metadata**

- Movies from TMDB (free API key); TV shows from TMDB, TheTVDB or TVmaze (no
  key); music from MusicBrainz and the Cover Art Archive (no key). See
  [Metadata sources](#metadata-sources).
- Local artwork and hand edits always win. Cast and crew have their own
  pages.

**Playback**

- Direct play, remux or transcode, chosen per device, with the reason shown.
  One Quality menu: Original, Automatic, or a lower quality
  ([docs/QUALITY.md](docs/QUALITY.md)).
- Hardware encoding (NVENC, Quick Sync, AMF, VAAPI, VideoToolbox) and GPU
  tone mapping with libplacebo, including Dolby Vision profile 5.
- Dolby Vision profile 7 (FEL or MEL) and HDR10+ are told apart from plain
  HDR10 and shown as badges; the player's details say what the screen gets.
- Subtitles: text tracks as WebVTT; picture tracks (PGS, VobSub, DVB)
  drawn over the video by the web, desktop, Android, Samsung and LG apps
  (PGS by Kodi too), so the file still plays as it is, and burned in
  elsewhere; a timing offset in every player, the TV apps included.
- Audio and subtitle defaults per profile, on every app; a choice carries
  over to the next episode. Among tracks in the profile's language, the
  server picks the one the device plays best (as is rather than converted;
  the 5.1 track for a player that passes it to a receiver).
- Resume, Up Next, Skip Intro and Skip Credits (from chapters, and by
  comparing the audio of episodes), seek-bar previews and chapter pictures.

**Watching**

- Progress, history, star ratings, Continue Watching and Next Up per user.
- Recently Added, Recently Released, Top Rated, More Like This and full-text
  search.
- Collections (from TMDB, curated, or smart) and playlists (ordered or smart).

**Accounts**

- Administrators, users and managed profiles for children, each with their
  own libraries and age limit.
- A profile switcher with optional PINs, invitation links, and a list of
  signed-in devices that can be signed out remotely.
- Optional two-step sign-in with an authenticator app, with recovery codes.
- Apps sign in on Pley's own page and get only what you allow (browsing,
  playing, playlists), never administration: OAuth 2.1 with client metadata
  documents. Pley's own apps work without internet access; apps by other
  publishers are off until an administrator allows them. TVs show a code to
  type on Pley's `/link` page ([docs/API.md](docs/API.md#apps-oauth-21)).
- First-run setup needs a one-time code from the server's log, which the tray
  icon fills in. Passwords have at least 8 characters and can't be common
  ones ([SECURITY.md](SECURITY.md)).

**Administration**

- Metadata editing with locked fields, Fix Match and a choice of artwork.
- Settings with a Test button for each API key and service, a live dashboard
  (playback, transcoder, scans, and charts of Pley's and the computer's CPU
  and memory) and daily database backups ([docs/STORAGE.md](docs/STORAGE.md)).

**Compatibility API** on Pley's own port (or 32400) for `python-plexapi`,
Tautulli and Kometa: browsing with their filters, editing, collections,
playlists, play queues, sessions and notifications
([docs/COMPATIBILITY.md](docs/COMPATIBILITY.md)).

**Offline**: on the home network everything works without the internet. Item
pages download a file's original, which the Android app (phones and tablets)
and the desktop app keep to play while the server is away; the Android app
reports what was watched once it is back.

**Performance**: single binaries; SQLite in WAL mode with one writer;
listings filtered, sorted and paged in SQL; compressed responses; pictures
resized once and cached; transcodes throttled to about twice real time.

## Apps

| App | Platforms | Docs |
|---|---|---|
| Web app | Any modern browser; installs as an app on phones, tablets and desktops | |
| Pley for Windows, macOS and Linux | Desktop (Tauri). Finds servers on the local network (or takes an address) and shows the server's web app | |
| Pley for Android | Phones, tablets, Android TV, Google TV, Fire TV. Native, with its own player: plays more files as they are than a browser, music in the background, a remote-first interface on TVs | [docs/ANDROID.md](docs/ANDROID.md) |
| Pley for Kodi | Kodi 20 and newer (LibreELEC, CoreELEC, Android TV boxes, computers). The Android TV app's screens, Kodi's own player, Skip Intro and Up Next | [docs/KODI.md](docs/KODI.md) |
| Pley for Roku | Roku. Native; sideloaded for now | [docs/ROKU.md](docs/ROKU.md) |

The TV apps sign in with a code typed on the server's `/link` page; the
others sign in on the server's own page.

## Quick start

**Docker** (Linux home server, behind a reverse proxy):

```sh
git clone https://github.com/hhaidar/pley.git
cd pley/deploy/docker && ./setup.sh
```

[deploy/docker/README.md](deploy/docker/README.md) is the full guide.

**Windows**: run the release's `pley-server-<version>-windows-<arch>-setup.exe`,
which installs Pley as a locked-down Windows service or in the notification
area ([deploy/windows/README.md](deploy/windows/README.md)).

**Linux and macOS services, and release archives**: see
[deploy/README.md](deploy/README.md).

**From source** (needs the tools under [Requirements](#requirements)):

```powershell
bun install; bun run build                       # build the web app (embedded into the server)
cargo build                                      # build pley and pley-transcoder
.\target\debug\pley.exe --data-dir .\data        # http://localhost:7539
```

On start, Pley prints a link with a one-time setup code
(`http://localhost:7539/setup?code=…`; on a desktop the tray icon opens it).
Open it, create the administrator account and add a library. Nobody without
the code can claim a new server.

`pley reset-password <user> [--admin]` resets a lost password. It asks for
the new one (or reads it from standard input) and turns that account's
two-step sign-in off.

## Configuration

Options come from command-line flags, `PLEY_*` environment variables, or
`pley.toml` in the data folder ([example](pley.example.toml)). For example,
`--plex-bind` is also `PLEY_PLEX_BIND` and `plex_bind`.

| Flag | Default | |
|---|---|---|
| `--bind` | `0.0.0.0:7539` | Listen address (7539 spells PLEY on a keypad); several separated by commas |
| `--plex-bind` | `web` (Pley's own port) | Compatibility API: `off`, or addresses of its own (`0.0.0.0:32400`), several as with `--bind` |
| `--data-dir` | `%LOCALAPPDATA%\pley`, `~/Library/Application Support/pley`, `~/.local/share/pley` | Database, caches and artwork ([docs/STORAGE.md](docs/STORAGE.md)) |
| `--ffmpeg`, `--ffprobe` | from `PATH` | Passed to the transcoder |
| `--transcoder-url` | | Use a transcoder running elsewhere instead of starting one |
| `--web-dir` | | Serve the web app from a folder instead of the embedded copy |
| `--log` | `info` | Log filter, e.g. `info,pley=debug`; also written to `logs/pley.log` |
| `--no-tray` | | Never show the tray icon |
| `--no-discovery` | | Don't answer apps looking for servers on the local network |

Everything else is set in the web app:

- **Settings → Server**: the server name, metadata sources and keys, scanning.
- **Settings → Transcoder**: hardware acceleration, tone mapping and transcode
  limits.
- **Settings → Network**: where Pley listens (networks, port, the
  compatibility API, discovery), changed while Pley runs and saved in
  `pley.toml`, unless `--bind`, `--plex-bind` or `--no-discovery` (or their
  `PLEY_*` variables) set it.

Pley stops cleanly, giving playback a few seconds to finish, on Ctrl+C,
`SIGTERM`, the tray's Quit and, on Windows, `taskkill` without `/F`, closing
its console window, or signing out.

### Metadata sources

| Library | Source | Key |
|---|---|---|
| Movies | [TMDB](https://www.themoviedb.org) | Free API key |
| TV shows | TMDB, [TheTVDB](https://thetvdb.com) or [TVmaze](https://www.tvmaze.com) | TMDB's key; TheTVDB's key (and subscriber PIN for user-supported keys); none for TVmaze |
| Music | [MusicBrainz](https://musicbrainz.org) and the [Cover Art Archive](https://coverartarchive.org) | None |

- *Automatic* uses TMDB for TV when it has a key, then TheTVDB, else TVmaze.
  Changing the source fetches every show again.
- Without a TMDB key, movies keep their file names and local artwork: no
  keyless source has movie artwork.
- TVmaze is in English and has no content ratings. Its specials are season 0,
  in air date order.
- MusicBrainz only fills in what the tags lack and allows one request a
  second, so the first look-up of a big music library takes a while.

This product uses the TMDB API but is not endorsed or certified by TMDB. TV
data from TVmaze is licensed
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). TV metadata
from TheTVDB; please consider adding missing information or subscribing.
Music data from MusicBrainz is public domain (CC0); covers from the Cover Art
Archive belong to their owners.

## Documentation

| Doc | What |
|---|---|
| [deploy/README.md](deploy/README.md) | Running Pley: Docker, systemd, launchd, Windows, firewall, reverse proxies |
| [docs/QUALITY.md](docs/QUALITY.md) | The Quality menu, Original and Automatic quality |
| [docs/STORAGE.md](docs/STORAGE.md) | What Pley keeps on disk, backups and clean-up |
| [docs/API.md](docs/API.md) | Pley's HTTP API, including app sign-in (OAuth 2.1) |
| [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) | The compatibility API for `python-plexapi`, Tautulli and Kometa |
| [docs/TRANSCODER.md](docs/TRANSCODER.md) | The transcoder and its protocol |
| [docs/ANDROID.md](docs/ANDROID.md), [docs/KODI.md](docs/KODI.md), [docs/ROKU.md](docs/ROKU.md), [docs/TV_PLATFORMS.md](docs/TV_PLATFORMS.md) | The Android, Kodi, Roku, Samsung and LG apps |
| [docs/RELEASING.md](docs/RELEASING.md) | Versions, changelogs, releases and CI |
| [SECURITY.md](SECURITY.md) | Security model and reporting |
| [LICENSING.md](LICENSING.md) | The license boundary between server and transcoder |

## Development

### Architecture

```
 browser ── HTTP ──▶ pley (server)                               pley-transcoder
   React UI            libraries · scanner · metadata · auth      ffprobe/ffmpeg · HLS jobs · tone mapping
   hls.js              sessions · hubs · search · /api            thumbnails · subtitles · keyframes · /metrics
                       SQLite (WAL)          ── HTTP/JSON ──▶      spawned & supervised by the server
```

The transcoder is a separate program, and the server talks to it only over
HTTP, so the two can have different licenses ([LICENSING.md](LICENSING.md)).

| Path | What |
|---|---|
| `apps/server` | The media server (crate `pley-server`, binary `pley`) |
| `apps/transcoder` | The FFmpeg engine (crate and binary `pley-transcoder`) |
| `apps/web` | The web app (React, TypeScript, Vite, Tailwind), embedded into the server |
| `apps/desktop` | Pley for Windows, macOS and Linux (Tauri) |
| `apps/android` | Pley for Android phones, tablets and TVs (Kotlin, Compose, Media3) |
| `apps/kodi` | Pley for Kodi, an add-on (Python, GPL-2.0-or-later) |
| `apps/roku` | Pley for Roku (BrightScript, SceneGraph) |
| `apps/tizen`, `apps/webos` | Pley for Samsung TV and Pley for LG TV |
| `packages/tv` | The TV app both share (TypeScript, Preact) |
| `packages/transcoder-api` | Protocol types shared by server and transcoder |
| `deploy/` | Docker, systemd and launchd setups, the Windows installer, store listings |
| `docs/` | Documentation ([above](#documentation)) |
| `scripts/` | Versions and releases (`version.mjs`, `release.mjs`), release builds, the transcoder source package, the license check, the apps' icons and store pictures (`render-icons.mjs`) |

### Requirements

- Rust 1.95 or newer.
- [Bun](https://bun.sh) 1.4 or newer (`package.json` pins 1.4.4) to build the
  web app and the other JavaScript apps. Bun installs packages from
  `bun.lock` and runs their scripts and tools (each package's `bunfig.toml`
  says so).
- Node.js 22 or newer, for the Roku app's tools only.
- FFmpeg and ffprobe on `PATH` or configured; 6.1 or newer throttles
  transcodes. For hardware transcoding and GPU tone mapping, use a build
  with NVENC, QSV, AMF or VAAPI and libplacebo: gyan.dev's "full" build on
  Windows, Debian 13's or jellyfin-ffmpeg on Linux, Homebrew's `ffmpeg-full`
  on macOS ([deploy/README.md](deploy/README.md)).

### Building and testing

| Command | What |
|---|---|
| `bun run dev` | The web app with hot reload on `http://localhost:5173`, proxying `/api` to the server on port 7539. Debug builds of the server read `apps/web/dist` from disk, so `bun run build` shows up without recompiling |
| `bun install --frozen-lockfile` | Installs exactly what `bun.lock` holds, as CI and the Docker build do |
| `cargo test --workspace` | The Rust tests, including end-to-end tests of both APIs |
| `cargo test -p pley-server --test providers -- --ignored` | Also checks the real TVmaze and MusicBrainz (and TheTVDB with `PLEY_TEST_TVDB_KEY` and, if needed, `PLEY_TEST_TVDB_PIN`) |
| `cargo clippy --workspace --all-targets`, `cargo fmt --all` | Lint and format the Rust code |
| `scripts/check-licenses.ps1` | Checks the license boundary |
| `bun run check` | The web app's and desktop app's type check, ESLint and tests (Vitest) |
| `bun run desktop` | Runs the desktop app (on Linux needs WebKitGTK: `libwebkit2gtk-4.1-dev`); `bun run --cwd apps/desktop build` makes its installers |
| `bun run tv:check` | Type-checks and tests the Samsung and LG apps; `bun run tizen` and `bun run webos` package them ([docs/TV_PLATFORMS.md](docs/TV_PLATFORMS.md)) |
| `bun install && bun run check` in `apps/roku` | Compiles, lints and tests the Roku app ([docs/ROKU.md](docs/ROKU.md)) |
| `scripts/build.ps1` (or `build.sh`) | Builds the server's release archive locally |

The Android app builds with the Android SDK and JDK 17 or newer (on Windows,
Android Studio's: `JAVA_HOME=C:\Program Files\Android\Android Studio\jbr`).
In `apps/android`:

```sh
./gradlew testDebugUnitTest lintDebug assembleDebug assembleStoreRelease   # as CI does
pwsh ../../scripts/check-android.ps1                                      # the release's libraries, native code and size
adb install app/build/outputs/apk/store/debug/app-store-debug.apk         # the debug app
```

Builds come in two flavors, `store` and `direct`;
[docs/ANDROID.md](docs/ANDROID.md) has the modules and the rest.

[GitHub Actions](.github/workflows) runs these checks on every push and pull
request, each app's workflow only when that app (or what it is built from)
changes. The server's runs on Linux, macOS and Windows, with a build on the
oldest supported Rust, the license check and the Docker build
([docs/RELEASING.md](docs/RELEASING.md#ci)).

### Versions and releases

Each app (the server with its transcoder and web app, desktop, Android,
Samsung, LG, Kodi and Roku) has its own version, `CHANGELOG.md` and release
tag (`kodi-v0.0.2`). `bun scripts/release.mjs <app> patch` bumps one and
dates its changelog; the Release workflow, run by hand, builds and publishes
it. Every app shows the commit it was built from beside its version, as
"0.0.1 (1a2b3c4d5)". [docs/RELEASING.md](docs/RELEASING.md) has the details.

## Contributing

Issues and pull requests are welcome.

- Run the checks for what you changed ([Building and testing](#building-and-testing));
  CI runs them too.
- Commit messages start with the area they touch, as in the history:
  `transcoder: …`, `kodi, roku: …`, `docs: …`.
- Add a line under `## [Unreleased]` in the changed app's `CHANGELOG.md`
  ([docs/RELEASING.md](docs/RELEASING.md#releasing)).
- Report security issues as [SECURITY.md](SECURITY.md) describes, not in a
  public issue.

The project is early: each app's version starts at 0.0.1.

## License

Pley is split along a license boundary ([LICENSING.md](LICENSING.md)):

| Part | License |
|---|---|
| Server and web app | All rights reserved |
| Desktop, Android, Roku, Samsung and LG apps | All rights reserved |
| Transcoder (`apps/transcoder`) | All rights reserved |
| Kodi add-on (`apps/kodi`) | GPL-2.0-or-later ([LICENSE.txt](apps/kodi/plugin.video.pley/LICENSE.txt)) |
| Protocol types (`packages/transcoder-api`) | All rights reserved |
