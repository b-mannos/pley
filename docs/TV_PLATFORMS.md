# Samsung and LG TV apps

pley for Samsung TV (`apps/tizen`, a Tizen `.wgt`) and pley for LG TV
(`apps/webos`, a webOS `.ipk`) are one TV app in two shells. Everything
that isn't a call into the TV is in `packages/tv` (TypeScript and Preact,
about 20,000 lines with its styles): the API client, sign-in, focus, the
screens, the player's session logic and the subtitles. Each shell adds its
TV's player, remote keys, capability probe and packaging (800 lines for
Samsung, 630 for LG). Both follow the Android TV app
([ANDROID.md](ANDROID.md)): the same 960×540 layout, D-pad focus, screens
and player features.

This page covers what each TV maker lets an outside developer ship (as
found in October 2026, with sources), the route chosen and why, how the
apps play, how to build, install and test them, and what still needs a
real TV.

## The decision

| | Samsung | LG |
|---|---|---|
| Route | Packaged Tizen web app (`.wgt`) | Packaged webOS web app (`.ipk`) |
| Player | AVPlay, Samsung's own player, on the hardware video plane | `<video src>`, which hands the address to LG's media pipeline |
| Native apps | Closed: C/C++ apps can't be installed; .NET apps can, with no better player | Closed to anyone without an LG agreement |
| Oldest TV | 2018 (Tizen 4.0, Chromium 56), sideloaded; the store takes new media apps for 2021 (Tizen 6.0) and later | 2018 (webOS 4.0, Chromium 53) |
| Shared code | everything but the shell | everything but the shell |

**Native apps are closed on both, so neither app is native.** What matters
for playback is still native on both: each app hands the file's address to
the TV's own player, the one the TV maker's apps use, with its hardware
decoders, HDR and sound output. Neither app plays video through a
JavaScript player (MSE).

- **Samsung.** Samsung's FAQ says its TVs run web apps (`.wgt`) only, not
  native `.tpk` packages
  ([FAQ, Q25](https://developer.samsung.com/smarttv/develop/faq/application-testing.html)).
  C/C++ runs only as WebAssembly inside a web app, from 2020's TVs
  ([WebAssembly](https://developer.samsung.com/smarttv/develop/extension-libraries/webassembly/overview.html)).
  .NET (C#) apps are still accepted for 2018 and later TVs
  ([TSO](https://developer.samsung.com/tv-seller-office/guides/applications/registering-application.html)),
  but Samsung's .NET guides are built on Xamarin.Forms, whose support
  Microsoft ended in May 2024
  ([Microsoft](https://dotnet.microsoft.com/en-us/platform/support/policy/xamarin)),
  its elementary-stream player is for partners only
  ([ESPlayer](https://developer.samsung.com/smarttv/develop/api-references/tizenfx-tv-api-references/tizen.tv.multimedia/esplayer-class.html)),
  and its public player is the platform player AVPlay already drives.
  Hosted web apps (loaded from a server) are refused except for partners
  ([FAQ](https://developer.samsung.com/smarttv/develop/faq/hosted-applications.html)).
  AVPlay is what Samsung points apps to for 4K, adaptive streams and
  track switching
  ([AVPlay guide](https://developer.samsung.com/smarttv/develop/guides/multimedia/media-playback/using-avplay.html)),
  and what other media apps on Samsung TVs use.
- **LG.** A store app's `type` is `web`
  ([appinfo.json](https://webostv.developer.lge.com/develop/references/appinfo-json)).
  In October 2026 LG's Seller Lounge told a developer in writing that
  native apps can't be registered or distributed by general developers
  ([plx-native #754](https://github.com/GLinnik21/plx-native/discussions/754)).
  LG's Flutter for webOS (2026) makes native apps, but only for webOS 26
  and later, with a Linux-only toolchain, a video player that sends no
  request headers and picks no audio track, and no word that the store
  takes such apps from non-partners
  ([guide](https://webostv.developer.lge.com/develop/guides/flutter-for-webos),
  [video player](https://github.com/lg-flutter-webos/plugins/blob/main/packages/video_player/API_REFERENCE.md)).
  A `<video src>` (not MSE) in a web app goes to LG's media pipeline
  (uMediaServer), which fetches and decodes the file itself
  ([webOS OSE source](https://github.com/webosose/chromium108/blob/master/src/media/neva/webos/umediaclient_impl.cc)).
- **Sharing.** Both TVs run Chromium-based engines, so sharing the app
  costs nothing: one bundle per TV, built from the same source, each
  compiled for its oldest engine.

## Samsung

### What a Samsung TV plays

From Samsung's per-year media specifications
(`developer.samsung.com/smarttv/develop/specifications/media-specifications/{year}-tv-video-specifications.html`),
its general and web-engine specifications
([general](https://developer.samsung.com/smarttv/develop/specifications/general-specifications.html),
[web engine](https://developer.samsung.com/smarttv/develop/specifications/web-engine-specifications.html)),
and field reports where Samsung says nothing:

| Year | Tizen | Chromium | Beyond H.264 and HEVC (Main 10) | Notes |
|---|---|---|---|---|
| 2018 | 4.0 | 56 | VP9 Profile 0 and 2 | DTS dropped |
| 2019 | 5.0 | 63 | VP9; 8K HEVC on 8K sets | |
| 2020 | 5.5 | 69 | AV1 | fMP4 in HLS from here ([streaming guide](https://developer.samsung.com/smarttv/develop/guides/multimedia/adaptive-streaming.html)) |
| 2021 | 6.0 | 76 | AV1 | codec queries, HTTP status in player errors; the store's floor for new media apps |
| 2022 | 6.5 | 85 | AV1 | |
| 2023 | 7.0 | 94 | AV1 | OS upgrades start; a Samsung certificate is needed for developer mode |
| 2024 | 8.0 | 108 | AV1 | HLS version 10 |
| 2025 | 9.0 | 120 | AV1 | |
| 2026 | 10.0 | 130 | AV1 | "HDR10+ Advanced" on some sets |

- **Containers:** MKV, MP4, TS and the older ones (AVI, MOV, ASF); HEVC
  only from MKV, MP4 and TS.
- **HDR:** HDR10 and HDR10+ (all UHD sets from 2017,
  [CE Pro](https://www.cepro.com/news/samsung_amazon_hdr10/2495/)), HLG on
  HDR sets from 2018 (field practice, not stated by Samsung). **No Samsung
  TV shows Dolby Vision**, 2026 included
  ([BGR](https://www.bgr.com/2260222/why-samsung-tv-doesnt-support-dolby-vision/)):
  the server sends such files' HDR10 base layer or converts them.
  `webapis.avinfo.isHdrTvSupport()` says whether a panel shows HDR at all.
  The TV switches HDR from the stream; there is nothing for an app to do.
- **Sound:** AAC, AC-3, E-AC-3 (5.1 decoded), Opus and AC-4 from 2018,
  FLAC, MP3, Vorbis, PCM. **DTS** was decoded through 2017 and is
  neither decoded nor passed to a receiver on 2018–2026 TVs (spec pages;
  Samsung's [eARC table](https://www.samsung.com/hk_en/support/tv-audio-video/what-is-earc-and-how-to-set-on-samsung-smart-tv/)).
  **E-AC-3 with Atmos** reaches a soundbar or receiver as it is when the
  TV's digital output is set to pass Dolby, which an app reads with
  `tizen.tvaudiocontrol.getOutputMode()` and can't change
  ([FAQ, Q12](https://developer.samsung.com/smarttv/develop/faq/multimedia.html)).
  **TrueHD** is in no app specification, and owners report TV apps can't
  pass it over eARC, though `webapis.systeminfo.isSupportedAudioCodec("TrueHD")` exists from
  Tizen 6.0.
- **Through `<video>` and MSE**, H.264 stops at 1080p; 4K and 8K need
  AVPlay ([FAQ](https://developer.samsung.com/smarttv/develop/faq/multimedia.html)).
- **AVPlay sends no request headers** but a cookie and a User-Agent
  ([AVPlay API](https://developer.samsung.com/smarttv/develop/api-references/samsung-product-api-references/avplay-api.html)),
  so it can't send a bearer token: the apps play from keyed addresses
  ([API.md, Keyed URLs](API.md#keyed-urls)), as other media servers'
  clients put their tokens in their URLs.
- **CORS is enforced** for packaged apps, whose requests come from
  `file://` ([Tizen CORS guide](https://github.com/Samsung/tizen-docs/blob/master/docs/application/web/guides/w3c/security/cors.md)):
  the server answers CORS for those origins ([API.md](API.md)).
- **Subtitles:** AVPlay renders a few formats itself but takes external
  files from local paths only; pley draws its own (below) and keeps
  AVPlay's silent.

### How pley plays on a Samsung TV

The capabilities report (`apps/tizen/src/caps.ts`, version 2, see
[API.md](API.md#version-2)) comes from the set's model year (from its
Tizen version, since each year ships one), its panel (`productinfo`:
4K, 8K; `avinfo`: HDR), its codec answers on Tizen 6.0 and later (AV1,
AC-4, TrueHD), and its sound output:

- H.264 to 4K60, HEVC Main 10 to 4K60 (8K on 8K sets), VP9 Profile 2, AV1
  from 2020 sets; HDR10, HDR10+ from 2017 and HLG from 2018 on HDR panels;
  never Dolby Vision.
- AC-3 and E-AC-3 sent to the receiver as they are when the TV's output
  is set to Dolby (DOLBY: AC-3; DOLBY_DIGITAL_PLUS: both); TrueHD and DTS
  only when Settings → Playback asks ("Send Dolby TrueHD as it is", "Send
  DTS as it is").
- HLS from the server with TS segments, fMP4 from 2020 sets.

AVPlay is driven by its documented rules and what Moonfin and others
learned on real sets
([Moonfin](https://github.com/Moonfin-Client/Smart-TV/blob/main/packages/platform-tizen/src/video.js)):
one call at a time while a seek runs, no seek to 0 (a start before 1 s
starts at the beginning), stop and close before the next source, the
picture placed in 1920×1080 coordinates whatever the page's size, 4K
allowed for HLS (`FIXED_MAX_RESOLUTION=3840X2160`), its own subtitles
silenced, the HTTP status read from its error messages on Tizen 6.0 and
later (a `410` stops playback with the server's message), and the screen
saver off while a video plays. When the TV hides the app (Home, another
app), the session is closed and a new one opens at the same place when
it comes back.

### Signing, developer mode, the emulator and the store

- **Certificates.** A Tizen author certificate (made by the SDK's CLI)
  signs packages, but Samsung TVs from 2023 (Tizen 7.0) in developer mode,
  and the TV emulator since TV Extension 7.0.1, install only packages
  signed with a **Samsung certificate**
  ([install policy](https://developer.samsung.com/smarttv/develop/getting-started/using-sdk/tv-emulator/application-install-policy.html),
  [field](https://github.com/Jellyfin2Samsung/Samsung-Jellyfin-Installer/wiki/FAQ)).
  Making one needs Samsung's Certificate Manager and a Samsung account;
  its distributor certificate lists the TVs' DUIDs
  ([certificates](https://developer.samsung.com/smarttv/develop/getting-started/setting-up-sdk/creating-certificates.html)).
  Keep the author certificate: every store update must be signed by it.
- **Developer mode** on the TV: Apps, then `12345` on the remote, then
  Developer mode On with the computer's IP address, then restart
  ([TV device](https://developer.samsung.com/smarttv/develop/getting-started/using-sdk/tv-device.html)).
- **SDK.** Tizen Studio is deprecated; the Tizen SDK 10.0 CLI
  (`web-cli_Tizen_SDK_10.0_windows-64.exe` from
  [download.tizen.org](https://download.tizen.org/sdk/Installer/tizen-sdk_10.0/))
  packages and installs. The TV emulator 10.0 comes from
  `download.tizen.org/sdk/extensions/tv_extensions`; it can't play 4K or
  DRM, and Samsung's installer insists on Intel HAXM, which can't run
  beside Hyper-V. On Windows 11 with Hyper-V the emulator's packages can
  be unpacked into the SDK by hand and it runs on WHPX.
- **Store (Samsung Apps TV Seller Office).** A public seller launches in
  the United States only; other countries need a partner contract
  ([membership](https://developer.samsung.com/tv-seller-office/guides/membership/becoming-seller-office-member.html)).
  Jellyfin, the closest case, got in after about two years, for 2021 and
  later TVs, with a demo server for the reviewers
  ([jellyfin-tizen #222](https://github.com/jellyfin/jellyfin-tizen/issues/222)).
  The mandatory checks include: launch within 10 seconds (5 ideal), no
  screen saver during playback, subtitles in sync, a message when the
  network drops, Back on the first screen asks before exiting, and the
  Exit key always exits
  ([mandatory features](https://developer.samsung.com/smarttv/develop/development-checklist/mandatory-features.html),
  [Return and Exit](https://developer.samsung.com/smarttv/develop/guides/fundamentals/terminating-applications.html)).

## LG

### What an LG TV plays

From LG's format pages for each webOS version
(`webostv.developer.lge.com/develop/specifications/video-audio-{30…260}`),
its [web engine](https://webostv.developer.lge.com/develop/specifications/web-api-and-web-engine)
and [streaming](https://webostv.developer.lge.com/develop/specifications/streaming-protocol-drm)
pages, and field reports:

| Year | webOS | Chromium | HLS | Notes |
|---|---|---|---|---|
| 2018 | 4.0 | 53 | v5 | DTS decoded |
| 2019 | 4.5 | 53 | v5 | DTS decoded |
| 2020 | 5 | 68 | v7 | AV1 and VP9 at 4K; DTS dropped |
| 2021 | 6 | 79 | v7 | |
| 2022 | 22 | 87 | v7 | |
| 2023 | 23 | 94 | v7 | DTS back on some models; native HLS sends multichannel sound as stereo |
| 2024 | 24 | 108 | v7 | same |
| 2025 | 25 | 120 | v7 | Dolby Vision from MKV; the stereo bug fixed; DTS dropped |
| 2026 | 26 | 132 | v7 | the stereo bug back; `requiredACG` honoured |

Selected 2022 and later sets get newer webOS versions (webOS Re:New), so a
2022 set may run webOS 25: the app goes by the running version, never by
the model year.

- **Containers:** MKV, MP4, TS, AVI and the older ones. H.264 and HEVC
  Main 10 at 4K, AV1 and VP9 at 4K from webOS 5, 8K HEVC and AV1 on 8K
  sets.
- **HDR:** HDR10 and HLG where `webOS.deviceInfo()` says `hdr10`. **Dolby
  Vision** profiles 5 and 8 where it says `dolbyVision`; profile 7 plays
  its HDR10 base layer. Before webOS 25, Dolby Vision shows only from MP4
  and TS: from MKV the TV shows the base layer
  ([jellyfin #16092](https://github.com/jellyfin/jellyfin/issues/16092),
  [jellyfin-web](https://github.com/jellyfin/jellyfin-web/blob/master/src/scripts/browserDeviceProfile.js)).
  **No LG TV shows HDR10+** (2026 C6 included).
- **Sound:** AAC, AC-3, E-AC-3, Opus, FLAC (2 channels), MP3, PCM. AC-3
  and E-AC-3, Atmos included, go to a receiver as they are when the
  sound goes out over ARC, eARC or optical and the digital output isn't
  PCM; apps can read these settings but not change them. Multichannel PCM
  never leaves an app
  ([forum 23069](https://forum.webostv.developer.lge.com/t/stereo-only-for-dolby-vision-video-with-multi-channel-audio-in-hls-streams/23069)).
  **TrueHD** has no known path out of an app on any LG
  ([plx-native #221](https://github.com/GLinnik21/plx-native/issues/221)).
  **DTS**: decoded on 2018–2019 sets, dropped in 2020, back on some
  2023–2024 sets (DTS, DTS-HD and DTS:X to a receiver), dropped again in
  2025 ([HDTVTest](https://www.hdtvtest.co.uk/news/lg-quitely-drops-support-for-dts-sound-on-its-2025-t-vs)).
- **Native HLS and surround:** with LG's own HLS player, multichannel
  sound reaches a receiver as stereo on webOS 23 and 24 (fixed in webOS
  25's firmware) and again on webOS 26, where LG promised a fix for late
  2026 ([forum 23069](https://forum.webostv.developer.lge.com/t/stereo-only-for-dolby-vision-video-with-multi-channel-audio-in-hls-streams/23069)).
  Files played as they are aren't affected.
- **Start position:** LG wants it in the `mediaOption` passed with the
  source (`option.transmission.playTime.start`): setting the time once
  the file loads buffers from the beginning first
  ([resuming media](https://webostv.developer.lge.com/develop/guides/resuming-media-with-mediaoption)).
- **No request headers, no cookies.** The pipeline sends only what the
  web engine gives it, and packaged apps have no cookies: the apps play
  from keyed addresses ([API.md](API.md#keyed-urls)).
- **CORS** differs between firmware versions (bypassed on some, enforced
  on others, [forum 28241](https://forum.webostv.developer.lge.com/t/how-are-we-supposted-to-handle-cors-with-a-non-hosted-app/28241)),
  so the server always answers it for `file://` and `null` origins, and
  the app signs in with bearer tokens only. Scripts are classic bundles,
  since module scripts fail from `file://`
  ([forum 28351](https://forum.webostv.developer.lge.com/t/module-script-loads-blocked-under-file-origin-for-installed-webos-app-no-cors-workaround-found/28351)).
- **Subtitles:** LG lists WebVTT only, and embedded tracks don't show up
  as text tracks; pley draws its own.
- **Permissions:** `requiredACG` in `appinfo.json` lists the system
  services an app calls; webOS 26 honours it and the store requires it
  from webOS 27 ([ACG guide](https://webostv.developer.lge.com/develop/guides/acg-guide)).
  pley declares `systemconfig.query`, `settings.query`, `arc.query` and
  `network.query`.

### How pley plays on an LG TV

The capabilities report (`apps/webos/src/caps.ts`) comes from the webOS
version, `webOS.deviceInfo()` (panel, HDR, Dolby Vision, Atmos) and the
sound settings (`com.webos.settingsservice`, with the HDMI sink's audio
formats from `tv.model.edidType`):

- H.264 to 4K (60 on webOS 4.0, 30 after), HEVC Main 10 to 4K60 (8K on 8K
  sets), VP9 Profile 2 and AV1 from webOS 5; HDR10 and HLG; Dolby Vision
  profiles 5 and 8 where the TV has it.
- A Dolby Vision file in a container the TV doesn't show it from (MKV
  before webOS 25) is asked for as a remux up front, so it keeps its
  Dolby Vision (`dvContainerMode` in `packages/tv/src/player/plan.ts`).
- AC-3 and E-AC-3 to a receiver as above; DTS to a receiver on webOS 23
  and 24 when the receiver takes it; TrueHD and DTS otherwise only when
  Settings asks.
- HLS with TS segments, fMP4 from webOS 5.

The `<video>` element gets one `<source>` whose type carries the
`mediaOption`: the start position, the pipeline's reports
(`umsmediainfo`, for Playback details) and, for HLS, 4K allowed. Frame
counters aren't read on LG, so a struggling stream isn't judged by them.

### Developer mode, the Simulator and the store

- **Developer Mode** app from the LG store, signed in with an LG
  developer account; a session lasts about 1,000 hours and can be
  extended; turning it off removes the apps it installed
  ([Developer Mode](https://webostv.developer.lge.com/develop/getting-started/developer-mode-app),
  [webosbrew](https://www.webosbrew.org/devmode/)).
- **CLI:** `@webos-tools/cli` (3.2.6 wants Node 20 or newer; here a dev
  dependency, run with `npx`): `ares-setup-device`, `ares-novacom --getkey`, `ares-package`,
  `ares-install`, `ares-launch`, `ares-inspect`
  ([CLI guide](https://webostv.developer.lge.com/develop/tools/cli-dev-guide)).
- **Simulator** (webOS 22 and later; the VirtualBox emulator stops at
  webOS 6 and is deprecated): it runs the TV's Chromium but not its media
  pipeline, so no `mediaOption`, DRM or TV formats
  ([Simulator](https://webostv.developer.lge.com/develop/tools/simulator-introduction)).
  It downloads only through LG's licence page.
- **Store (LG Seller Lounge):** free registration, review in three
  stages (pretest, function, content), every update re-tested; an IPK, a
  UX scenario, the self-checklist and a privacy policy; reviewers need a
  server to sign in to. Back on the first screen opens the TV's exit
  popup (webOS 6 and later; older ones go Home)
  ([approval](https://webostv.developer.lge.com/distribute/app-approval-process),
  [self-checklist](https://webostv.developer.lge.com/distribute/app-self-checklist)).

## How the apps work

`packages/tv/src`:

| Path | What |
|---|---|
| `api/`, `net/`, `auth/` | The HTTP API, the event stream, finding a server, signing in with a code (QR and short link), profiles and their PINs |
| `state.ts`, `platform.ts` | What the TV keeps (servers, sign-ins, settings, a per-install player id), and what a shell provides |
| `ui/focus.ts`, `ui/input.ts`, `ui/keys.ts` | Focus moved by the app: rows, columns, grids and free groups, remembered places, panels that keep focus; the remote's keys, held keys at most every 100 ms, long OK |
| `ui/shell/`, `ui/screens/`, `ui/kit/` | The rail, the back stack and the screens: Home (Continue Watching with remove, My List), libraries with A–Z, items, people, collections, playlists, search, music, Settings, history |
| `player/` | The playback session: the play request, the recovery ladder, quality, Up Next, skip markers, seek previews, the sleep timer, reports, music in the background |
| `subs/` | Subtitles drawn by the app: text as WebVTT from the server, pictures (PGS, VobSub, DVB) decoded from the server's `.mks` and drawn on a canvas, with the offset the person sets |
| `caps/` | The server's direct-play rules, so the player knows what a play request will get, and Playback details can say why |

Each shell has `facts.ts` (what the TV says about itself; on LG through
its system services, `luna.ts`), `caps.ts` (the report, a pure function
with snapshot tests), `engine.ts` (the player: AVPlay on Samsung, a
`<video>` with a `mediaOption` on LG), `platform.ts` (keys, lifecycle,
storage), `main.ts`, the package's manifest (`config.xml`,
`appinfo.json`) and `scripts/` (building, packaging, and LG's browser
server).

What keeps them quick on a 2018 TV:

- One script (410 KB minified, 135 KB compressed) and one stylesheet,
  packaged with the fonts: nothing is downloaded before the first screen,
  which comes from what the TV saved, before any network.
- Preact with signals, compiled ahead for Chrome 53 (LG) and 56
  (Samsung), with a few small polyfills, and a prelude ahead of the
  script for what esbuild's own helpers read as the script starts.
- The page is laid out at 960×540 and zoomed to the TV's graphics plane
  (1920×1080, or 1280×720 on LG's HD sets).
- Only `transform` and `opacity` move, on the compositor; no filters,
  blurs, big shadows, or anything that lays out a frame (the rail's
  entries grow by scaling, the scrubber's thumb too). Held keys move rows
  without animating.
- A move draws only what follows it: tiles are drawn again only when what
  they show changes, and what follows focus (Home's backdrop and info,
  the episode info, the A–Z jump) reads it from a signal, so the page and
  its rows aren't drawn again. The player's half-second tick draws only
  its clock.
- Rows and grids draw only what is near the screen. Pictures are asked
  for at the width they are drawn (the server's width buckets) and loaded
  nearest-focus first, fewer at a time on low-end sets.
- When focus rests on a title for 400 ms, what playing it needs is
  fetched, so OK starts playback at once.

Signing in, finding the server on the network (a sweep of the TV's own
subnet), the player's keys and panels, and the offline behaviour follow
the Android TV app; `docs/API.md` has the server's side (built-in OAuth
clients `tizen.json` and `webos.json`, keyed addresses, CORS).

## Building, installing, testing

```sh
npm run tv:check                     # type checks and tests: the shared package, Samsung, LG
npm run tizen                        # apps/tizen/out/pley.wgt, signed
npm run webos                        # apps/webos/out/app.pley.tv_<version>_all.ipk
node apps/webos/scripts/build.mjs    # only an app's dist/ (apps/tizen too); --dev keeps it readable
```

The tests run in Vitest with jsdom: focus moves, keys, playback
negotiation, the seek keys, real TVs' capabilities as snapshots, AVPlay's
errors.

`node scripts/render-icons.mjs` draws the apps' pictures from the logo,
the P on the logo's tile colour (#0a1622) to the edges, as 24-bit RGB PNGs
at 72 dpi: Samsung's 512×423 icon (Samsung asks for that format), LG's 80
and 130 px icons, which `iconColor` in `appinfo.json` matches on the TV's
own tile, and LG's 1920×1080 launch picture (`splashBackground`). The
stores want their own on top when the apps are published: Samsung's
Seller Office a 512×423 and a 1920×1080 logo with its background, LG's
Seller Lounge a 400×400 icon.

**Samsung.** Packaging needs the Tizen SDK's command-line tools
(`TIZEN_STUDIO` set to the SDK's folder, or `tizen` on the PATH) and a
security profile, named by `PLEY_TIZEN_PROFILE` (default `pley-dev`). A
Tizen author certificate makes a package, but a TV from 2023 on in
developer mode, and the TV emulator, want a Samsung certificate, made in
Samsung's Certificate Manager with the TV's DUID. Keep the author
certificate: the store wants every update signed with it. To install:

1. On the TV: Apps, `12345` on the remote, Developer mode On with this
   computer's address, then restart the TV.
2. `sdb connect <TV address>`; `sdb devices` names the target.
3. With a Samsung certificate, `tizen install-permit -t <target>` once.
4. `tizen install -n pley.wgt -t <target> -- apps/tizen/out`.

**LG.** LG's command-line tools (`@webos-tools/cli`) come with the
workspace's dependencies; run them with `npx`. LG signs store builds
itself. To install:

1. On the TV: Developer Mode from LG's store, signed in with an LG
   developer account; Dev Mode Status on (the TV restarts), then Key
   Server on.
2. `npx ares-setup-device --add tv -i "host=<TV address>" -i
   "port=9922" -i "username=prisoner"`
3. `npx ares-novacom --device tv --getkey`, with the passphrase Developer
   Mode shows.
4. `npx ares-install -d tv apps/webos/out/app.pley.tv_1.0.0_all.ipk`,
   then `npx ares-launch -d tv app.pley.tv`. `npx ares-inspect -d tv -a
   app.pley.tv --open` opens the web inspector.

**Without a TV.** `node apps/webos/scripts/build.mjs --dev`, then `node
apps/webos/scripts/serve.mjs`, serves the LG build on
`http://localhost:7650` and passes everything else to a pley server
(`PLEY_SERVER`, default `http://127.0.0.1:7640`), so the app and the
server share an origin. At 960×540 it is the TV's layout; arrows, Enter
and Escape (Back) are the remote. It plays what the browser plays.

## What still needs a real TV

Samsung, on a 2018–2019 set and a 2021 or later one:

1. Install with a Samsung certificate; launch time (10 s at most, 5 s
   ideal).
2. Time to first frame for a 4K HEVC HDR10 film played as it is, and for
   a transcode (HLS), against the Android TV app's 2.7 s.
3. HDR10+ from an MKV (the TV's info shows HDR10+), HLG, and a Dolby
   Vision profile 8 film showing as HDR10.
4. E-AC-3 with Atmos to a receiver with the output on Dolby Digital+;
   what TrueHD does when Settings sends it.
5. Switching the audio track while playing; AVPlay's subtitles staying
   silent under pley's; picture subtitles from the `.mks`.
6. fMP4 HLS on 2020–2021 sets; 4K HLS.
7. A session stopped from the web (`410`), on Tizen 6.0 and later and on
   older sets (no HTTP status in their errors).
8. No screen saver during playback; Home and back (the session reopens
   at the same place); the network dropping.
9. D-pad smoothness and memory on a 2018 set.

LG, on a webOS 4–6 set and a webOS 23 or later one:

1. Install in Developer Mode; launch time.
2. Dolby Vision profiles 5 and 8 from MP4 and TS (and MKV on webOS 25),
   and the remux asked for an MKV before webOS 25; the first Dolby Vision
   film after launch sometimes showing as HDR10.
3. A transcode's surround sound to a receiver on webOS 23–24 firmware
   before the fix, and on webOS 26: if it comes out stereo, HLS through
   hls.js.
4. CORS on each firmware (a sign-in and the API with `Authorization`).
5. Audio track switching while playing a file as it is, and the order of
   `audioTracks` against the server's.
6. `umsmediainfo` reaching Playback details; the start position from
   `mediaOption`.
7. DTS to a receiver on a 2023–2024 set; what `tv.model.edidType` says.
8. Back on the first screen (the TV's exit popup) and `requiredACG` on
   webOS 26.

