# Android and TV apps

One app, `app.pley.android`, for phones, tablets, Android TV, Google TV and
Fire TV (Fire OS 7 and later; Android 8 is the minimum), native in Kotlin
and Compose: `MainActivity` on phones and tablets, `TvActivity` on TVs. It
replaced a web view app (0.1: a list of servers, the server's web app and
a Media3 player), which an update clears up after (below).

## Installing

Release builds aren't signed yet, and no store carries the
app. Until then it is sideloaded, a debug or self-signed build:

- From a computer: `adb install -r app-store-debug.apk` (`adb connect
  <ip>:5555` first for a TV or Fire TV, see Emulators).
- On a phone: open the APK in a file manager and allow that app to install
  apps when asked.
- On a Fire TV: turn on the developer options (below) and "Install unknown
  apps" for the Downloader app, then open the APK's address in Downloader.

The phone or tablet's launcher opens `MainActivity`; the TV home screen
shows the 320×180 banner and opens `TvActivity`. A TV that opens
`MainActivity` (some launchers do) is sent on to `TvActivity`.

## Fire TV

Fire TV runs the same APK and the same TV screens as Android TV; there is
no Fire TV build. The app tells it apart by Amazon's
`amazon.hardware.fire_tv` feature and then only changes what Amazon asks
of a player: decoders queue their buffers asynchronously (Fire TV's
media guidelines; a per-model switch can turn it off), surround output
follows Fire OS's own audio setting, Alexa's play and pause reach the
player's media session, and Fire OS 16 asks for its nearby-devices
permission before searching the network.

The remote works as on Android TV: Select opens, holding Select or
pressing Menu (☰) opens a title's actions, Back goes back, and play/pause,
rewind and fast forward work in the player. Fire OS 7 (Android 9) is the
oldest supported; an `android-28` Android TV emulator with 1 GB of memory
stands in for a Fire TV stick (see Emulators), apart from video decoding:
its decoders are software only, which the app doesn't use on TVs, so it
can't play video.

## Offline

The app needs no internet: the server signs pley's own apps in itself
(their client documents are built in, never fetched), and on a phone
whose Wi-Fi has no internet, traffic to the server at home goes over the
Wi-Fi even while the phone's default network is mobile data
(`LocalRoute`). Away from the server, phones and tablets play downloads.

With the server out of reach, the screens show what was saved of them,
under "Showing what pley had saved. Reconnecting…": a banner under the
top bar on phones (leading to the downloads), a notice at the top right
on TVs. The server's event stream keeps trying, and the screens come back
on their own once it answers. A home address or name (`192.168.…`,
`nas.local`) has 3 s to accept a connection rather than OkHttp's 10, as
one at home accepts at once or never (`HttpRoot.homeConnectTimeout`), so
pressing play with the server off says "Can't reach your server" within
seconds, with Try again.

Downloads (`:core:downloads`) keep the original file of a movie or an
episode (of several versions, the sharpest up to 1080p of those the
device plays as they are, by the player's capabilities), with its
details, a picture and its text subtitles as WebVTT, in the app's own
storage (removed with the app), per server and profile. A track the
server is still reading out of the file (minutes for a big one on a
network share) is kept as far as it goes while the file comes, then
asked for again once the file is down, and when the app opens while it
still falls short. A file the device
doesn't play as it is, or every file when Settings → Downloads → Quality
picks a smaller one, is made by the server first (`POST /api/conversions`
with the player's capabilities, docs/API.md "Downloads"): the download
shows "Preparing on the server" with the server's progress, looks again
every 5 s, then downloads the made file, which keeps the original's
subtitle tracks (the player finds them in it as in the original). A
server whose transcoder can't make files gives the original, unless a
smaller quality was asked for. They come in a foreground service of type
dataSync with a notification, one at a time; one cut short goes on from
where it stopped (Range). They wait for Wi-Fi or Ethernet unless Settings
→ Downloads allows mobile data ("Waiting for Wi-Fi"), and for the server
when it doesn't answer or its transcoder isn't running ("Waiting for your
server"); those waiting go on when the app opens. A failed one says why
in its menu, with Try again and Delete download; deleting one the server
is still making stops it there. Download sits under Play on a movie's or
an episode's page, beside each episode of a season (and for the whole
season), and they are listed under Me → Downloads, which the offline
banner leads to as well.

A downloaded file plays from the device whether the server answers or not
("Downloaded" in the player); a quality picked by hand streams from the
server instead, and so does a download whose file isn't the one the server
now has under that number (by its name and size: a server whose database was
made anew numbers its files again). A download waits 1.5 s at most for the
server's details, then starts from the ones saved with it (away from home,
the server's address can take long to fail). Progress is kept on the device
while the server is out of reach, then sent when it answers again (the app
opening, a network coming up), each report with its `age_ms`: the server
keeps what happened to the item since (docs/API.md). An episode played from
a download goes on to the next one of its show downloaded when the server
doesn't say what's next within 3 s. A download the device can't decode after
all falls back to the server, which converts it; with the server away, the
player says so. TVs stay where the server is and download nothing.

## Updating from the web view app

An update installed over the web view app keeps what matters and nothing
else:

- Its saved servers and its install id (the player's `client_id`, so the
  server sees the same player) are read once into the native app's store
  (`:core:data`, `WebViewMigration`). The last server used stays selected.
- Its sign-in doesn't carry over: the web view held a full-rights browser
  session, the native app holds app tokens with viewer rights only. The app
  opens at sign-in, on the server it had.
- Once, after the first frame and off the main thread
  (`app/.../migration/WebViewCleanup`, run by AndroidX Startup): the web
  view's session cookie on each saved server is sent to
  `POST /api/auth/logout` as a Bearer token, which ends that 90-day session
  (best effort: a server out of reach keeps it until it expires); then its
  cookies, web storage and folders (`app_webview`, `app_textures`, its
  caches and preferences) are deleted. On a fresh install the folder isn't
  there and no web view ever starts. Nothing else in the app reads cookies
  (`CutoverTest`).

The web app keeps its hooks for the Android web view (`apps/web/src/lib/shell.ts`
and the player hand-off) for one more release, then they go.

## Modules

Everything is in `apps/android`, sources in `src/main/kotlin` (`:app` keeps
the old saved-servers format, `ServerStore`, in `src/main/java` for the
startup read). Each module sees only what it builds on.

| Module | What it holds | Builds on |
|---|---|---|
| `:app` | `PleyApplication`, the app's parts made by hand (`AppGraph`), `MainActivity` (phones), `TvActivity` (TVs), the clean-up after the web view app (`migration/`) | everything |
| `:core:common` | server address rules, discovery, local network access, the device's profile and tier, picture sizes, trace names, test tags | nothing |
| `:core:model` | the server's answers and request bodies (kotlinx.serialization), server events, the capability report; plain Kotlin, no Android | nothing |
| `:core:auth` | signing in (`app.pley.android.auth`): the server's page in the browser, the TV's code, tokens and where they may go | common |
| `:core:network` | the OkHttp clients, `PleyApi`, server events, discovery, address checks | common, model, auth |
| `:core:data` | repositories, snapshots, servers, session, settings, live updates | network, auth, model |
| `:core:caps` | what the device plays: decoders, Dolby Vision, HDR, display modes, passthrough, quirks | common, model, Media3 |
| `:core:player` | the Media3 engine, playback sessions, subtitles, previews, markers, queues, music | caps, data, network |
| `:core:downloads` | downloads for watching away from the server: the queue, its foreground service, the files and their index, progress kept while offline | data |
| `:core:designsystem` | colours, icons, the image loader, artwork and tiles; Compose foundation only, never Material | common, model |
| `:core:screens` | one ViewModel per screen, shared by phone and TV; the routes (`PleyKey`), `Navigator`, `PlayerLauncher` | data, player, caps, auth |
| `:core:testing` | fakes, JSON fixtures written from docs/API.md, test rules; tests only | the contracts |
| `:ui:phone` | the phone and tablet interface: Material 3, the navigation bar or rail, the player with picture in picture | screens, designsystem, player |
| `:ui:tv` | the TV interface: tv-material for the chrome, our own tiles, the rail, the D-pad rules, the TV player | screens, designsystem, player |
| `:testing:fixtures` | `FixtureServer`, a fake pley server on 127.0.0.1 | model |
| `:benchmark` | Macrobenchmarks and the Baseline and Startup Profile generators | `:app` |

Rules the build keeps:

- `:ui:tv` never sees Material 3 or `:ui:phone`: the two libraries each have
  a `MaterialTheme`, and mixing them mixes colours, type and shapes. (Debug
  builds' Compose previews bring Material 3 at run time only.)
- No screen module depends on another: screens meet through `PleyKey`
  routes and the `Navigator`.
- Contracts (the interfaces in `:core:*`) change only by adding to them.
  Each has a stub that says what isn't built yet (`NotBuiltYet`).
- Build files and `gradle/libs.versions.toml` are changed on purpose, in
  one place: the convention plugins in `build-logic` (`pley.android.application`,
  `pley.android.library`, `pley.android.compose`, `pley.jvm.library`,
  `pley.android.test`) hold what modules share.
- The app has no native code (no `.so` of its own), so it runs on Fire TV's
  32-bit userspace.

## Builds

Two flavors, `store` (Google Play and the Amazon Appstore, no self-updater)
and `direct` (the download the pley server will offer, which may hand an
update to the installer).

Launchers, the same in every build: "pley" on phones and tablets
(`MainActivity`, `LAUNCHER`) and on TV home screens (`TvActivity`,
`LEANBACK_LAUNCHER`, with the 320×180 banner). Exported, beside these two,
are only the sign-in's return address (`SignInRedirectActivity`) and the
music service (`MusicService`); `CutoverTest` holds the project's manifests
to that.

## Commands

In `apps/android`, with Android Studio's JDK (Windows):

```powershell
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"
.\gradlew.bat testDebugUnitTest lintDebug assembleDebug assembleStoreRelease
```

`testDebugUnitTest` and `lintDebug` cover every module, the plain Kotlin
ones and both flavors of the app included. `-Ppley.composeReports=true`
writes the Compose compiler's reports to `build/compose/` of each module.
Gradle runs in parallel with its configuration cache.

The debug APKs are `app/build/outputs/apk/store/debug/app-store-debug.apk`
and `.../direct/debug/app-direct-debug.apk`.

Baseline and Startup Profiles are made on purpose, never during a build, on
an emulator or device with API 33 or later:

```powershell
.\gradlew.bat :app:generateBaselineProfile
```

They land in `app/src/main/generated/baselineProfiles` and are committed.
Sideloaded installs only use them after the device's overnight compile;
testers run `adb shell cmd package compile -m speed-profile -f app.pley.android`.

## Emulators

| Name | Image | For |
|---|---|---|
| `pley_test` | API 36 phone (`google_apis`) | the phone interface, UI tests |
| `pley-tv-34` | `system-images;android-34;google-tv;x86` on `tv_1080p` | D-pad and focus tests, profile generation |
| an API 37 phone | Android 17 | the local network permission |
| (optional) | `system-images;android-36;google-tv;x86_64` | Fire OS 16's level: predictive Back, edge to edge |
| (optional) | `system-images;android-28;android-tv;x86` | Fire OS 7's level |

```powershell
sdkmanager "system-images;android-34;google-tv;x86"
avdmanager create avd -n pley-tv-34 -k "system-images;android-34;google-tv;x86" -d tv_1080p
emulator -avd pley-tv-34 -gpu host
```

Emulators never count for performance, and can't show HDR, Dolby Vision,
display-mode switching, passthrough or real decoder limits: those are
checked on devices. On a Fire TV, select the device name 7 times in
Settings → My Fire TV → About, turn on ADB debugging in the developer
options that appear, and run `adb connect <ip>:5555`.

## A server to test against

Nothing in tests, benchmarks or manual checks signs in to, plays on or
otherwise writes to a pley server people use: signing in creates grants
and playing writes progress. Unit and UI tests use `FixtureServer`. For a
real one, start a disposable server with its own data folder:

```powershell
$data = Join-Path $env:TEMP "pley-android-test"
pley --data-dir $data reset-password admin <password> --admin
pley --data-dir $data --bind 127.0.0.1:7712 --plex-bind off --no-discovery --no-tray
adb reverse tcp:7712 tcp:7712
```

The app then opens `http://127.0.0.1:7712` (an emulator also reaches the
computer at `http://10.0.2.2:7712`). Delete the folder afterwards.

## Performance

The budgets, and how they are measured, are kept here as they are
measured on devices: cold start, scrolling, pictures, playback start and
memory, on a mid-range phone, a 1 GB Fire TV stick and a Chromecast with
Google TV HD. Emulator numbers never count.

### TV cold start

`:benchmark`'s `TvStartupBenchmark` signs in to the demo server once, then
starts the TV app cold ten times with its Baseline Profile: the time to
the first frame, to the first screen fully drawn (the app reports it once
Home has its rows, or at once for another first screen) and the app's own
startup steps, with a Perfetto trace of each start. Run it with adb on one
device, never as a connected Gradle task, which installs the app on every
device attached (a phone's own pley among them):

```powershell
.\gradlew.bat :app:assembleStoreBenchmarkRelease :benchmark:assembleBenchmarkRelease
adb -s <device> install -r app\build\outputs\apk\store\benchmarkRelease\app-store-benchmarkRelease.apk
adb -s <device> install -r benchmark\build\outputs\apk\benchmarkRelease\benchmark-benchmarkRelease.apk
adb -s <device> shell am instrument -w -e class app.pley.android.benchmark.TvStartupBenchmark `
  app.pley.android.benchmark/androidx.test.runner.AndroidJUnitRunner
```

On an emulator add `-e androidx.benchmark.suppressErrors EMULATOR`. The
traces are in `/storage/emulated/0/Android/data/app.pley.android.benchmark/cache`.
Benchmark builds start the demo server in the app's process, whose
catalogue takes CPU and memory a release never spends.

What a cold start does, in order: the startup read (the server, its
tokens, Home's snapshot) and the graph's warm-up start in the
background as the app starts; the TV's start facts (that read, "Always
start as" and the household's size) are read together as its activity is
made, so the first frame is the first screen; and the libraries'
initializers that would run on the main thread before it (EmojiCompat's,
OkHttp's) don't, OkHttp's running where the first client is made.
