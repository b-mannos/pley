# Samsung and LG TV apps

Pley for Samsung TV (`apps/tizen`, a Tizen `.wgt`) and Pley for LG TV
(`apps/webos`, a webOS `.ipk`) are one TV app in two shells. Everything
that doesn't call into the TV is in `packages/tv` (TypeScript and Preact):
the API client, sign-in, focus, screens, the player's session logic and
subtitles. Each shell adds its TV's player, remote keys, capability probe
and packaging. Both follow the Android TV app ([ANDROID.md](ANDROID.md)):
the same 960×540 layout, D-pad focus, screens and player features.

## Why packaged web apps

| | Samsung | LG |
|---|---|---|
| Package | Tizen web app (`.wgt`) | webOS web app (`.ipk`) |
| Player | AVPlay, Samsung's own player, on the hardware video plane | `<video src>`, which hands the address to LG's media pipeline (uMediaServer) |
| Native apps | Closed: no C/C++ apps; .NET apps get no better player | Closed without an LG agreement |
| Oldest TV | 2018 (Tizen 4.0, Chromium 56) sideloaded; the store takes new media apps for 2021 (Tizen 6.0) and later | 2018 (webOS 4.0, Chromium 53) |

Neither maker lets outside developers ship native apps, so both apps are
web apps that hand the file's address to the TV's own player, with its
hardware decoders, HDR and sound output. No video goes through a
JavaScript player (MSE).

## Installing

There is no store release: build the package ([Building](#building)), then
install it on a TV in developer mode.

### On a Samsung TV

1. **Certificate.** TVs from 2023 (Tizen 7.0) in developer mode, and the TV
   emulator, install only packages signed with a **Samsung certificate**
   ([install policy](https://developer.samsung.com/smarttv/develop/getting-started/using-sdk/tv-emulator/application-install-policy.html)).
   Make one in Samsung's Certificate Manager with a Samsung account; it
   lists the TVs' DUIDs. Older TVs take a Tizen author certificate. Keep
   the author certificate: every store update must be signed with it.
2. **Developer mode** on the TV: Apps, `12345` on the remote, Developer
   mode On with the computer's IP address, then restart the TV.
3. `sdb connect <TV address>`; `sdb devices` names the target.
4. With a Samsung certificate, `tizen install-permit -t <target>` once.
5. `tizen install -n pley.wgt -t <target> -- apps/tizen/out`.

### On an LG TV

1. Install **Developer Mode** from LG's store and sign in with an LG
   developer account. Turn Dev Mode Status on (the TV restarts), then Key
   Server on. A session lasts about 1,000 hours and can be extended;
   turning it off removes the apps it installed.
2. `bunx ares-setup-device --add tv -i "host=<TV address>" -i
   "port=9922" -i "username=prisoner"`
3. `bunx ares-novacom --device tv --getkey`, with the passphrase Developer
   Mode shows.
4. `bunx ares-install -d tv apps/webos/out/app.pley.tv_<version>_all.ipk`,
   then `bunx ares-launch -d tv app.pley.tv`. `bunx ares-inspect -d tv -a
   app.pley.tv --open` opens the web inspector.

LG's CLI (`@webos-tools/cli`, a dev dependency, run with `bunx`) comes with
`bun install`. LG signs store builds itself.

### Stores

- **Samsung Apps TV Seller Office:** a public seller launches in the
  United States only; other countries need a partner contract. Reviewers
  need a demo server. Mandatory: launch within 10 s (5 s ideal), no screen
  saver during playback, subtitles in sync, a message when the network
  drops, Back on the first screen asks before exiting, the Exit key always
  exits
  ([checklist](https://developer.samsung.com/smarttv/develop/development-checklist/mandatory-features.html)).
- **LG Seller Lounge:** free registration; review in three stages
  (pretest, function, content), every update re-tested. It needs an IPK, a
  UX scenario, the self-checklist, a privacy policy and a server reviewers
  can sign in to. Back on the first screen opens the TV's exit popup
  (webOS 6 and later; older ones go Home)
  ([self-checklist](https://webostv.developer.lge.com/distribute/app-self-checklist)).

## Samsung

### What a Samsung TV plays

From Samsung's per-year media
[specifications](https://developer.samsung.com/smarttv/develop/specifications/general-specifications.html)
and field reports:

| Year | Tizen | Chromium | Beyond H.264 and HEVC Main 10 | Notes |
|---|---|---|---|---|
| 2018 | 4.0 | 56 | VP9 Profile 0 and 2 | no DTS |
| 2019 | 5.0 | 63 | VP9; 8K HEVC on 8K sets | |
| 2020 | 5.5 | 69 | AV1 | fMP4 in HLS |
| 2021 | 6.0 | 76 | AV1 | codec queries, HTTP status in player errors; the store's floor |
| 2022 | 6.5 | 85 | AV1 | |
| 2023 | 7.0 | 94 | AV1 | OS upgrades start; developer mode needs a Samsung certificate |
| 2024 | 8.0 | 108 | AV1 | HLS version 10 |
| 2025 | 9.0 | 120 | AV1 | |
| 2026 | 10.0 | 130 | AV1 | "HDR10+ Advanced" on some sets |

- **Containers:** MKV, MP4, TS and older ones (AVI, MOV, ASF); HEVC only
  from MKV, MP4 and TS.
- **HDR:** HDR10 and HDR10+ on all UHD sets; HLG on HDR sets from 2018
  (field practice). **No Samsung TV shows Dolby Vision**: the server sends
  the HDR10 base layer or converts. The TV switches HDR from the stream;
  `webapis.avinfo.isHdrTvSupport()` says whether the panel shows HDR.
- **Sound:** AAC, AC-3, E-AC-3 (5.1 decoded), Opus, AC-4, FLAC, MP3,
  Vorbis, PCM. **No DTS** on 2018–2026 TVs, decoded or passed through.
  **E-AC-3 with Atmos** reaches a receiver as it is when the TV's digital
  output passes Dolby; apps can read that setting
  (`tizen.tvaudiocontrol.getOutputMode()`) but not change it.
  **TrueHD:** apps reportedly can't pass it over eARC, though
  `webapis.systeminfo.isSupportedAudioCodec("TrueHD")` exists from Tizen
  6.0.
- **`<video>` and MSE** stop H.264 at 1080p; 4K and 8K need AVPlay.
- **AVPlay sends no request headers** but a cookie and a User-Agent, so
  the app plays from keyed addresses ([API.md](API.md#keyed-urls)).
- **CORS is enforced** for packaged apps (requests from `file://`); the
  server answers CORS for those origins.
- **Subtitles:** AVPlay takes external files from local paths only, so
  Pley draws its own and keeps AVPlay's silent.

### How Pley plays on a Samsung TV

The capabilities report (`apps/tizen/src/caps.ts`, [version 2](API.md#version-2))
comes from the model year (from the Tizen version), the panel (4K, 8K,
HDR), codec queries on Tizen 6.0 and later (AV1, AC-4, TrueHD), and the
sound output:

- H.264 and HEVC Main 10 to 4K60 (8K on 8K sets), VP9 Profile 2, AV1 from
  2020; HDR10, HDR10+ and HLG on HDR panels; never Dolby Vision.
- AC-3 and E-AC-3 passed through when the TV's output is set to Dolby
  (DOLBY: AC-3; DOLBY_DIGITAL_PLUS: both). TrueHD and DTS only when
  Settings → Playback asks ("Send Dolby TrueHD as it is", "Send DTS as it
  is").
- HLS with TS segments; fMP4 from 2020.

AVPlay rules the engine keeps: one call at a time while a seek runs; no
seek to 0 (a start before 1 s starts at the beginning); stop and close
before the next source; the picture placed in 1920×1080 coordinates; 4K
allowed for HLS; on Tizen 6.0 and later a `410` in its error stops
playback with the server's message; screen saver off during playback.
When the TV hides the app, the session closes and reopens at the same
place when it comes back.

## LG

### What an LG TV plays

From LG's per-version
[specifications](https://webostv.developer.lge.com/develop/specifications/web-api-and-web-engine)
and field reports:

| Year | webOS | Chromium | HLS | Notes |
|---|---|---|---|---|
| 2018 | 4.0 | 53 | v5 | DTS decoded |
| 2019 | 4.5 | 53 | v5 | DTS decoded |
| 2020 | 5 | 68 | v7 | AV1 and VP9 at 4K; no DTS |
| 2021 | 6 | 79 | v7 | |
| 2022 | 22 | 87 | v7 | |
| 2023 | 23 | 94 | v7 | DTS on some models; native HLS sends surround as stereo |
| 2024 | 24 | 108 | v7 | same |
| 2025 | 25 | 120 | v7 | Dolby Vision from MKV; stereo bug fixed; no DTS |
| 2026 | 26 | 132 | v7 | stereo bug again; `requiredACG` honored |

Some 2022 and later sets get newer webOS versions (webOS Re:New), so the
app goes by the running version, never the model year.

- **Containers:** MKV, MP4, TS, AVI and older ones. H.264 and HEVC Main 10
  at 4K; AV1 and VP9 at 4K from webOS 5; 8K HEVC and AV1 on 8K sets.
- **HDR:** HDR10 and HLG where `webOS.deviceInfo()` says `hdr10`. **Dolby
  Vision** profiles 5 and 8 where it says `dolbyVision` (profile 7 plays
  its HDR10 base layer); before webOS 25 only from MP4 and TS. **No LG TV
  shows HDR10+.**
- **Sound:** AAC, AC-3, E-AC-3, Opus, FLAC (2 channels), MP3, PCM. AC-3
  and E-AC-3 (Atmos included) pass through over ARC, eARC or optical when
  the digital output isn't PCM; apps can read these settings but not
  change them. Multichannel PCM never leaves an app. **No TrueHD** out of
  an app. **DTS:** decoded on 2018–2019 sets; passed through (DTS, DTS-HD,
  DTS:X) on some 2023–2024 sets; otherwise none.
- **LG's HLS player and surround:** on webOS 23, 24 and 26 multichannel
  sound reaches a receiver as stereo (LG promised a webOS 26 fix for late
  2026). Files played as they are aren't affected.
- **Start position** goes in the source's `mediaOption`
  (`option.transmission.playTime.start`); seeking after load buffers from
  the start first.
- **No request headers, no cookies:** the app plays from keyed addresses.
- **CORS** varies by firmware, so the server always answers it for
  `file://` and `null` origins and the app signs in with bearer tokens
  only. Scripts are classic bundles: module scripts fail from `file://`.
- **Subtitles:** LG lists WebVTT only and embedded tracks don't show as
  text tracks, so Pley draws its own.
- **Permissions:** `requiredACG` in `appinfo.json` lists the system
  services an app calls (honored from webOS 26, required by the store from
  webOS 27). Pley declares `systemconfig.query`, `settings.query`,
  `arc.query` and `network.query`.

### How Pley plays on an LG TV

The capabilities report (`apps/webos/src/caps.ts`) comes from the webOS
version, `webOS.deviceInfo()` (panel, HDR, Dolby Vision, Atmos) and the
sound settings (`com.webos.settingsservice`; the receiver's formats from
`tv.model.edidType`):

- H.264 to 4K (60 on webOS 4.0, 30 after), HEVC Main 10 to 4K60 (8K on 8K
  sets), VP9 Profile 2 and AV1 from webOS 5; HDR10, HLG; Dolby Vision
  profiles 5 and 8 where the TV has it.
- A Dolby Vision MKV before webOS 25 is requested as a remux, so it keeps
  Dolby Vision (`packages/tv/src/player/plan.ts`).
- AC-3 and E-AC-3 passed through as above; DTS passed through on webOS 23
  and 24 when the receiver takes it; TrueHD and DTS otherwise only when
  Settings asks.
- HLS with TS segments; fMP4 from webOS 5.

The `<video>` gets one `<source>` whose type carries the `mediaOption`:
start position, the pipeline's reports (`umsmediainfo`, for Playback
details) and, for HLS, 4K allowed.

## How the apps work

`packages/tv/src`:

| Path | What |
|---|---|
| `api/`, `net/`, `auth/` | HTTP API, event stream, server discovery, sign-in with a code (QR and short link), profiles and PINs |
| `state.ts`, `platform.ts` | What the TV keeps (servers, sign-ins, settings, a per-install player id); what a shell provides |
| `ui/focus.ts`, `ui/input.ts`, `ui/keys.ts` | Focus (rows, grids, remembered places), remote keys (held keys at most every 100 ms, long OK), LG's Magic Remote pointer and wheel |
| `ui/shell/`, `ui/screens/`, `ui/kit/` | The rail, back stack and screens: Home, libraries (A–Z, the profile's order), items, people, collections, playlists (smart ones from a library's filters), search, music, Settings, Diagnostics, history |
| `player/` | Playback: the play request, recovery, quality, Up Next, skip markers, seek previews, sleep timer, reports, background music |
| `live/` | Live TV: what's on, the filters, the guide's chunks and cells, and a channel watched on the TV's player |
| `subs/` | Subtitles drawn by the app: text as WebVTT, pictures (PGS, VobSub, DVB) decoded from the server's `.mks` onto a canvas |
| `caps/` | The server's direct-play rules, so Playback details can say why |

The picture subtitle decoders (the `.mks` reader, PGS, VobSub, DVB, and
the track that says what shows when) are in `packages/picture-subtitles`
(`pley-picture-subtitles`), which the web app draws with too: plain
TypeScript without dependencies, compiled by each TV build for its own
engine; `bun run tv:check` runs its tests first.

Each shell has `facts.ts` (what the TV says about itself; LG's system
services in `luna.ts`), `caps.ts` (the report, with snapshot tests),
`engine.ts` (AVPlay, or `<video>` with `mediaOption`), `platform.ts`,
`main.ts`, the manifest (`config.xml`, `appinfo.json`) and `scripts/`.

Sign-in, server discovery (a sweep of the TV's subnet), the player's keys
and panels, and offline behavior follow the Android TV app.
[API.md](API.md) has the server's side: OAuth clients `tizen.json` and
`webos.json`, keyed addresses, CORS.

**Built for a 2018 TV:** one script and one stylesheet packaged with the
fonts, and the first screen drawn from saved data before any network;
compiled for Chrome 53 (LG) and 56 (Samsung) with small polyfills; laid
out at 960×540 and zoomed to the TV's graphics plane; only `transform`
and `opacity` animate; a focus move redraws only what changes; rows and
grids draw only what is near the screen, pictures at their drawn width,
nearest focus first; focus resting 400 ms on a title prefetches what
playing it needs.

**Remote and pointer:** Next and Previous move between a film's chapters
(Next plays a playlist's or collection's next title). LG's Magic Remote
works as a mouse: tiles come into view when pointed at, the wheel moves a
row a notch, a click on the player's bar seeks there. Sorting libraries
on an older server says "Sorting libraries needs a newer Pley on the
server."

### Audio and subtitle tracks

- The server picks the audio track, in the profile's language, that this
  TV plays best, unless the person picked one on the title's page or in
  the player (`audio_index`).
- Picks are saved for the show or movie (`PUT /api/items/{id}/streams`);
  subtitles picked alone are saved without the audio. The next episode
  starts with the tracks picked while its Up Next showed. Subtitles forced
  only by their title say Forced in the track menus.
- Settings → Profile sets the defaults (Audio language, Subtitles, Subtitle
  language; `PUT /api/me/track-defaults`).
- Subtitle sync: 0.1 s steps, up to 10 s either way, kept per file on the
  TV.
- Settings → Subtitles and display: size, background, font (Default,
  Serif, Monospace, Casual) and position (Bottom, A little higher, Higher,
  Highest: raised 5, 10 or 16 % of the height), kept on the TV. A font is
  offered only if the TV has a face for it. Picture
  subtitles keep their own position.
- Find subtitles (`GET /api/items/{id}/subtitles/search`) lists
  OpenSubtitles results in the profile's subtitle language (or another
  picked there), those made for this file first; one picked is
  added on the server for everyone and shows at once. Without OpenSubtitles
  set up (`503`, `setup: false`) the search says so and points an
  administrator to Settings → Server; other profiles lose the entry, as
  does everyone on a server too old to search (`404`). The TVs have no
  file upload and no delete.

### Playback speed

0.5×, 0.75×, 1.25×, 1.5× or 2× for the title playing. On LG through
`playbackRate`, which keeps pitch. Samsung offers none: AVPlay's speeds are
trick play, 2× and up, with no promise about sound.

### Quality

The menu, Original and Automatic follow [QUALITY.md](QUALITY.md#the-menu),
in the player's Quality panel and Settings → Playback → Quality. Automatic
is the default until a quality is picked on this TV (`pley.qualityPicked`).

Both TVs' players go black between streams, so Automatic follows the
[rules for such players](QUALITY.md#players-that-go-black-between-streams),
in `packages/tv/src/player/autoquality.ts`. A transcode comes in one size
(neither TV says `hls.variants`). TV specifics:

- **Signals:** waits are the engine buffering while playing (not a pause,
  seek or new session). LG measures delivery and network speed from its
  buffer; Samsung has only waits and, for a stream, AVPlay's
  `CURRENT_BANDWIDTH`. Neither TV counts dropped frames.
- **Decoding:** an original that fails, or drops frames in its first
  minute, falls back once to the server's quality.
- **Bitrate changed as it is sent:** the server's answer is the bitrate it
  goes on at; a new stream of the same quality (recovery, track change)
  starts at the bitrate the last one reached; a `400` (older transcoder)
  makes the stream one whose bitrate can't change.
- **Memory on the TV:** 30 minutes per server
  (`pley.autoQuality.<server>`), used when the server doesn't answer; an
  older server (`404`) isn't asked again until the app restarts.
- **Shown:** a toast on a step down; the Quality row reads "Automatic · 4
  Mbps 1080p", with the reason under it and in Info.
- **Speed test:** 32 MiB read through XMLHttpRequest progress, stopped 2 s
  after the first byte (or 1 s without one), so it delays the start by at
  most about 3.5 s; not repeated within 10 minutes. The network question
  goes out with the item's details and capabilities probe.

**Switching streams** (a quality picked, an audio track the engine can't
switch, burned-in picture subtitles, Automatic's step from the original,
Play original; `player/prepare.ts`): the new session starts beside the old
one (`keep_previous`), the app waits up to 20 s for its playlist and the
segment at start + 4 s (`HEAD`), then moves the engine there. The screen is
black meanwhile on both TVs, and the loading ring waits 2.5 s. With no
transcode free for two (`503`), the old session ends first.

### The web player

`apps/web/src/player` shares these rules. Web-only facts: the slow network
is kept in the browser per server (`pley.autoQuality`), the browser asks
the server's network memory as one client (`pley.networkClient`), `<` and
`>` step the playback speed, Subtitle style (font and position) is kept in
the browser and applies to the iPhone's fullscreen player, and Find
subtitles also uploads a file (.srt, .vtt, .ass, .ssa, up to 2 MB) and
deletes added tracks where the server allows (`deletable`).

### Live TV

The web app's Live TV ([API.md](API.md#live-tv)) on a remote. The rail has
Live TV after Home when `GET /api/live` says `available` (kept on the TV,
so it shows at launch); sources, the lineup and the options stay in Pley on
the web, as the TVs' other administration does.

- **The page:** a category of channels at a time, as TiviMate shows them,
  in the Guide or as Channels (both kept on the TV). Left from the
  channels opens the categories: Search at their head, then Favourites,
  Recently watched and All channels, then Regions, in alphabetical order,
  each opening to its groups ("UK| Sport", "UK| News"…), and More groups,
  the rest in the provider's order (a region of one group is one of them;
  with fewer than two regions, the groups are one plain list). Search
  (Left goes to it from anywhere in the list; OK opens the TV's own
  keyboard) looks among every channel by name, number or what's on,
  whatever category was open (`browse?category=all&q=`), and the header
  offers the groups whose names match to go to; Back leaves the results
  for the category before. In a group, channel names leave out the region
  it says. The channels come a page of 100 at a time (`GET /api/live/browse`) as they
  scroll into view, and the guide only for the rows drawn (`ids`), so a
  lineup of tens of thousands opens at once. The guide shows two and a
  half hours, four px a minute, with now marked; Up and Down keep the time
  focus is at, Left and Right go programme by programme and move the guide
  through time, rewind and fast-forward (or Previous and Next) move it two
  hours, CH+ and CH− a screen of channels, and Back comes back to now
  before it goes to the rail. OK on what's on watches the channel; on
  anything else (or a long OK, or the menu key) it opens the programme's
  details, with Watch and the channel's favourite. A channel without a
  guide shows its name across it. Channels shows cards with what's on, its
  progress and what's next. Typing a channel's number goes to it, in the
  category shown, else among every channel. Back from the player lands on
  the channel watched last, in the category it was played in.
- **The player:** `POST …/watch` with this TV's id (`pley.installId`), so
  changing channels holds one connection, and `keyed_urls`, so AVPlay and
  LG's pipeline play the playlist without the token. The app waits for the
  playlist (the server answers once the channel's first segments are
  made), then plays it from its live edge (LG: no `playTime` in the
  `mediaOption`). Up and Down, CH+ and CH−, Previous and Next change
  channels within the category it was opened from, round its ends (the
  page `around` the channel, and the one before or after at its edges);
  digits tune by number among every channel; OK shows the controls (the
  channel list, favourite, back and forward 10 s, Play/Pause, Live or how
  far behind it, channel down and up). A live channel has no position
  bar: Live sits with the buttons, and behind live it is the button that
  goes back. The menu key (and Samsung's CH LIST) opens the channel list,
  a page at a time as it scrolls, open at the channel playing, beside the
  picture: the picture and its controls move aside into the 520 px left of
  it (AVPlay's plane is placed there too) rather than going under it. Left
  in the list opens the categories, with the same search, to list another
  (changing to one of its channels makes CH+ and CH− follow it). What's on
  shows only when asked: Info, or OK with the controls; not as each
  channel starts. Tuning shows the channel's logo with the spinner centred
  under it. Pause and rewind
  stay within the server's timeshift window, as the engine reports it
  (`video.seekable` on LG, AVPlay's `GET_LIVE_DURATION`); without one, the
  10 s buttons hide. Paused, the playlist is asked for every 20 s, as the
  server lets an HLS viewer go after 45 s of nothing. A channel with no
  picture shows its logo and what's on.
- **Leaving:** `DELETE /api/live/sessions/{id}` on Back, on each change of
  channel, and when the TV hides the app (it watches again from live when
  it shows). A player failure is watched anew twice (1 s, then 3 s), then
  said, with Try again, Other channels and Back; `409` shows the server's
  words naming the channels holding the connections; an older server
  without keyed addresses is asked to be updated.
- **Keys:** Samsung's `ChannelUp`, `ChannelDown`, `ChannelList` and `Guide`
  are registered; LG's CH+ and CH− are 33 and 34 (Page Up and Page Down
  on a keyboard).

## Building

With Bun, from the repository root (`bun install` first). A `bunfig.toml`
in each package has Bun run the tools (TypeScript, Vitest, LG's CLI):

```sh
bun run tv:check                     # type checks and tests: the shared package, Samsung, LG
bun run --cwd apps/tizen package     # apps/tizen/out/pley.wgt, signed (bun run tizen too)
bun run --cwd apps/webos package     # apps/webos/out/app.pley.tv_<version>_all.ipk (bun run webos too)
bun run --cwd apps/webos build       # only an app's dist/ (apps/tizen too); add --dev to keep it readable
```

Tests run in Vitest with jsdom.

**Samsung packaging** needs the Tizen SDK 10.0 CLI
([download.tizen.org](https://download.tizen.org/sdk/Installer/tizen-sdk_10.0/);
Tizen Studio is deprecated), found through `TIZEN_STUDIO` or `tizen` on the
PATH, and a security profile named by `PLEY_TIZEN_PROFILE` (default
`pley-dev`).

**Icons:** `bun scripts/render-icons.mjs` draws Samsung's 512×423 icon,
LG's 80 and 130 px icons (matched by `iconColor` in `appinfo.json`) and
LG's 1920×1080 launch picture (`splashBackground`). The stores want their
own on publication: Samsung a 512×423 and a 1920×1080 logo, LG a 400×400
icon.

## Testing without a TV

- **LG app in a browser:** `bun run --cwd apps/webos serve` builds with
  `--dev`, serves on `http://localhost:7650` (`PORT` to change) and
  proxies everything else to a Pley server (`PLEY_SERVER`, default
  `http://127.0.0.1:7640`). Arrows, Enter and Escape (Back) are the remote.
  It plays what the browser plays. `localStorage["pley.screenReader"] =
  "on"` stands in for the TV's screen reader.
- **Samsung TV emulator 10.0** (`download.tizen.org/sdk/extensions/tv_extensions`):
  no 4K or DRM; installs only Samsung-certificate packages. Its installer
  wants Intel HAXM, which can't run beside Hyper-V; on Windows 11 with
  Hyper-V, unpack its packages into the SDK by hand and it runs on WHPX.
- **LG Simulator** (webOS 22 and later): the TV's Chromium without its
  media pipeline, so no `mediaOption`, DRM or TV formats. It downloads
  only through LG's license page.

## What still needs a real TV

Samsung, on a 2018–2019 set and a 2021 or later one:

1. Install with a Samsung certificate; launch time.
2. Time to first frame, direct and transcoded.
3. HDR10+ from MKV, HLG, Dolby Vision profile 8 showing as HDR10.
4. E-AC-3 Atmos to a receiver; TrueHD when Settings sends it.
5. Audio track switching; AVPlay's subtitles silent; picture subtitles.
6. fMP4 HLS on 2020–2021 sets; 4K HLS.
7. A session stopped from the web (`410`), on Tizen 6.0+ and older.
8. No screen saver; Home and back; the network dropping.
9. D-pad smoothness and memory on a 2018 set.
10. Automatic's speed test on a remote network.
11. Available subtitle fonts; AVPlay at 2× with sound.
12. Live TV: a keyed live playlist from its edge; `GET_LIVE_DURATION`, pause
    and rewind in the window; CH+, CH−, CH LIST and GUIDE registered; a
    radio channel; `410` when a channel is stopped from the web.

LG, on a webOS 4–6 set and a webOS 23 or later one:

1. Install in Developer Mode; launch time.
2. Dolby Vision 5 and 8 from MP4, TS (and MKV on webOS 25); the remux
   before webOS 25.
3. Transcoded surround on webOS 23–24 and 26; if stereo, HLS through
   hls.js.
4. CORS on each firmware.
5. Audio track switching; `audioTracks` order against the server's.
6. `umsmediainfo` in Playback details; start position from `mediaOption`.
7. DTS to a receiver on 2023–2024; what `tv.model.edidType` says.
8. Back on the first screen; `requiredACG` on webOS 26.
9. The Magic Remote: OK selects once, wheel moves a notch, pointer hides.
10. Audio Guidance on each move; Samsung's Voice Guide.
11. Playback speed: pitch, streams and files, after a seek.
12. Automatic's speed test on a remote network; available subtitle fonts.
13. Live TV: a live playlist without `playTime` starting at its edge;
    `video.seekable` for pause and rewind; CH+ and CH− as 33 and 34.

## References

Samsung:
[testing FAQ (Q25)](https://developer.samsung.com/smarttv/develop/faq/application-testing.html),
[hosted apps FAQ](https://developer.samsung.com/smarttv/develop/faq/hosted-applications.html),
[multimedia FAQ](https://developer.samsung.com/smarttv/develop/faq/multimedia.html),
[WebAssembly](https://developer.samsung.com/smarttv/develop/extension-libraries/webassembly/overview.html),
[ESPlayer](https://developer.samsung.com/smarttv/develop/api-references/tizenfx-tv-api-references/tizen.tv.multimedia/esplayer-class.html),
[AVPlay guide](https://developer.samsung.com/smarttv/develop/guides/multimedia/media-playback/using-avplay.html),
[AVPlay API](https://developer.samsung.com/smarttv/develop/api-references/samsung-product-api-references/avplay-api.html),
[adaptive streaming](https://developer.samsung.com/smarttv/develop/guides/multimedia/adaptive-streaming.html),
[web engine](https://developer.samsung.com/smarttv/develop/specifications/web-engine-specifications.html),
[CORS](https://github.com/Samsung/tizen-docs/blob/master/docs/application/web/guides/w3c/security/cors.md),
[certificates](https://developer.samsung.com/smarttv/develop/getting-started/setting-up-sdk/creating-certificates.html),
[TV device setup](https://developer.samsung.com/smarttv/develop/getting-started/using-sdk/tv-device.html),
[Return and Exit](https://developer.samsung.com/smarttv/develop/guides/fundamentals/terminating-applications.html),
[Seller Office membership](https://developer.samsung.com/tv-seller-office/guides/membership/becoming-seller-office-member.html),
[registering an app](https://developer.samsung.com/tv-seller-office/guides/applications/registering-application.html),
[eARC](https://www.samsung.com/hk_en/support/tv-audio-video/what-is-earc-and-how-to-set-on-samsung-smart-tv/).

LG:
[Developer Mode app](https://webostv.developer.lge.com/develop/getting-started/developer-mode-app),
[CLI guide](https://webostv.developer.lge.com/develop/tools/cli-dev-guide),
[Simulator](https://webostv.developer.lge.com/develop/tools/simulator-introduction),
[appinfo.json](https://webostv.developer.lge.com/develop/references/appinfo-json),
[ACG guide](https://webostv.developer.lge.com/develop/guides/acg-guide),
[streaming protocols](https://webostv.developer.lge.com/develop/specifications/streaming-protocol-drm),
[resuming media](https://webostv.developer.lge.com/develop/guides/resuming-media-with-mediaoption),
[app approval](https://webostv.developer.lge.com/distribute/app-approval-process),
[Flutter for webOS](https://webostv.developer.lge.com/develop/guides/flutter-for-webos)
and its [video player](https://github.com/lg-flutter-webos/plugins/blob/main/packages/video_player/API_REFERENCE.md),
[webOS OSE media client](https://github.com/webosose/chromium108/blob/master/src/media/neva/webos/umediaclient_impl.cc),
[webosbrew developer mode](https://www.webosbrew.org/devmode/); forum threads on
[multichannel audio with Dolby Vision](https://forum.webostv.developer.lge.com/t/stereo-only-for-dolby-vision-video-with-multi-channel-audio-in-hls-streams/23069),
[CORS in packaged apps](https://forum.webostv.developer.lge.com/t/how-are-we-supposted-to-handle-cors-with-a-non-hosted-app/28241)
and [module scripts under file://](https://forum.webostv.developer.lge.com/t/module-script-loads-blocked-under-file-origin-for-installed-webos-app-no-cors-workaround-found/28351).

Other projects and reports:
[Microsoft's Xamarin support policy](https://dotnet.microsoft.com/en-us/platform/support/policy/xamarin),
[plx-native #754](https://github.com/GLinnik21/plx-native/discussions/754),
[plx-native #221](https://github.com/GLinnik21/plx-native/issues/221),
[jellyfin-tizen #222](https://github.com/jellyfin/jellyfin-tizen/issues/222),
[jellyfin #16092](https://github.com/jellyfin/jellyfin/issues/16092),
[jellyfin-web's device profile](https://github.com/jellyfin/jellyfin-web/blob/master/src/scripts/browserDeviceProfile.js),
[Samsung Jellyfin Installer FAQ](https://github.com/Jellyfin2Samsung/Samsung-Jellyfin-Installer/wiki/FAQ),
[Moonfin's Tizen player](https://github.com/Moonfin-Client/Smart-TV/blob/main/packages/platform-tizen/src/video.js),
[CE Pro on HDR10+](https://www.cepro.com/news/samsung_amazon_hdr10/2495/),
[BGR on Dolby Vision](https://www.bgr.com/2260222/why-samsung-tv-doesnt-support-dolby-vision/),
[HDTVTest on DTS](https://www.hdtvtest.co.uk/news/lg-quitely-drops-support-for-dts-sound-on-its-2025-t-vs).
