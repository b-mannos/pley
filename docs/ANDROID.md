# Android and TV apps

One app, `com.formandelement.pley`, for phones, tablets, Android TV, Google TV and
Fire TV (Fire OS 7 and later; Android 8 is the minimum), native in Kotlin
and Compose: `MainActivity` on phones and tablets, `TvActivity` on TVs. It
replaced a web view app (0.1: a list of servers, the server's web app and
a Media3 player), which an update clears up after (below).

## Installing

No store carries the app yet; release builds are signed when the release
key is given (Releases, below). Until then it is sideloaded, a debug build
or a signed release:

- From a computer: `adb install -r app-store-debug.apk` (`adb connect
  <ip>:5555` first for a TV or Fire TV, see Emulators).
- On a phone: open the APK in a file manager and allow that app to install
  apps when asked.
- On a Fire TV: turn on the developer options (below) and "Install unknown
  apps" for the Downloader app, then open the APK's address in Downloader.

The phone or tablet's launcher opens `MainActivity`; the TV home screen
opens `TvActivity`. Android TV's shows the 320×180 banner; Google TV's
draws the app's icon, the P, in a circle (`TvActivity` has no icon of its
own, so it is never the banner cut to a circle); Fire TV's shows a
sideloaded app by its icon in a square between grey bars, whatever the
icon's shape, and an app from its store by the store's pictures. A TV
that opens `MainActivity` (some launchers do) is sent on to `TvActivity`.

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
rewind and fast forward work in the player. Fire OS's home screen has no
Watch Next row (below), so nothing is published to its TV provider. Fire OS 7 (Android 9) is the
oldest supported; an `android-28` Android TV emulator with 1 GB of memory
stands in for a Fire TV stick (see Emulators), apart from video decoding:
its decoders are software only, which the app doesn't use on TVs, so it
can't play video.

## Audio and subtitles

The player names an audio track to the server (`audio_index`) only when
the person picked one, on the title's page or in the player's menu;
otherwise the server picks, of the profile's language, the track this
device plays best, and the player switches to the one its answer names,
which the menu marks. Tracks picked in the player, on another episode or
as new defaults are the ones the next episode and the title's page start
with (`PUT /api/items/{id}/streams`); subtitles changed alone are saved
without the audio, so the track one device played isn't pinned for every
other. Downloads, with no server to ask, still choose the audio here.
The profile's defaults (Audio language, Subtitles, Subtitle language)
are under Me on phones and in Settings on TVs
(`PUT /api/me/track-defaults`). The subtitle menu's Sync moves subtitles
in 0.1 s steps, up to 10 s either way, remembered for each file on the
device. Their size, background, font (sans serif, serif, monospace, and
the system's casual hand where it has one: not every Fire OS) and
position (at the bottom, or raised by 5, 10 or 15 % of the picture) are
Settings' own, also set in the player: the phone's menu has Subtitle
style, the TV's Subtitles panel lists them under the tracks. A font counts
as Pley's own style, over the system's captions; the position applies
either way, to text only (pictures keep their cue's place).

Find subtitles…, at the end of the subtitle menu while the file streams
from the server (not a download), lists what the server finds online for
the file in the profile's subtitle language (`GET
/api/items/{id}/subtitles/search`), the subtitles made for this very file
first; the one picked is added on the server (`POST
/api/items/{id}/subtitles`), joins the file's tracks as a subtitle file
beside the media, and shows. A server without search set up says
"Subtitle search isn't set up on this server"; an older one, that it can't
search.

The player plays at 0.5×, 0.75×, 1×, 1.25×, 1.5× or 2× (Speed: in the
phone's settings, a button at the TV's left), the sound's pitch kept, and
moves the sound up to 5 s earlier or later than the picture in 50 ms steps
(Audio delay, above the audio tracks; Left and Right on the TV). Both are
the title's only: the next one plays at 1× with no delay. The delay moves
the audio output's clock rather than the sound (`DelayedAudioSink`), so it
holds with passthrough too; the speed can't change for sound passed
through to a receiver, nor either of them with tunneled playback, where
the TV keeps the picture with the sound itself: the menus say so.

## Quality

The menu and Automatic are docs/QUALITY.md's. The quality is kept per
device and network kind (Settings → Quality on Wi-Fi and on mobile data on
phones, Quality on TVs), Automatic until one is chosen, in the player or
in Settings. Automatic starts at most at the ceiling the server keeps for
this player on this network (`GET /api/playback/network`, asked alongside
the item's details, 1.5 s at most), after a speed test when the server
asks for one (a network it knows nothing of, away from home: 2 s of `GET
/api/playback/speedtest` from its first byte, the result `PUT` back, all of
it 3.5 s at most, during Preparing); every ceiling Automatic finds or
forgets goes to the server too (`NetworkStart`). A server that can't
answer, or is too old to, leaves it to the device's own memory below. The decisions are `AutoQuality`'s (core/player), fed every
half second: rebuffering from the player's state, delivery from the
buffered position and the playhead, the network from Media3's bandwidth
estimate, and dropped frames as the recovery ladder counts them. It
settles as docs/QUALITY.md says: a step up that a step down undoes within
10 minutes isn't tried again during the title (the original included),
and each such step doubles the calm the next step up waits for (3, 6,
then 12 minutes, the network measured over all of it); a new title, or
a quality picked, starts afresh. At the original, dropped frames are the
ladder's switch to the server's quality,
as with Original (then Automatic offers Original in its menu rather than
Play original), and it never steps back up to the original; at a lower
quality they go to fewer lines. Automatic works out the server's quality
for a file itself (the server's rule) until the server names it, which it
does only when it converts. A server that converts too slowly makes
Automatic step down rather than offer a lower quality. The network's
ceiling is kept in the app's preferences for each server, on the
device's clock since it started: a ceiling from before a restart is
forgotten. Downloads play as they are, in Automatic too. Waits while the
server is out of reach (Reconnecting) aren't play issues. A transcode
comes in several sizes (`hls.variants`, docs/QUALITY.md) that Media3
moves between by itself, no bigger than the screen, and up a size only
with 20 s buffered (`minDurationForQualityIncreaseMs`, Media3's default
10 s; the rest of its `AdaptiveTrackSelection` as Media3 has it,
`Media3Engine.sizeSwitching`): in such a session
(`PlaybackInfo.adaptive`) the half-second feed leaves `AutoQuality` be,
and only stalls and dropped frames can still start it again lower.

A change that needs a new session while the video plays (a quality
picked, an audio track the engine can't switch to itself, picture
subtitles burned in, Automatic's steps, the switch to the server's
quality for dropped frames, Play original) doesn't stop it: the session
controller starts the next session beside the one playing
(`keep_previous`), asks for its playlist, the first size listed and the
segment holding the start plus 4 s through the engine's media client, so
the transcoder makes them, then ends the one playing and moves the engine
to the new one from where it got to. The wait is 20 s at most; meanwhile
the old one's stalls and dropped frames don't count. The screen keeps the
last picture on the surface across the swap (`PlayerUiState.handoff`
hides the black shutter until the new stream's first frame) and shows
the spinner only after 2.5 s instead of 0.4 s (0.5 s on TVs). With no
transcode free for two at once (`503`) the session before ends first, as
before. Recoveries from errors and stalls start the new session at once:
the stream playing is what failed. Media3's own moves between the sizes
of one stream keep their buffer: nothing in the app sets track selection
parameters when a size changes.

## Watch Next

On Android TV and Google TV, the home screen's Watch Next row shows the
profile's Continue Watching (home's row, in its order, ten at most):
titles watched part way as Continue, with their place, and episodes not
started as Next (`watchnext/`). It follows what the app already keeps
current while the TV app is in the foreground (progress, a title finished
or taken off Continue Watching, the server's events) and asks the server
for nothing of its own; another profile or server replaces its programs,
and signing out empties it. A program the person takes off the row is
left off. Selecting one opens the app on the title's page and plays it
from where it was, once its profile is the one signed in ("Who's
watching?" first, if the TV asks); otherwise the app just opens. The
home screen can't send the profile's token, so the app fetches the
pictures itself and hands them over from a read-only provider
(`WatchNextImageProvider`), by names no other app can guess. Not on
phones, and not on Fire TV, whose home screen has no such row.

## Screens

Libraries are sorted for the profile: on phones, Sort in the Libraries
tab's top bar (a handle to drag, Move up and Move down for TalkBack); on
TVs, Sort by the Libraries page's title, Move in a library's actions, and
Settings → Sort libraries, since with three libraries or fewer the rail
lists them and there is no Libraries page. Each move is saved at once,
and a server too old to sort says so. A row's See all (Recently Added,
Recently Released, Top Rated), on Home and in a library's Recommended
tab, shows the Library tab in the row's order, as the web's link does,
without changing the view the library was left in. On the TV, a title's Media info
stays closed until Select opens it, OK while a seek is coming or within
two seconds of one goes there and plays on (later, OK pauses), and the
rail keeps its panel until it has closed. Dolby Vision profile 7 is
labelled FEL or MEL, and a hybrid file shows both DV and HDR10+. With a
large system font, labels and poster titles shrink to fit rather than
break inside a word.

## Offline

The app needs no internet: the server signs Pley's own apps in itself
(their client documents are built in, never fetched), and on a phone
whose Wi-Fi has no internet, traffic to the server at home goes over the
Wi-Fi even while the phone's default network is mobile data
(`LocalRoute`). Away from the server, phones and tablets play downloads.

With the server out of reach, the screens show what was saved of them,
under "Showing what Pley had saved. Reconnecting…": a banner under the
top bar on phones (leading to the downloads); on TVs, a notice at the
top right for a few seconds, then a mark on the rail's avatar while it
lasts (no corner is free of controls on every TV screen). The server's event stream keeps trying, and the screens come back
on their own once it answers. A home address or name (`192.168.…`,
`nas.local`) has 3 s to accept a connection rather than OkHttp's 10, as
one at home accepts at once or never (`HttpRoot.homeConnectTimeout`), so
pressing play with the server off says "Can't reach your server" within
seconds, with Try again.

A playback that loses the server (the Wi-Fi dropping, the server
restarting) keeps its place: it plays what it has, then waits with
"Reconnecting…" under its ring while it tries again, up to 2 s apart, for
90 s from the first failure (`MediaRetries`; the stall ladder holds off
meanwhile), and goes on by itself once the server answers. Past that it
says "Can't reach your server", with Try again.

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
| `:app` | `PleyApplication`, the app's parts made by hand (`AppGraph`), `MainActivity` (phones), `TvActivity` (TVs), the TV's Watch Next row (`watchnext/`), the clean-up after the web view app (`migration/`) | everything |
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
| `:testing:fixtures` | `FixtureServer`, a fake Pley server on 127.0.0.1 | model |
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
and `direct` (the download the Pley server will offer, which may later hand
an update to the installer; it asks for that permission only once it does).
Release builds are minified, and signed when the release key is given
(below).

The version is Pley's, from `VERSION` at the repository's root, and the
version code follows from it (1.2.3 is 1020300), so every release installs
over the one before. Google Play and the Amazon Appstore take each code
once, so a rebuild of the same version for them sets `PLEY_BUILD` (1 to
99), which is added to the code (0.2.0 with `PLEY_BUILD=1` is 20001).
Builds record the commit they were made from
(`PLEY_COMMIT`, else git's); Settings and Diagnostics show both, as
"0.2.0 (1a2b3c4d5)".

### Releases

Release builds are signed with the key these environment variables name,
and unsigned without them:

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

The APK to sideload (phones, Android TV, Fire TV) is
`app/build/outputs/apk/direct/release/app-direct-release.apk`; the signed
bundle for the stores (`:app:bundleStoreRelease`) is
`app/build/outputs/bundle/storeRelease/app-store-release.aab`.

A tag's release workflow builds both, with the key from the repository's
secrets: `ANDROID_KEYSTORE_BASE64` (the keystore, base64-encoded),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and, when it differs,
`ANDROID_KEY_PASSWORD`. Without them the release has no Android app.
A new key is made once, and kept safe: every update must be signed with it.

```sh
keytool -genkeypair -v -keystore pley-release.keystore -alias pley \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 pley-release.keystore   # the ANDROID_KEYSTORE_BASE64 secret
```

Apps installed from a build signed with another key (a debug build) must
be uninstalled once before a release installs.

Google Play keeps the key that signs the app on devices (Play App Signing):
the key above is then the upload key, which Play can reset if it is lost.
The store's icon, feature graphic, TV banner and words are in
`deploy/stores`.

Launchers, the same in every build: "Pley" on phones and tablets
(`MainActivity`, `LAUNCHER`) and on TV home screens (`TvActivity`,
`LEANBACK_LAUNCHER`, with the 320×180 banner). Exported, beside these two,
are only the sign-in's return address (`SignInRedirectActivity`), the
music service (`MusicService`) and the Watch Next row's pictures
(`WatchNextImageProvider`); `CutoverTest` holds the project's manifests to
that.

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
testers run `adb shell cmd package compile -m speed-profile -f com.formandelement.pley`.

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
otherwise writes to a Pley server people use: signing in creates grants
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
device attached (a phone's own Pley among them):

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
