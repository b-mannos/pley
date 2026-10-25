# Android and TV apps

One app, `app.pley.android`, for phones, tablets, Android TV, Google TV and
Fire TV (Fire OS 7 and later; Android 8 is the minimum). It is being rebuilt
as a native app in Kotlin and Compose. Until the native app replaces it,
release builds are still the web view app: a list of servers
(`ServersActivity`), the server's web app (`WebActivity`) and a Media3 player
(`PlayerActivity`). Debug builds carry both.

## Modules

Everything is in `apps/android`, sources in `src/main/kotlin` (the web view
app's in `src/main/java`). Each module sees only what it builds on.

| Module | What it holds | Builds on |
|---|---|---|
| `:app` | `PleyApplication`, the app's parts made by hand (`AppGraph`), `MainActivity` (phones), `TvActivity` (TVs), the web view app until it goes | everything |
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

Launchers:

- Release: the web view app's `ServersActivity` only.
- Debug: also "pley (native)" (`MainActivity`) and, on TV home screens,
  "pley (TV)" (`TvActivity`, with the 320×180 banner), from
  `app/src/debug/AndroidManifest.xml`.

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
