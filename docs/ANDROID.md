# Android and TV apps

One app, `com.formandelement.pley`, for phones, tablets, Android TV, Google
TV and Fire TV (Fire OS 7 and later). Android 8 is the minimum. It is
native Kotlin and Compose: `MainActivity` on phones and tablets,
`TvActivity` on TVs.

## Installing

No store carries the app yet, so it is sideloaded, as a debug build or a
signed release ([Releases](#releases)):

- From a computer: `adb install -r app-store-debug.apk` (for a TV or Fire
  TV, `adb connect <ip>:5555` first; see [Emulators](#emulators)).
- On a phone: open the APK in a file manager, and allow that app to install
  apps when asked.
- On a Fire TV: turn on the developer options ([Emulators](#emulators)) and
  "Install unknown apps" for the Downloader app, then open the APK's
  address in Downloader.

The phone launcher opens `MainActivity` and the TV home screen
`TvActivity`; a TV that opens `MainActivity` is sent on to `TvActivity`.
Android TV shows the 320×180 banner, Google TV the icon in a circle, and
Fire TV a sideloaded app's icon in a square between grey bars.

## Fire TV

Fire TV runs the same APK and TV screens as Android TV. The app recognizes
it by the `amazon.hardware.fire_tv` feature and then follows Amazon's
player guidelines: decoders queue buffers asynchronously (a per-model
switch can turn it off), surround output follows Fire OS's audio setting,
Alexa's play and pause reach the player, and Fire OS 16 asks for the
nearby-devices permission before searching the network. There is no Watch
Next row.

The remote works as on Android TV: Select opens, holding Select or Menu
(☰) opens a title's actions, Back goes back, and play/pause, rewind and
fast forward work in the player.

## Audio and subtitles

- The player sends an audio track (`audio_index`) only when the person
  picked one; otherwise the server picks the profile's language in the
  best format this device plays. Downloads choose on the device.
- Tracks picked in the player are remembered for the title
  (`PUT /api/items/{id}/streams`); subtitles changed alone leave the audio
  unpinned.
- The profile's defaults (Audio language, Subtitles, Subtitle language) are
  under Me on phones and in Settings on TVs.
- **Sync** (subtitle menu): 0.1 s steps, up to 10 s either way, remembered
  per file on the device.
- **Subtitle style** (Settings, or Subtitle style in the phone's player
  menu and the TV's Subtitles panel): size, background, font (sans serif,
  serif, monospace, and a casual hand where the system has one, not on
  every Fire OS) and position (bottom, or raised 5, 10 or 15 %). A chosen
  font overrides the system's captions style. Position applies to text
  subtitles only.
- **Find subtitles…** (end of the subtitle menu, while streaming, not for
  downloads) lists what the server finds online in the profile's subtitle
  language; the one picked is added on the server and shows. A server
  without search set up says "Subtitle search isn't set up on this
  server".
- **Speed**: 0.5× to 2×, pitch kept (the phone's player settings; a button
  at the TV player's left).
- **Audio delay**: up to 5 s either way in 50 ms steps (above the audio
  tracks; Left and Right on the TV). It works with passthrough.
- Speed and delay last for the title only. Speed can't change for audio
  passed through to a receiver, and neither can with tunneled playback; the
  menus say so.

## Quality

The menu, Original and Automatic are as in
[QUALITY.md](QUALITY.md#the-menu). On Android:

- The quality is kept per device and network kind: Settings → Quality on
  Wi-Fi and Quality on mobile data on phones, Quality on TVs. Automatic
  until one is chosen.
- Automatic asks the server's network memory alongside the item's details
  and runs a speed test when the server asks for one, both during
  Preparing. Without an answer it uses its own memory, which a restart
  clears.
- Waits while the server is out of reach (Reconnecting) aren't play issues.
- Dropped frames at the original switch to the server's quality, and
  Automatic doesn't step back up to the original during the title.
- Downloads play as they are, in Automatic too.
- [Several sizes](QUALITY.md#several-sizes-adaptive-bitrate): Media3 moves
  between them, never bigger than the screen.
- A change that needs a new session (a quality, an audio track the player
  can't switch to itself, burned-in subtitles, Automatic's steps, Play
  original) is prepared beside the playing one (`keep_previous`): the last
  picture stays on screen until the new stream's first frame, and the
  spinner shows only after 2.5 s. With no transcode free for two (`503`),
  the old session ends first.

## Watch Next

On Android TV and Google TV, the home screen's Watch Next row shows the
profile's Continue Watching (`watchnext/`, ten at most): Continue for
titles watched part way, Next for episodes not started. It follows what
the app already keeps current, changes with the profile or server, and is
emptied on sign-out. A program removed from the row stays off. Selecting
one opens the title and plays it once its profile is signed in ("Who's
watching?" first, if asked). Not on phones or Fire TV.

## Screens

- **Library order** is the profile's. On phones: Sort in the Libraries
  tab's top bar (drag, or Move up and Move down for TalkBack). On TVs: Sort
  by the Libraries page's title, Move in a library's actions, and Settings
  → Sort libraries (with three libraries or fewer there is no Libraries
  page). A server too old to sort says so.
- **See all** on a row (Recently Added, Recently Released, Top Rated) shows
  the Library tab in that row's order without changing the library's view.
- **On the TV player**, OK within two seconds of a seek plays on from
  there; later, OK pauses.
- **Badges**: Dolby Vision profile 7 is labelled FEL or MEL; a hybrid file
  shows both DV and HDR10+.

## Live TV

Live TV ([API.md](API.md#live-tv)) shows where `GET /api/live` says
`available`: first in the phone's Libraries tab, after Home in the TV's
rail. Its sources, lineup and connections are set up in the web app.

- **Categories**: Favourites, Recently watched, All channels, then
  Regions, alphabetically, each opening to its groups ("UK| Sport",
  "UK| News"), then More groups: those without a region, or whose region
  has one group, in the provider's order. With fewer than two regions
  it is one list in the provider's order. A picker on phones; a column
  beside the guide on TVs, where Left from the guide's start or the
  cards' first column goes to the category shown, scrolled into view.
  The category, the view (guide or cards) and the channel last watched
  are kept per server and profile (`LiveTvViews`), so Live TV opens as it
  was left after a restart too.
- **Search**: one field (the top of the TV's column, with the system
  keyboard) looks among every channel (`browse?category=all&q=`),
  whatever category is open, and names the groups it matches as quick
  links; choosing a category ends it. The player's channel list searches
  every channel too.
- **Pages**: a category's channels come 100 at a time (`browse`) as the
  guide or the cards scroll, and the guide asks for the programmes of
  the pages in view only (`ids`), six hours at a time; a provider of
  50,000 channels opens at once. The groups and each category's first
  page are snapshots; the rest is held in memory.
- **The guide**: time across from half an hour before now, now marked;
  on TVs focus moves by programme (Left and Right), Up and Down keep the
  time, OK watches what's on now and opens a later programme's details,
  holding OK or Menu opens them too, and Back goes back to now first.
  A channel without a guide shows its name across it. Coming back from
  the player, it opens on the channel last watched.
- **The player** (`LivePlayerActivity`, `TvLivePlayerActivity`) tunes
  with this device's client id, which ends what it watched before, so
  zapping holds one connection. Channel up and down stay in the category
  it was opened from, round its ends (Up and Channel up the next one on
  TVs, a swipe or the chevrons on phones); the channel list's heading
  shows another group's channels, and a channel tuned from there makes
  channel up and down follow that group. A typed number tunes that
  number among every channel. What's on and next shows only when asked
  for (Info on TVs, a tap that brings the controls on phones), not as a
  channel tunes. It is watched as it airs, as the web's: no pause, no
  going back, and Live is a plain badge; play, pause, rewind and fast
  forward (media keys, the media session, Left and Right on TVs) do
  nothing, and a pause from elsewhere (headphones pulled out) plays on.
  `409` shows the server's sentence (which channels hold the
  connections), with Try again and Other channels. Leaving (onStop, not
  in picture in picture) lets the session go; coming back tunes again at
  the live edge. The TV's display mode isn't matched for live channels.

## Offline

The app needs no internet: the server signs Pley's own apps in itself. On
a phone whose Wi-Fi has no internet, traffic to the home server still goes
over the Wi-Fi while mobile data is the default network (`LocalRoute`).

- **Server out of reach**: the screens show what was saved, under "Showing
  what Pley had saved. Reconnecting…" (a banner leading to downloads on
  phones; a notice, then a mark on the rail's avatar, on TVs). They come
  back on their own once the server answers. A home address (`192.168.…`,
  `nas.local`) gets 3 s to connect, so "Can't reach your server" comes
  within seconds, with Try again.
- **Playback that loses the server** keeps its place, plays what it has,
  then shows "Reconnecting…" and retries for 90 s before "Can't reach your
  server", with Try again.

### Downloads

Phones and tablets only; TVs download nothing.

- Download sits under Play on a movie's or episode's page, beside each
  episode and for a whole season. They are listed under Me → Downloads.
- A download is the original file (of several versions, the sharpest up to
  1080p the device plays as it is) with its details, a picture and its
  text subtitles, in the app's own storage per server and profile (removed
  with the app).
- A file the device can't play as it is, or every file when Settings →
  Downloads → Quality picks a smaller one, is converted by the server first
  (`POST /api/conversions`, [API.md](API.md#downloads)): "Preparing on the
  server", then the download. A server whose transcoder can't make files
  gives the original, unless a smaller quality was asked for.
- Downloads run one at a time in a foreground service with a notification,
  and resume where they stopped. They wait for Wi-Fi or Ethernet unless
  Settings → Downloads allows mobile data ("Waiting for Wi-Fi"), and for
  the server ("Waiting for your server"). A failed one says why in its
  menu, with Try again and Delete download.
- A download plays from the device whether the server answers or not
  ("Downloaded" in the player). A quality picked by hand streams instead,
  as does a download whose file the server no longer has.
- Progress made offline is sent when the server answers again. An episode
  goes on to the next downloaded one when the server can't say what's
  next. A download the device can't decode falls back to the server.

## Updating from the web view app

Installing this app over the earlier web view app (0.1) keeps its saved
servers and install id (`WebViewMigration`) and asks to sign in again. Its
old session is ended on each server (`POST /api/auth/logout`, best effort)
and its web view data deleted (`migration/WebViewCleanup`, checked by
`CutoverTest`). The web app's hooks for the web view
(`apps/web/src/lib/shell.ts`) go after one more release.

## Modules

Everything is in `apps/android`, sources in `src/main/kotlin`. Each module
sees only what it builds on.

| Module | What it holds | Builds on |
|---|---|---|
| `:app` | `PleyApplication`, `AppGraph`, `MainActivity`, `TvActivity`, Watch Next (`watchnext/`), the web view clean-up (`migration/`) | everything |
| `:core:common` | address rules, discovery, local network access, device profile and tier, picture sizes, trace names, test tags | nothing |
| `:core:model` | the server's answers and requests, events, the capability report; plain Kotlin | nothing |
| `:core:auth` | sign-in: the browser page, the TV's code, tokens | common |
| `:core:network` | OkHttp clients, `PleyApi`, server events, discovery | common, model, auth |
| `:core:data` | repositories, snapshots, servers, session, settings, live updates | network, auth, model |
| `:core:caps` | what the device plays: decoders, Dolby Vision, HDR, display modes, passthrough, quirks | common, model, Media3 |
| `:core:player` | the Media3 engine, playback sessions, subtitles, previews, markers, queues, music | caps, data, network |
| `:core:downloads` | downloads: the queue, its service, the files, offline progress | data |
| `:core:designsystem` | colours, icons, image loading, artwork, tiles; Compose foundation only | common, model |
| `:core:screens` | one ViewModel per screen, shared by phone and TV; routes (`PleyKey`), `Navigator`, `PlayerLauncher` | data, player, caps, auth |
| `:core:testing` | fakes, JSON fixtures from docs/API.md, test rules | the contracts |
| `:ui:phone` | the phone and tablet interface (Material 3) | screens, designsystem, player |
| `:ui:tv` | the TV interface (tv-material, the rail, D-pad rules, the TV player) | screens, designsystem, player |
| `:testing:fixtures` | `FixtureServer`, a fake Pley server on 127.0.0.1 | model |
| `:benchmark` | Macrobenchmarks and the Baseline and Startup Profile generators | `:app` |

Rules the build keeps:

- `:ui:tv` never sees Material 3 or `:ui:phone`; their two `MaterialTheme`s
  would mix.
- Screen modules don't depend on each other; they meet through `PleyKey`
  routes and the `Navigator`.
- Contracts (the interfaces in `:core:*`) change only by adding to them;
  unbuilt parts are stubs (`NotBuiltYet`).
- Shared build setup lives in the convention plugins in `build-logic`
  (`pley.android.application`, `pley.android.library`,
  `pley.android.compose`, `pley.jvm.library`, `pley.android.test`) and
  `gradle/libs.versions.toml`.
- No native code, so the app runs on Fire TV's 32-bit userspace.

## Builds

Two flavors: `store` (Google Play and the Amazon Appstore, no self-updater)
and `direct` (the download the Pley server will offer). Release builds are
minified, and signed when the release key is given.

The version is `appVersion` in `app/build.gradle.kts`, set by
`bun scripts/release.mjs android <patch|minor|major>`; the version code
follows from it ([RELEASING.md](RELEASING.md#androids-version-code)), plus
`PLEY_BUILD` (1 to 99) for a store rebuild of the same version. Builds show
their commit (`PLEY_COMMIT`, else git's) in Settings and Diagnostics, as
"0.0.1 (1a2b3c4d5)".

The only exported components are the two launchers (`MainActivity`,
`TvActivity`), the sign-in's return address (`SignInRedirectActivity`),
`MusicService` and `WatchNextImageProvider`; `CutoverTest` enforces it.

### Releases

The shared process is in [RELEASING.md](RELEASING.md). Release builds are
signed with the key these environment variables name, and unsigned
without them:

| Variable | |
|---|---|
| `PLEY_KEYSTORE` | the keystore file |
| `PLEY_KEYSTORE_PASSWORD` | its password |
| `PLEY_KEY_ALIAS` | the key's alias |
| `PLEY_KEY_PASSWORD` | the key's password, when not the keystore's |

```powershell
$env:PLEY_KEYSTORE = "$HOME\keys\pley-release.keystore"   # and the others
.\gradlew.bat :app:assembleDirectRelease :app:bundleStoreRelease
```

This gives the APK to sideload,
`app/build/outputs/apk/direct/release/app-direct-release.apk`, and the
store bundle, `app/build/outputs/bundle/storeRelease/app-store-release.aab`.
The Release workflow builds both with the key from the repository's
secrets: `ANDROID_KEYSTORE_BASE64` (the keystore, base64-encoded),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and, when it differs,
`ANDROID_KEY_PASSWORD`.

Make the key once and keep it safe: every update must be signed with it.

```sh
keytool -genkeypair -v -keystore pley-release.keystore -alias pley \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 pley-release.keystore   # the ANDROID_KEYSTORE_BASE64 secret
```

A debug build must be uninstalled once before a release installs over it.
With Play App Signing, this key is Google Play's upload key, which Play can
reset if it is lost. Store pictures and listings are in `deploy/stores`.

## Commands

In `apps/android`, with Android Studio's JDK (Windows):

```powershell
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"
.\gradlew.bat testDebugUnitTest lintDebug assembleDebug assembleStoreRelease
```

`testDebugUnitTest` and `lintDebug` cover every module and both flavors.
`-Ppley.composeReports=true` writes Compose compiler reports to each
module's `build/compose/`. The debug APKs are
`app/build/outputs/apk/store/debug/app-store-debug.apk` and
`.../direct/debug/app-direct-debug.apk`.

Baseline and Startup Profiles are generated by hand, on an emulator or
device with API 33 or later, and committed
(`app/src/main/generated/baselineProfiles`):

```powershell
.\gradlew.bat :app:generateBaselineProfile
```

Sideloaded installs use them only after the device's overnight compile;
testers run `adb shell cmd package compile -m speed-profile -f
com.formandelement.pley`.

## Emulators

| Name | Image | For |
|---|---|---|
| `pley_test` | API 36 phone (`google_apis`) | the phone interface, UI tests |
| `pley-tv-34` | `system-images;android-34;google-tv;x86` on `tv_1080p` | D-pad and focus tests, profile generation |
| an API 37 phone | Android 17 | the local network permission |
| (optional) | `system-images;android-36;google-tv;x86_64` | Fire OS 16's level: predictive Back, edge to edge |
| (optional) | `system-images;android-28;android-tv;x86`, 1 GB | Fire OS 7's level (a Fire TV stick); can't play video |

```powershell
sdkmanager "system-images;android-34;google-tv;x86"
avdmanager create avd -n pley-tv-34 -k "system-images;android-34;google-tv;x86" -d tv_1080p
emulator -avd pley-tv-34 -gpu host
```

Emulators don't count for performance and can't show HDR, Dolby Vision,
display-mode switching, passthrough or real decoder limits: check those on
devices.

On a Fire TV: select the device name 7 times in Settings → My Fire TV →
About, turn on ADB debugging in the developer options that appear, and run
`adb connect <ip>:5555`.

## A server to test against

Never test against a Pley server people use: signing in creates grants and
playing writes progress. Unit and UI tests use `FixtureServer`. For a real
server, start a disposable one with its own data folder:

```powershell
$data = Join-Path $env:TEMP "pley-android-test"
pley --data-dir $data reset-password admin <password> --admin
pley --data-dir $data --bind 127.0.0.1:7712 --plex-bind off --no-discovery --no-tray
adb reverse tcp:7712 tcp:7712
```

The app then opens `http://127.0.0.1:7712` (an emulator also reaches the
computer at `http://10.0.2.2:7712`). Delete the folder afterwards.

## Performance

Performance is measured on devices (a mid-range phone, a 1 GB Fire TV
stick, a Chromecast with Google TV HD), never on emulators.

`:benchmark`'s `TvStartupBenchmark` starts the TV app cold ten times
against the demo server and records the time to the first frame and to
Home fully drawn, with a Perfetto trace of each start. Run it with adb on
one device, not as a connected Gradle task, which installs the app on
every attached device:

```powershell
.\gradlew.bat :app:assembleStoreBenchmarkRelease :benchmark:assembleBenchmarkRelease
adb -s <device> install -r app\build\outputs\apk\store\benchmarkRelease\app-store-benchmarkRelease.apk
adb -s <device> install -r benchmark\build\outputs\apk\benchmarkRelease\benchmark-benchmarkRelease.apk
adb -s <device> shell am instrument -w -e class app.pley.android.benchmark.TvStartupBenchmark `
  app.pley.android.benchmark/androidx.test.runner.AndroidJUnitRunner
```

On an emulator add `-e androidx.benchmark.suppressErrors EMULATOR`. Traces
are in `/storage/emulated/0/Android/data/app.pley.android.benchmark/cache`.
Benchmark builds run the demo server inside the app, which costs CPU and
memory a release doesn't.
