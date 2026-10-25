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
