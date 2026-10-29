# Pley for Roku

A native SceneGraph app for Roku players and Roku TVs, in `apps/roku` (paths
below are relative to it). It is not in Roku's channel store yet: it is
sideloaded in developer mode.

The app talks to the Pley server and nothing else, so it works on a home
network without internet: pictures come from the server, the fonts are the
Roku's own, and the client ID
(`https://hhaidar.github.io/pley/oauth/roku.json`) is only a name the
server knows; nobody fetches it.

## Installing on a Roku

1. Turn on developer mode: on the Roku remote press Home three times, Up
   twice, then Right, Left, Right, Left, Right. Note the address shown,
   choose a password and let the Roku restart.
2. `bun run package` (in `apps/roku`) writes `out/pley-roku.zip`. bsc adds
   BrighterScript's runtime (`source/bslib.brs`, MIT) to every package and
   can't leave it out; its notice ships in `licenses/bslib.txt`.
3. Open the Roku's address in a browser, sign in as `rokudev` with that
   password, upload the zip and press Install. Pley appears on the Home
   screen as a developer app; installing again replaces it.

`bunx bsc --deploy --host <address> --password <password>` packages and
installs in one go. The Roku's debug console (`telnet <address> 8085`)
shows what the app prints and its errors.

The manifest asks for SceneGraph 1.2, which every Roku OS from 9 runs.
Roku's store certification asks for 1.3 from October 2026, which needs
Roku OS 15. The app uses nothing of 1.3, so the switch is one line when it
goes to the store.

## What it does

It finds Pley servers on the local network (or takes an address), signs
in with a code shown on the TV and approved on the server's `/link` page
(or a QR code of it, drawn by the server), and asks "Who's watching?" at
start when the household has more than one profile. Then:

- **Home's rows**, and each library with the web app's tabs (Recommended,
  Library sorted and filtered, Collections).
- **A title's page:** Resume and Play from start, a show's next episode
  and its episodes, Mark as watched, My List, Go to show, and rows of
  seasons, cast and crew, extras, More Like This and the collections it is
  in; people's and collections' pages.
- **Music:** artists, albums and their tracks, in a music player that
  keeps playing while the app is used, with Now Playing.
- **Playlists:** Play, Shuffle, Rename, Delete, remove an entry.
- **Search** with the on-screen keyboard (titles and people).
- **Settings:** the profile's audio and subtitle defaults, Quality, Play
  the next episode by itself, Watch history (remove a play with the
  Options key), Switch profile, Sort libraries, Switch server, Sign out.
- **The Options key (\*)** on a title: Mark as watched or unwatched, Add
  to or Remove from My List, Add to playlist (or a new one named on the
  keyboard), Remove from Continue Watching, Go to show.
- **The player** plays files as they are whenever the Roku can (it reports
  what it decodes, from `roDeviceInfo`), HLS otherwise. It reports
  progress, offers Skip Intro and Skip Credits as the web player does, Up
  Next at an episode's credits (a playlist's next entry), and Pley's menu
  (Down): Quality, subtitle sync, the file's chapters, Find subtitles…
  and Playback details.

Not offered: playback speed and audio delay. The Video node has no field
for a playback rate on current Roku OS (its reference lists `control` and
`seek`) nor for an audio delay. Subtitles look as
the Roku's own caption settings say (Settings → Accessibility → Captions
style: size, font, colors, edges, background, opacity). The app never sets
the Video node's `captionStyle`, which would override the person's choices
for the whole Roku.

## Playback

### Pley's menu

Down while a title plays opens Pley's menu at the right, over the video,
which plays on (play, fast forward and rewind still work). Back goes back
in it, then closes it. Subtitles and audio stay in Roku's own options (the
Options key), which know the Video node's tracks. The menu holds:

- **Quality** ("Automatic · Original"; [below](#quality)).
- **Subtitle sync**, for a file with text subtitles (below).
- **Chapters**, for a file that has them: OK goes to one; the one playing
  is checked.
- **Find subtitles…** (below).
- **Playback details**, as the other apps say them: how it plays ("Direct
  play", "Direct stream", "Transcoding") and why, its video and audio as
  they are and as the server converts them (`GET /api/playback/{session}`),
  its bitrate, what the screen gets of HDR, subtitles burned in, and how
  far ahead the conversion is.

### Errors and stopped streams

A stream that breaks is checked with the server. Stopped by an
administrator (`410`), the player leaves and says why; otherwise it starts
again where it was with the current token. A file that fails before its
first picture (anything but HTTP), or with a media error later, is played
again converted at the server's own quality, once. A session that answers
after the player was left is closed at once.

### Subtitles

- **Text subtitles** go to the Video node as WebVTT. **Picture subtitles**
  (PGS, VobSub, DVB) the person's choices pick are sent as
  `subtitle_index`, and the server draws them onto the video, as for the
  web player.
- **Captions:** Roku shows a subtitle track only with captions on, a
  setting of the whole Roku. When Pley picks a track, it turns captions on
  for that playback and back as they were after, unless the person changed
  them meanwhile.
- **Track names** in Roku's options: a subtitle track's title, else its
  language's name (its code for one not among the 34), "(Forced)" added
  only when the title doesn't say it already.
- **Subtitle sync** (as the Android TV player's Sync): it shows at the top
  of the screen; left and right (rewind and fast forward too) move the
  subtitles 0.1 s, from 10 s earlier to 10 s later, shown as "+0.3 s";
  Reset puts them back, Done or Back closes it. The Video node draws a
  sideloaded track as it comes and has no delay, so the server moves the
  cues (`offset_ms` on every text track's address, [API.md](API.md)) and
  the stream opens again where it was, the same session, with the same
  subtitles showing. The offset is remembered per file per server in the
  registry (section `subtitleSync`, the latest 50 files) and applies from
  the start next time.
- **Find subtitles…** (`components/lib/SubtitleSearch.brs`): the server
  looks up the file on OpenSubtitles in the person's subtitle language
  (`GET /api/items/{id}/subtitles/search?file_id=`, no `language`, so the
  server takes the profile's, English without one). Results are listed in
  the server's order, those made for this very file first: "English ·
  Movie.2019.1080p.BluRay"; under the list, for the one in focus, the
  whole name, "Made for this file", "Hearing impaired", "Forced" and the
  downloads. OK adds it on the server (`POST /api/items/{id}/subtitles`
  with the result and the file). The answer is an ordinary text track: it
  joins the session's tracks, is saved as the person's pick for the show
  or movie (`PUT /api/items/{id}/streams`), and shows with captions turned
  on, the stream reopening where it was as subtitle sync does (a new
  session where the server drew picture subtitles). "Subtitles added:
  English" shows at the top. A server without OpenSubtitles set up (`503`,
  `setup: false`): the menu says subtitle search isn't set up and that an
  administrator sets it up on the web, in Settings → Server. A server too
  old (`404`): it needs a newer Pley. There is no language switch: change
  the profile's subtitle language in Settings. Subtitles added on any
  device are ordinary tracks on every one.

### Audio

The play request names no audio track, so the server picks, in the
profile's language (the choice remembered for the show or movie, else the
profile's defaults), the track this Roku plays best. Its answer's
`audio_index` is what the app counts as playing. Roku doesn't report
`audio_track_switching` and the app doesn't set the Video node's
`audioTrack`, so any audio but the file's default makes the file a stream
with its video copied.

### Tracks picked in Roku's options

Captions turned on or off, another subtitle track, or another audio track
of a file played as it is are saved as on Samsung and LG (`PUT
/api/items/{id}/streams`: the file, its audio, and its subtitles or `null`
for off), so the next episode, or the movie next time, starts with
matching tracks.

- What shows until the stream has played 2 s is Pley's or the Roku's own
  choice, as is what shows when subtitle sync reopens the stream: neither
  is saved.
- A subtitle track is identified by its address (subtitle sync's
  `offset_ms` removed). Captions only on replay or on mute, and captions
  in the video itself, are left alone.
- Roku's audio tracks are matched to the file's audio streams in order,
  only when Roku lists as many as the file has and their languages agree.
  A converted stream has one.
- Text subtitles picked while the server draws picture ones onto the video
  start a stream without them, where it was.
- Subtitles changed alone are saved without the audio (`audio_index` left
  out), so turning captions on or off doesn't pin the server's choice for
  this Roku on every other device.

### Audio and subtitle defaults

Settings, as on Samsung and LG: Audio language (the file's default, or one
of 34 languages), Subtitles (Off, Only for foreign dialogue, When the audio
is in another language, Always, each with its hint), and Subtitle language
(the audio's, or one of the 34) for the last two. OK opens a row's choices
in a panel at the right; a choice applies at once and is saved with all
three (`PUT /api/me/track-defaults`), the server's answer becoming the
profile. A server or sign-in that doesn't let apps change them (`403`,
`404`, `405`) puts the choice back and says to change them on the web, at
the server's `/account`.

### Quality

The menu, Original and Automatic follow [QUALITY.md](QUALITY.md#the-menu);
the code is `components/lib/AutoQuality.brs`. Roku specifics:

- **Settings → Quality** lists the menu with hints for Original and
  Automatic, kept on the Roku (registry section `playback`), Automatic
  unless another was chosen. A quality is asked for as on Android
  (`mode: "transcode"`, `max_bitrate`, `max_height`) when the file is
  offered it; otherwise the file plays as it is.
- **The player's Quality** lists the qualities the file is offered
  ([QUALITY.md](QUALITY.md#the-menu)). One chosen there becomes the Roku's
  setting, and the title reopens at it where it was.
- **Several sizes.** In a session the server sends in several sizes
  (`PlaybackInfo.adaptive`, see
  [QUALITY.md](QUALITY.md#several-sizes-adaptive-bitrate)), the Video node
  moves between them itself and Automatic leaves the waits to it.
  The original, its remuxes and a conversion too small for smaller sizes
  come in one size, so the app does the stepping down.
- **Waits:** the Video node's state `buffering` after at least 2 s of
  playing (not at a start, after a seek Pley made or a pause), and not
  when `bufferingStatus.isUnderrun` says it isn't for want of data.
- **Not seen:** Roku tells an app nothing of how much it has read ahead,
  so there is no delivery warning, nor dropped frames. What the Roku
  can't decode is converted: a media error plays the file again at the
  server's own quality.

The Video node replaces its content to play another stream and the screen
is black meanwhile, so Roku follows the rules for
[players that go black between streams](QUALITY.md#players-that-go-black-between-streams):

- **A conversion whose bitrate changes as it is sent**
  (`PlaybackInfo.live_bitrate`): a play issue lowers its bitrate only, to
  what the network carried times 0.75 in whole Mbps. What the network
  carried is the HLS segments' sizes over their download times in the last
  20 s (from `downloadedSegment`), no more than the bitrate playing, or
  half that bitrate without segments. The player
  asks the server (`PUT /api/playback/{session}/bitrate`) and plays on
  from the same playlist and size. "Quality lowered to 4 Mbps 1080p" shows
  at the top for 4 s at once; the menu says "Automatic · 4 Mbps 1080p" and
  the details "Lowered for your connection". A refusal (an older server or
  transcoder, a session gone) leaves the stream as it was, treated as one
  whose bitrate can't change.
- **The original (direct play or remux), or a conversion whose bitrate
  can't change**, moves only after a wait of 2 s or more. The ceiling is
  what the network carried times 0.75 (half the bitrate for a file played
  as it is, which has no segments), never above
  the server's own quality for the file (`auto_quality`), always at least
  one quality lower among those the file is offered, each of less bitrate
  than the one above (a 4K file goes 30 Mbps 4K, 15 Mbps 4K, then 12 Mbps
  1080p). Two short waits restart the calm before a step up but don't
  move it. The stream reopens where it was as a new session at that
  quality (`mode: "transcode"`), and "Quality lowered to 8 Mbps 1080p"
  shows at the top for 4 s once it plays.
- **Memory on the Roku:** each step's ceiling is kept 30 minutes per
  server in the registry; a title started in Automatic meanwhile (Up
  Next's too) starts at the quality under it when that makes the file
  smaller.
- **Back up**, silently, by the menu's bitrates (2, 4, 8, 12, 15, 20,
  30 Mbps) then the stream's own, measured on the segments since the last
  step (20 s of video at least). A stream that can't go higher without
  another raises where the next title starts instead (at the
  original it is forgotten). Without segments to measure, it stays
  where it went down to.

**Where Automatic starts.** The server's network memory
([API.md](API.md#network-memory-and-speed-test), `GET
/api/playback/network`) is asked at a title's first opening in Automatic
(the API task's `network` op, 1.5 s at most). Its ceiling replaces the
registry's; without an answer (out of reach, or an older server's `404`)
the registry's counts.

- **Speed test**, where the server asks for one (a network away from it
  that it knows nothing of): the API task fetches 8 MiB of `GET
  /api/playback/speedtest` to a file in `tmp:` (`speedTest`).
  roUrlTransfer gives nothing of a body before its end, so the whole
  request is timed and reported with `probe_complete: false`; the answer
  is the ceiling (70 % of the speed when the server can't keep it). The
  test gives up after about 3 s less the asking (1.5 s at least); what
  arrived by then counts, and under 256 KiB measures nothing, so the
  original starts. The loading circle shows meanwhile; the render thread
  waits for none of it.
- The task keeps the server's answer 10 minutes per server, a `404` too,
  forgotten with each sign-in and profile, so a play on the home network
  doesn't wait for it.
- Each ceiling the registry keeps is told to the server too (`tellNetwork`,
  nothing waits for it): a step down's, a step up's, and none back at
  Original or with Automatic chosen again in Settings or the player's
  menu. Original and the chosen qualities ask nothing.

### Skip markers and Up Next

Skip Intro and Skip Credits show from a marker's start until a second
before its end, for the markers of the file playing. An episode's credits
bring up Up Next instead; its countdown runs only while Settings → Play the
next episode by itself is on (off, the next waits for Play now, and the
episode's end leaves the player). The button is a child of the Video node
and has the focus while shown: OK skips; play pauses or resumes; fast
forward and rewind move 10 s; Back leaves the player. While Up Next counts
down, Back cancels it, and play, fast forward and rewind cancel it and do
their usual action.

## How the app works

| Path | What |
|---|---|
| `source/main.brs` | Starts the one scene |
| `components/MainScene.*` | The stack of screens, the HTTP agent for pictures, sign-in when the server ends it |
| `components/MusicPlayer.*` | The music player: a queue of tracks on the scene's Audio node, as `m.global.music` |
| `components/screens/` | Servers, sign-in with a code, "Who's watching?", Home, Library, Show, Details (a title's page), Grid (a person's, a collection's or an artist's titles, the playlists), Playlist, Album, Now Playing, Player, Search, Settings, Watch history, "Can't reach your server" |
| `components/shared/` | The screens' helpers (`Ui.brs`), and what can be done with a title (`Titles.brs`) |
| `components/items/` | The tiles and rows of lists and grids, the player's skip button |
| `components/tasks/ApiTask` | The one place that talks to the server |
| `components/tasks/DiscoveryTask` | `pley?` to UDP port 7539 (`apps/server/src/discovery.rs`) |
| `components/tasks/EventsTask` | The server's event stream, on a socket (http:// servers) |
| `components/lib/` | Plain BrightScript, no SceneGraph: addresses, sign-in decisions, token storage, the capability report, the server's JSON as tiles, times, the player's decisions |
| `tests/` | Unit tests of `components/lib`, run on a computer |
| `scripts/` | Test runner, smoke run, preview, bench, the images' generator |
| `licenses/` | Notices of what the package carries from others |

### Requests and sign-in

- **Every HTTP call goes through `ApiTask`**, a Task with all its
  transfers on one message port; the render thread never makes one. Lists
  (Home's rows, a library's pages, search, seasons, episodes) are built
  into ContentNodes there too and reach the render thread whole.
- **The access token** goes in `Authorization: Bearer` only, never in an
  address: on requests, and on the HTTP agents of pictures (the scene's)
  and of the Video node (playlists, segments, subtitles), which get a new
  agent each time the token changes.
- **Refreshes** run one at a time per server, and the new pair is written
  to the registry before it is used: a refresh token works once, and two
  refreshes with the same one end the sign-in. A refresh is waited for up
  to two minutes (not the usual 30 s), and the pair it brings is kept only
  while the registry still holds the one it replaces. If the person signed
  out, forgot the server or signed in again meanwhile, it is revoked
  instead. A token refused moments after a refresh gave it ends the
  sign-in, as in the Android app; so does one that expires with nothing to
  refresh it. A revocation the server is too busy for (`503`) is sent
  again once.
- **User-Agent:** every request says `pleyRoku/<version> (Roku OS
  <version>; <model>)`, the model being `roDeviceInfo`'s display name. The
  server's `/link` page shows it to whoever types the code.
- **QR code:** the sign-in screen shows the server's QR code (`GET
  /api/oauth/device/qr`, fetched without the token since anyone may fetch
  it) once it has loaded. An older server answers `404`; the code and
  address are enough.
- **"Who's watching?"**, as the other TVs ask it: at start on a server
  whose household had more than one profile when last asked (Home asks
  `GET /api/oauth/profiles` once; the registry's `household` section keeps
  the count), and from Settings → Switch profile. OK on a profile switches
  with the switch-profile grant ([API.md](API.md)), after its PIN (Roku's
  PIN pad) if it has one. The profile signed in goes on, its PIN asked
  first at a start, unless a sign-in with a code chose it in the browser.
  A profile that needs its password is signed in with a code instead,
  chosen in the browser, since apps never see passwords. The switch spends
  the refresh token, so it never runs beside a refresh: one asked for
  during a refresh waits for it, and requests meanwhile wait for the new
  pair. After a switch the app restarts at Home as that profile. A sign-in
  made before the app asked for `profiles` says to sign in again.
- **Scopes:** the app asks for `library play playlists profiles`, as Kodi
  does; a sign-in made before that answers `403` to a playlist change, and
  the app says to sign in again.
- **Unreachable server:** the screen that needed it gives way to "Can't
  reach …" with Retry and Switch server.

### Keeping screens current

- **Event stream** (`GET /api/events`), on an http:// server.
  roUrlTransfer gives an answer only once it is whole, so
  `components/tasks/EventsTask` reads it from a socket of its own, parsing
  HTTP/1.1 and chunks by hand (`components/lib/Events.brs`). It reconnects
  with each new token and, after the stream ends or fails, 2 s later,
  doubling to a minute. Watch state, My List and library changes,
  gathered for 0.3 s, have Home ask for its rows again (at once if it
  shows, else when it shows). A stream of this Roku's stopped from
  elsewhere ends at once, saying whether an administrator stopped it.
  Roku's sockets have no TLS, so an https server's stream isn't followed.
- **Home's rows** are asked for again only after something played, when
  they are a minute old (for changes the event stream can't report), after
  an event, or when Home is picked in its menu. The task rebuilds only the
  rows whose JSON changed; the others stay as they are and where they were
  scrolled, focus stays on the same row and title, and the menu changes
  only with the libraries.
- **Local changes:** a title marked watched or unwatched, played, added
  to or removed from My List, or removed from Continue Watching is noted
  on the scene (its latest 30 changes, `uiNoteChange`). The screen where
  it happened follows at once; covered screens follow as they show again
  (Home asks for its rows, grids redraw the title's tile) without asking
  the server for the whole list. My List as Home last saw it decides
  whether the Options key offers Add or Remove.
- **Focus:** a screen shown again takes the focus at once, before any
  answer, so the remote always works. An answer moves the focus only
  while its screen has it, so a dialog over it keeps it.

### Screens

- **A title's page** comes with its cast, crew and extras, built in the
  task (`build: "detail"`). A show's seasons, More Like This and its
  collections are asked for beside it, and each row shows as it comes, in
  that order. Down past the buttons slides the page up to the rows; Up
  from the first row (or Back) slides it down again.
- **A library** opens on the tab it was left on: Recommended (its rows,
  `GET /api/libraries/{id}/hubs`), Library (a grid, a page at a time) or
  Collections (not for music). Left, right and OK move between the chips
  at the top. On the Library tab two chips at the right show its sort
  ("Year, newest first") and filters ("Unwatched, Drama"); OK opens their
  choices in a panel at the right: the web app's sorts for the library's
  kind (OK on the checked one reverses the order), and Show (All titles,
  Unwatched, Watched), Genre and Year from `GET
  /api/libraries/{id}/filters`, and Clear filters. Tab, sort and filters
  are kept per server, profile and library (registry section
  `libraryViews`), as the web app keeps them in the browser; the profile
  is the one `GET /api/auth/me` names, asked when a server is chosen
  signed in and after each sign-in.
- **Sort libraries** (Settings), as on Android TV: OK lifts a library, up
  and down move it, OK drops it, Back puts it back; "Use the server's
  order" returns to the order everyone sees. Each drop saves at once (`PUT
  /api/me/libraries/order`, every library the profile may open, so music
  ones too though Roku shows none), and the server's answer becomes the
  list; Home asks for its menu and rows again when it next shows. An older
  server says sorting needs a newer Pley, and nothing moves.
- **Music:** music libraries list their artists (square tiles), an artist
  its albums, an album its tracks. Play, Shuffle or OK on a track hand the
  tracks to the music player (`components/MusicPlayer`, on the scene, as
  `m.global.music`) and open Now Playing: the cover, the track, its
  progress, and Previous, Pause, Next and Stop (the remote's play, fast
  forward and rewind too). Each track gets its own session (`POST
  /api/playback` with the Roku's capabilities), plays on the Audio node
  (the token in its HTTP agent), reports progress as videos do, and its
  session closes when another starts. A track that doesn't play as it is
  plays again converted, once; one that doesn't play at all is skipped,
  with a note on Now Playing. Back leaves Now Playing and the music goes
  on; Home's menu then has Now playing. A video, another profile, sign-in
  or server stops it. A music playlist plays the same way.
- **Playlists** (Home's menu) are the profile's own, shown as tiles of
  their first entry's picture. A playlist's page plays its entries in
  order from the top, shuffled, or from the entry OK is pressed on: the
  player gets them as its queue, and Up Next brings on the next entry, a
  movie as well as an episode. Rename (the keyboard) and Delete (asked
  first) change it; the Options key on an entry removes it (not in a
  smart playlist, which its filter fills).

### Launch beacons

`AppLaunchComplete` once the first screen can be used (Home with its rows,
or Servers or sign-in when the app opens on them), and `AppDialogInitiate`
/ `AppDialogComplete` around a sign-in shown at launch.

## Checking it on a computer

In `apps/roku`, with Bun and Node 22 or newer. Bun installs the packages
and runs the scripts, but the tools they call (bsc, bslint) run on Node:
`apps/roku` has no `bunfig.toml` making Bun their runtime, because
bslint's plugin loader needs Node's `module.require`.

```powershell
bun install
bun run check     # bsc's compile check, bslint, the unit tests
bun run smoke     # the whole app in brs-node, against a fake server
bun run bench     # the list work timed in brs-node
bun run preview   # the app in a browser, the keyboard as its remote
```

**Preview.** `bun run preview` runs the app in brs-node and serves it on
http://127.0.0.1:8086/: the TV's picture as brs-node draws it, with the
arrows, Enter (OK), Esc or Backspace (Back), Space (play/pause), `[` and
`]` (rewind, fast forward) and `*` (options) as the remote, and letters
typed on its keyboards. It runs against the smoke run's fake server, which
approves the TV code by itself; `bun run preview --server
http://<address>:7539` runs it against a real one, whose `/link` page
approves the code. `--port` chooses another port.

**Unit tests** run `components/lib` and `tests/*.test.brs` in brs-node
(lvcabral's BrightScript engine, published as `brs-node`, formerly
`brs-engine`): each `sub test_…()` is a test.

**Smoke run.** `bun run smoke` starts a fake Pley server on 127.0.0.1
(`scripts/fixture-server.mjs`, answers written from [API.md](API.md)) and
walks the app with the remote:

- sign-in with a code and its QR code; Home; a title's details after a
  refused token; Back while the player starts (the session that comes
  after is closed);
- the player with picture subtitles to draw, its menu, subtitle sync
  (shown, moved 0.3 s and remembered), a chapter, the playback details,
  Find subtitles… (the results, the first added and saved as the pick, the
  stream reopened without the drawn pictures);
- Automatic's speed test before the first play (8 MiB fetched, measured
  and reported, the answer kept for the next plays), Home asking again
  after it;
- the Options key's Mark as watched, a library's second page, its
  Recommended and Collections tabs and its sort, a show's page, its
  episodes and a person's page;
- music (an artist, an album, its tracks played and stopped); Search, Home
  not asking again after it; a playlist (an entry removed, then played);
- Settings, the quality, sorting the libraries (Movies after TV Shows,
  then the server's order again, Home asking again), subtitles changed to
  Always (the languages sent as `null`), "Who's watching?" and another
  profile;
- the server gone and Retry, then signing out and in again on a server
  without QR codes; then a restart of the app, signed in, in a second
  process (brs-node runs one app per process).

It fails on any BrightScript error, a request without the User-Agent, a
token in an address or sent for a QR code, a refresh token used twice,
two refreshes at once or a profile switch beside one. It saves each screen
in `out/smoke/`.

brs-node is a simulator: it draws with its own fonts, plays no video (so
Skip Intro and Up Next have only their unit tests), fetches every address
with a Node process of its own, one at a time, numbers transfers at random
below 80 (the API task keeps those in progress apart), drops a remote key
now and then, and can't stand in for a Roku's speed.

**Images.** `bun run images` redraws the watched mark and the buttons'
focus (`scripts/make_images.py`, Python's standard library only):
nine-patch pictures, so the buttons are focused in the app's accent, as
its lists are, not in Roku's grey. The home screen icons (540 × 405 and
290 × 218, the P and the wordmark on Pley's tile, as the Android TV app's
banner) and the splash screens (the logo on the app's background) are
rendered from Pley's logo with the other apps' icons:
`bun scripts/render-icons.mjs` at the repository's root.

## Not yet

- Playback speed and an audio delay, which the Video node doesn't offer; a choice of language in Find subtitles….
- Following other devices' changes at once from an https server: Home
  asks again once its rows are a minute old.
- Audio other than the file's default played as it is: reporting
  `audio_track_switching` and starting the Video node on the server's
  `audio_index`, once tried on a Roku.
- The Show screen asks for its season's episodes and its next episode
  again each time it shows; Home's row fingerprints would spare that when
  nothing changed.
- Seek previews in Roku's own bar, which takes BIF files; rating
  titles; reordering a playlist's entries; "Always start as"; an editable music queue.
- Roku's channel store: a developer account, certification, SceneGraph
  1.3 and the launch beacons' order.
