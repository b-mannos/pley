# pley for Roku

A native SceneGraph app for Roku players and Roku TVs. It finds pley
servers on the local network (or takes an address), signs in with a code
shown on the TV and approved on the server's `/link` page (or a QR code of
it, which the server draws), and shows Home's rows, each library as a grid
of posters, a show's seasons and episodes, a title's details with Resume and
Play from start, Search with the on-screen keyboard, and Settings with the
profile's audio and subtitle defaults, Sort libraries, Switch server and
Sign out. The player plays files as they are whenever the
Roku can (it reports what it decodes, from `roDeviceInfo`), HLS otherwise,
reports progress, offers Skip Intro and Skip Credits as the web player does,
the next episode at the credits of one, and subtitle sync (Down). Music isn't played yet: music
libraries and their titles are left out.

It talks to the pley server and nothing else, so it works on a home network
without internet: pictures come from the server, the fonts are the Roku's
own, and the client ID (`https://hhaidar.github.io/pley/oauth/roku.json`) is
only a name the server knows; nobody fetches it.

The app is in `apps/roku`; paths below are relative to it.

## How it's built

| Path | What |
|---|---|
| `source/main.brs` | Starts the one scene |
| `components/MainScene` | The stack of screens, the HTTP agent of pictures, sign-in when the server ends it |
| `components/screens/` | Servers, sign-in with a code, Home, Library, Show, Details, Player, Search, Settings, "Can't reach your server" |
| `components/items/` | The tiles and rows of lists and grids, the player's skip button |
| `components/tasks/ApiTask` | The one place that talks to the server |
| `components/tasks/DiscoveryTask` | `pley?` to UDP port 7539 (apps/server/src/discovery.rs) |
| `components/lib/` | Plain BrightScript, no SceneGraph: addresses, sign-in decisions, token storage, the capability report, the server's JSON as tiles, times, the player's decisions |
| `tests/` | Unit tests of `components/lib`, run on a computer |
| `scripts/` | Test runner, smoke run, bench, the icons' generator |
| `licenses/` | Notices of what the package carries from others |

Rules the code keeps:

- Every HTTP call goes through `ApiTask`, a Task with all its transfers on
  one message port; the render thread never makes one. Lists (Home's rows,
  a library's pages, search, seasons, episodes) are made into ContentNodes
  there too, and reach the render thread whole.
- The access token goes in `Authorization: Bearer` only, never in an
  address: requests carry it, and so do the HTTP agents of pictures (the
  scene's) and of the Video node (playlists, segments, subtitles), which get
  a new agent each time the token changes.
- Refreshes run one at a time for each server, and the new pair is written
  to the registry before it is used: a refresh token works once, and two
  refreshes with the same one end the sign-in. A refresh is waited for two
  minutes, not given up on after the usual 30 s, and the pair it brings is
  kept only while the registry still holds the one it replaces: signed out,
  the server forgotten or signed in again meanwhile, it is revoked instead.
  A token refused moments after a refresh gave it ends the sign-in too, as
  in the Android app; so does one that runs out with nothing to refresh it.
  A revocation the server is too busy for (503) is sent again once.
- A stream that breaks is checked with the server: stopped by an
  administrator (410), the player leaves and says why; otherwise it starts
  again where it was with the current token. A file that fails before its
  first picture (anything but HTTP), or with a media error later, is played
  again converted, once. A session that answers after the player was left
  is closed at once.
- Subtitles: text ones go to the Video node as WebVTT; picture ones (PGS,
  VobSub, DVB) the person's choices pick are sent as `subtitle_index`, which
  the server draws onto the video, as the web player asks. Roku shows a
  subtitle track only with captions on, a setting of the whole Roku: when
  pley picks one, it turns captions on for that playback and back as they
  were after, unless the person changed them meanwhile.
- Subtitle sync, as the Android TV player's Sync: Down while a file with
  text subtitles plays brings it up at the top of the screen; left and
  right (rewind and fast forward too) move the subtitles 0.1 s, from 10 s
  earlier to 10 s later, shown as "+0.3 s"; Reset puts them back, Done or
  Back closes it. The Video node draws a sideloaded track as it comes and
  has no delay, so the server moves the cues (`offset_ms` on every text
  track's address, docs/API.md) and the stream opens again where it was,
  the same session, the subtitles that showed shown again. The offset is
  remembered for each file of each server in the registry (section
  `subtitleSync`, the latest 50 files) and moves the file's subtitles from
  the start the next time.
- Subtitles or audio picked in Roku's options while a title plays (captions
  turned on or off, another subtitle track, another audio track of a file
  played as it is) are saved as on Samsung and LG (`PUT
  /api/items/{id}/streams`, the file, its audio and its subtitles or `null`
  for off), so the next episode, or the movie next time, starts with
  matching tracks. What shows until the stream has played two seconds is
  pley's choice or the Roku's own, and so is what shows as subtitle sync
  opens the stream again: neither is saved. A subtitle track is told by its
  address (subtitle sync's `offset_ms` taken off); captions only on replay
  or on mute, and captions in the video itself, are left alone. Roku's
  audio tracks are taken for the file's audio streams in order, only when
  Roku lists as many as the file has and their languages agree; a converted
  stream has one. Text subtitles picked while the server draws picture ones
  onto the video start a stream without them, where it was. Subtitles
  changed alone are saved without the audio (`audio_index` left out), so
  turning captions on or off doesn't pin the track the server chose for
  this Roku on every other device.
- Audio: the play request names no audio track, so the server picks, of
  the profile's language (the choice remembered for the show or the movie,
  else the profile's defaults), the track this Roku plays best, and its
  answer's `audio_index` is what the app counts as playing. Roku doesn't
  report `audio_track_switching` and the app doesn't set the Video node's
  `audioTrack`, so any audio but the file's default makes the file a
  stream with its video copied.
- Track names in Roku's options: a subtitle track's title, else its
  language's name (its code for one not among the 34), "(Forced)" added
  only when the title doesn't say it already.
- Audio and subtitle defaults (Settings), as on Samsung and LG: Audio
  language (the file's default, or one of 34 languages), Subtitles (Off,
  Only for foreign dialogue, When the audio is in another language,
  Always, each with its hint), and Subtitle language (the audio's, or one
  of the 34) for the last two. OK opens a row's choices in a panel at the
  right; a choice shows at once and is saved with all three (`PUT
  /api/me/track-defaults`), the server's answer becoming the profile. A
  server or sign-in that doesn't let apps change them (403, 404, 405) puts
  the choice back and says to change them on the web, at the server's
  `/account`.
- Sort libraries (Settings), as on Android TV: OK lifts a library, up and
  down move it, OK puts it there, Back puts it back; "Use the server's
  order" goes back to the order everyone sees. Each drop saves the order
  at once (`PUT /api/me/libraries/order`, every library the profile may
  open, so music ones are listed too though Roku shows none), and the
  server's answer becomes the list; Home asks for its menu and rows again
  when it shows next. A server from before says sorting needs a newer
  pley, and nothing moves.
- Every request says `pleyRoku/<version> (Roku OS <version>; <model>)`, the
  model being `roDeviceInfo`'s display name: the server's `/link` page shows
  that part to whoever types the code.
- When the server doesn't answer, the screen that needed it gives way to
  "Can't reach …" with Retry and Switch server.
- The sign-in screen shows the server's QR code
  (`GET /api/oauth/device/qr`), fetched without the token since anyone may
  fetch it, once it has loaded: a server older than it answers 404, and the
  code and address are enough.
- Skip Intro and Skip Credits show while the video plays, from a marker's
  start until a second before its end, for the markers of the file playing;
  an episode's credits bring up Up Next instead. The button is a child of
  the Video node and has the focus while shown: OK skips; play pauses or
  goes on, fast forward and rewind move 10 s; Back leaves the player as
  ever. While Up Next counts down, Back cancels it, and play, fast forward
  and rewind cancel it and do the same.
- Home asks for its rows again only after something played, when they are a
  minute old (for what other devices played or marked meanwhile: a Roku's
  HTTP can't follow the server's event stream), or when Home is picked in
  its menu. The task makes again only the rows whose JSON changed; the
  others stay as they are, scrolled where they were, the focus stays on the
  same row and title, and the menu changes only with the libraries.
- A screen shown again takes the focus at once, before any answer, so the
  remote works whatever comes back; an answer moves the focus only while its
  screen has it, so a dialog over it keeps it.
- Roku's launch beacons: `AppLaunchComplete` once the first screen can be
  used (Home with its rows, or Servers or sign-in when the app opens on
  them), and `AppDialogInitiate` / `AppDialogComplete` around a sign-in
  shown at launch.

## Checking it on a computer

In `apps/roku`, with Bun and Node 22 or newer. Bun installs the packages
and runs the scripts, but the tools they call (bsc, bslint) run on Node:
apps/roku has no `bunfig.toml` making Bun their runtime, as bslint's
plugin loader needs Node's `module.require`.

```powershell
bun install
bun run check     # bsc's compile check, bslint, the unit tests
bun run smoke     # the whole app in brs-node, against a fake server
bun run bench     # the list work timed in brs-node
bun run preview   # the app in a browser, the keyboard as its remote
```

`bun run preview` runs the app in brs-node and serves it on
http://127.0.0.1:8086/: the TV's picture as brs-node draws it, with the
arrows, Enter (OK), Esc or Backspace (Back), Space (play/pause), `[` and `]`
(rewind, fast forward) and `*` (options) as the remote, and letters typed
on its keyboards. It runs against the smoke run's fake server, which allows
the TV code by itself; `bun run preview --server http://<address>:7539`
runs it against a real one instead, whose `/link` page allows the code.
`--port` chooses another port.

The unit tests run `components/lib` and `tests/*.test.brs` in brs-node
(lvcabral's BrightScript engine, whose command line and Node library moved
from `brs-engine` to `brs-node`): each `sub test_…()` is a test.
`bun run smoke` starts a fake pley server on 127.0.0.1
(`scripts/fixture-server.mjs`, answers written from docs/API.md) and walks
the app with the remote: sign-in with a code and its QR code, Home, a
title's details after a refused token, Back while the player starts (the
session that comes after is closed), the player with picture subtitles to
draw and its subtitle sync (shown, moved 0.3 s and remembered), Home
asking again after it, a library's second page, a show, Search,
Home not asking again after it, Settings, sorting the libraries (Movies put
after TV Shows, then the server's order again, Home asking again),
subtitles changed to Always (the languages sent as `null`), the server gone
and Retry, then
signing out and in again on a server without QR codes; then it starts the
app again, signed in, in a second process (brs-node runs one app per
process). It fails on any BrightScript error, on a request without the
User-Agent, a token in an address or sent for a QR code, a refresh token
used twice or two refreshes at once, and saves each screen in `out/smoke/`.
brs-node is a simulator: it draws with its own fonts, plays no video (so
Skip Intro and Up Next have only their unit tests), fetches every address
with a Node process of its own, one at a time, numbers transfers at random
below 80 (the API task keeps those under way apart), drops a remote key now
and then, and can't stand in for a Roku's speed.

`bun run images` draws the watched mark again (`scripts/make_images.py`,
Python's standard library only). The home screen icons (540 × 405 and
290 × 218, the P and the wordmark on pley's tile, as the Android TV app's
banner) and the splash screens (the logo on the app's background) are
rendered from pley's logo with the other apps' icons:
`bun scripts/render-icons.mjs` at the repository's root.

## Installing on a Roku

There is no channel in Roku's store yet; the app is sideloaded:

1. Turn on developer mode: with the Roku remote press Home three times, Up
   twice, then Right, Left, Right, Left, Right. Note the address shown,
   choose a password and let the Roku restart.
2. `bun run package` writes `out/pley-roku.zip`. bsc adds BrighterScript's
   runtime (`source/bslib.brs`, MIT) to every package and has no switch to
   leave it out; its notice goes along in `licenses/bslib.txt`.
3. Open the Roku's address in a browser, sign in as `rokudev` with that
   password, upload the zip and press Install. pley appears on the Home
   screen as a developer app; installing again replaces it.

`bunx bsc --deploy --host <address> --password <password>` packages and
installs in one go. The Roku's debug console (`telnet <address> 8085`) shows
what the app prints and its errors.

The manifest asks for SceneGraph 1.2, which every Roku OS since 9 runs.
Roku's store certification asks for 1.3 since October 2026, which needs Roku
OS 15; the app uses nothing of 1.3, so the switch is one line when it goes
to the store.

## Not yet

- Music libraries, "Who's watching?" and switching profiles.
- Hearing what other devices change at once: Home asks again once its rows
  are a minute old.
- Audio other than the file's default played as it is: reporting
  `audio_track_switching` and starting the Video node on the server's
  `audio_index`, once tried on a Roku.
- The Show screen asks for its seasons again each time it shows; Home's row
  fingerprints would spare that when nothing changed.
- Roku's channel store: a developer account, certification, SceneGraph 1.3
  and the launch beacons' order.
