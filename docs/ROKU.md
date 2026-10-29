# Pley for Roku

A native SceneGraph app for Roku players and Roku TVs. It finds Pley
servers on the local network (or takes an address), signs in with a code
shown on the TV and approved on the server's `/link` page (or a QR code of
it, which the server draws), asks "Who's watching?" as it starts when the
household has more than one profile, and shows:

- Home's rows, and each library with the web app's tabs (Recommended,
  Library sorted and filtered, Collections);
- a title's page: Resume and Play from start, a show's next episode and
  its episodes, Mark as watched, My List, Go to show, and rows of its
  seasons, cast and crew, extras, More Like This and the collections it
  is part of; people's and collections' pages;
- music: artists, albums and their tracks, played by a music player that
  goes on while the app is used, with Now Playing;
- playlists: Play, Shuffle, Rename, Delete, an entry taken out;
- Search with the on-screen keyboard (titles and people);
- Settings: the profile's audio and subtitle defaults, Quality, Play the
  next episode by itself, Watch history (a play taken out with the
  Options key), Switch profile, Sort libraries, Switch server and Sign
  out.

The Options key (*) on a title offers Mark as watched or unwatched, Add
to or Remove from My List, Add to playlist (a new one named on the
keyboard too), Remove from Continue Watching and Go to show. The player
plays files as they are whenever the Roku can (it reports what it
decodes, from `roDeviceInfo`), HLS otherwise, reports progress, offers
Skip Intro and Skip Credits as the web player does, the next episode at
the credits of one (a playlist's next entry), and Pley's menu (Down):
Quality, subtitle sync, the file's chapters, Find subtitles… and the
playback's details.

It talks to the Pley server and nothing else, so it works on a home network
without internet: pictures come from the server, the fonts are the Roku's
own, and the client ID (`https://hhaidar.github.io/pley/oauth/roku.json`) is
only a name the server knows; nobody fetches it.

The app is in `apps/roku`; paths below are relative to it.

## How it's built

| Path | What |
|---|---|
| `source/main.brs` | Starts the one scene |
| `components/MainScene` | The stack of screens, the HTTP agent of pictures, sign-in when the server ends it |
| `components/MusicPlayer` | The music player: a queue of tracks on the scene's Audio node, as `m.global.music` |
| `components/screens/` | Servers, sign-in with a code, "Who's watching?", Home, Library, Show, Details (a title's page), Grid (a person's, a collection's or an artist's titles, the playlists), Playlist, Album, Now Playing, Player, Search, Settings, Watch history, "Can't reach your server" |
| `components/shared/` | The screens' helpers (`Ui.brs`), and what can be done with a title (`Titles.brs`) |
| `components/items/` | The tiles and rows of lists and grids, the player's skip button |
| `components/tasks/ApiTask` | The one place that talks to the server |
| `components/tasks/DiscoveryTask` | `pley?` to UDP port 7539 (apps/server/src/discovery.rs) |
| `components/tasks/EventsTask` | The server's event stream, on a socket (http:// servers) |
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
  Pley picks one, it turns captions on for that playback and back as they
  were after, unless the person changed them meanwhile.
- Pley's menu: Down while a title plays opens it at the right, over the
  video, which plays on (play, fast forward and rewind still work): Quality
  ("Automatic · Original"), Subtitle sync for a file with text subtitles,
  Chapters for a file that has them (OK goes to one; the one playing is
  checked), Find subtitles… (below), and Playback details, as the other
  apps say them: how it
  plays ("Direct play", "Direct stream", "Transcoding") and why, its video
  and audio as they are and as the server converts them (`GET
  /api/playback/{session}`), its bitrate, what the screen gets of HDR,
  subtitles burned in, and how far ahead the conversion is.
  Back goes back in it, then closes it. Subtitles and audio stay in
  Roku's own options (the Options key), which know the Video node's
  tracks.
- Subtitle sync, as the Android TV player's Sync: chosen in Pley's menu
  for a file with text subtitles, it shows at the top of the screen; left and
  right (rewind and fast forward too) move the subtitles 0.1 s, from 10 s
  earlier to 10 s later, shown as "+0.3 s"; Reset puts them back, Done or
  Back closes it. The Video node draws a sideloaded track as it comes and
  has no delay, so the server moves the cues (`offset_ms` on every text
  track's address, docs/API.md) and the stream opens again where it was,
  the same session, the subtitles that showed shown again. The offset is
  remembered for each file of each server in the registry (section
  `subtitleSync`, the latest 50 files) and moves the file's subtitles from
  the start the next time.
- Find subtitles… (Pley's menu; `components/lib/SubtitleSearch.brs`):
  the server looks for the file playing on OpenSubtitles in the person's
  subtitle language (`GET /api/items/{id}/subtitles/search?file_id=`, no
  `language`, so the server takes the profile's, English without one),
  and the results are listed in the menu in its order, those made for
  this very file first: "English · Movie.2019.1080p.BluRay", the whole
  name, "Made for this file", "Hearing impaired", "Forced" and the
  downloads under the list for the one in focus. OK adds it on the server
  (`POST /api/items/{id}/subtitles` with the result and the file), whose
  answer is an ordinary text track: it joins the session's tracks, is
  saved as the person's pick for the show or the movie (`PUT
  /api/items/{id}/streams`, as a pick in Roku's options is), and shows,
  captions turned on for it, the stream opening again where it was as
  subtitle sync opens it (a new session where the server drew picture
  subtitles onto the video). "Subtitles added: English" shows at the top.
  A server without OpenSubtitles set up (503, `setup: false`) has the
  menu say that subtitle search isn't set up, and that an administrator
  sets it up on the web, in Settings → Server; a server from before it
  (404) that it needs a newer pley. There is no switch to another
  language: the profile's subtitle language is changed in Settings.
  Subtitles added on any device are ordinary tracks on every one.
- Subtitles or audio picked in Roku's options while a title plays (captions
  turned on or off, another subtitle track, another audio track of a file
  played as it is) are saved as on Samsung and LG (`PUT
  /api/items/{id}/streams`, the file, its audio and its subtitles or `null`
  for off), so the next episode, or the movie next time, starts with
  matching tracks. What shows until the stream has played two seconds is
  Pley's choice or the Roku's own, and so is what shows as subtitle sync
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
- Quality (Settings, docs/QUALITY.md; `components/lib/AutoQuality.brs`):
  the menu in its order, Original, Automatic, 30 Mbps 4K, 15 Mbps 4K,
  20 Mbps 1080p and down to 2 Mbps 480p, Original and Automatic with
  their hint (the others need none), kept on the
  Roku (registry section `playback`), Automatic unless another was
  chosen. A quality is asked for as on Android: converted (`mode:
  "transcode"`, `max_bitrate`, `max_height`) when the file is offered it,
  having more lines or more than 1.25 times its bitrate (its details
  say); else the file plays as it is. The player's menu has Quality too:
  the qualities the file is offered, named by its own lines when taller
  ("8 Mbps 720p"), the tallest first, then by bitrate; one chosen there
  becomes the Roku's setting, and the title opens again at it where it
  was.
- Automatic plays the original and watches it. A transcode comes in
  several sizes (`hls.variants`, docs/QUALITY.md) the Video node moves
  between by itself, so in such a session (`PlaybackInfo.adaptive`)
  Automatic leaves its waits to it; the original, its remuxes and
  a conversion too small for smaller sizes come in one, so stepping down
  from them is the app's:
  - *Waits*: the Video node's state `buffering` after at least 2 s of
    playing (not a start, nor after a seek Pley made or a pause), and not
    when `bufferingStatus.isUnderrun` says it isn't for want of data. One
    of 2 s or more, or two shorter ones within a minute, is a play issue.
  - *Delivery* before a wait: Roku tells an app nothing of how much it has
    read ahead, so this one isn't seen; nor are dropped frames (the watch's
    rule for them, in the title's first minute another stream and later the
    next title a quality lower, waits for a Roku that says). What the Roku
    can't decode is converted, as before (a media error plays the file again
    converted at the server's own quality).

  The Video node replaces its content to play another stream, and the
  screen is black meanwhile (docs/QUALITY.md, "Players that go black
  between streams"), so Automatic never moves to another stream while one
  plays well:
  - *A conversion whose bitrate changes as it is sent*
    (`PlaybackInfo.live_bitrate`): a play issue lowers its bitrate and
    nothing else, to what the network carried (the HLS segments' sizes over
    their download times in the last 20 s, from `downloadedSegment`, no more
    than the bitrate playing; half of it without segments) times 0.75,
    rounded down to whole Mbps as on Samsung and LG, never under its size's
    floor (8 Mbps for 4K, 3 Mbps for 1080p, 1.5 for 720p and 1 below) nor
    within 20 s of the last change. The player asks the server (`PUT
    /api/playback/{session}/bitrate`) and plays on: the same playlist and
    size, the segments the Video node hasn't fetched yet at the new bitrate.
    "Quality lowered to 4 Mbps 1080p" shows at the top for 4 s at once; the
    menu says "Automatic · 4 Mbps 1080p" and the details "Lowered for your
    connection". At the floor the network waits. A refusal (a server or
    transcoder from before, a session gone) leaves the stream as it was, and
    it moves as one whose bitrate can't change.
  - *The original* (direct play or remux), *or a conversion whose bitrate
    can't change*: another stream only once the picture has stopped
    anyway. A wait of 2 s or more steps down by the spec's rule: what the
    network carried (as above; half the bitrate for a file played as it
    is, which has no segments) times 0.75, never above the server's own
    quality for the file (`auto_quality`), always a quality lower among
    those the file is offered, each of less bitrate than the one above (a
    4K file goes 30 Mbps 4K, 15 Mbps 4K, then 12 Mbps 1080p), never under
    2 Mbps 480p, and not within 20 s of the last change. Two short waits
    are an issue all the same (the calm before a step up starts again)
    but don't move it. The stream opens again where it was, as a new
    session at that quality (`mode: "transcode"`), and "Quality lowered
    to 8 Mbps 1080p" shows at the top for 4 s once it plays.

  The ceiling of each step is kept 30 minutes for each server in the
  registry; a title started in Automatic meanwhile (Up Next's too) starts at
  the quality under it when that makes the file smaller. Back up: after 3
  minutes without an issue and since the last step, and 5 after a step down,
  when the segments since then (20 s of video at least) carried 1.6 times
  the step up, a bitrate lowered as it is sent climbs one step, silently:
  the menu's next bitrate above it (2, 4, 8, 12, 15, 20, 30 Mbps), then the
  stream's own, never more. A stream that can't go higher without another
  (at its own bitrate, one whose bitrate can't change, or one in several
  sizes) stays: where the next title starts goes up a quality instead, by
  the same rule (the network's memory, in the registry and on the server, a
  quality's bitrate; at the original it is forgotten). Without segments to
  measure, it stays where it went down to, as the spec allows. It settles: a
  step up, or a raise of where the next title starts, that a step down
  undoes within 10 minutes isn't tried again during the title, and each one
  doubles the calm a step up waits for (3, then 6, then 12 minutes).
- Where Automatic starts: the server remembers what this Roku's network
  carries too (`GET /api/playback/network`, docs/API.md), a day after a
  step down and a week after a speed test. A title's first opening in
  Automatic asks it (the API task's `network` op, 1.5 s at most), and its
  ceiling takes the place of the one kept in the registry; without an
  answer (out of reach, or a server from before it, 404) the registry's
  counts, as before. Where the server asks for a speed test (a network
  away from it that it knows nothing of), the API task fetches 8 MiB of
  `GET /api/playback/speedtest` to a file in `tmp:` (`speedTest`):
  roUrlTransfer gives nothing of a body before its end, so the whole
  request is timed and told to the server with `probe_complete: false`,
  and its answer is the ceiling (70 % of the speed when the server can't
  keep it). The test is given up after about 3 s less the asking (1.5 s
  at least); what came by then counts, where the Roku wrote it, and under
  256 KiB measures nothing and the original starts. The loading circle
  shows meanwhile; the render thread waits for none of it. The task keeps
  the server's answer 10 minutes for each server, a 404 too, forgotten
  with each sign-in and profile, so a play on the home network doesn't
  wait for it. Each ceiling the registry keeps is told to the server as
  well (`tellNetwork`, nothing waits for it): a step down's, a step up's,
  none back at Original or with Automatic chosen again in Settings or the
  player's menu. Original and the chosen qualities ask nothing.
- Playback speed, audio delay and the subtitles' look, which other apps'
  players offer: the Video node has no field for a playback speed on current
  Roku OS (its reference lists `control` and `seek`, nothing of a rate) nor
  for an audio delay, so neither is offered. Subtitles look as the Roku's
  own caption settings say (Settings → Accessibility → Captions style: size,
  font, colours, edges, background, opacity): the app never sets the Video
  node's `captionStyle`, which would put its own over the person's choices
  for the whole Roku.
- Sort libraries (Settings), as on Android TV: OK lifts a library, up and
  down move it, OK puts it there, Back puts it back; "Use the server's
  order" goes back to the order everyone sees. Each drop saves the order
  at once (`PUT /api/me/libraries/order`, every library the profile may
  open, so music ones are listed too though Roku shows none), and the
  server's answer becomes the list; Home asks for its menu and rows again
  when it shows next. A server from before says sorting needs a newer
  Pley, and nothing moves.
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
  an episode's credits bring up Up Next instead (its countdown only while
  Settings → Play the next episode by itself is on: off, the next waits
  for Play now, and the end of the episode leaves the player). The button is a child of
  the Video node and has the focus while shown: OK skips; play pauses or
  goes on, fast forward and rewind move 10 s; Back leaves the player as
  ever. While Up Next counts down, Back cancels it, and play, fast forward
  and rewind cancel it and do the same.
- What the person's other devices change comes from the server's event
  stream (`GET /api/events`), on an http:// server: roUrlTransfer gives
  an answer only once it is whole, so `components/tasks/EventsTask` reads
  it from a socket of its own, HTTP/1.1 and chunks by hand
  (`lib/Events.brs`), connecting again with each new token and, after
  the stream ends or fails, 2 s later and doubling to a minute. Watch
  state, My List and library changes, gathered for 0.3 s, have Home ask
  for its rows again (at once if it shows, else as it shows); a stream
  of this Roku's stopped from elsewhere ends at once, saying whether an
  administrator stopped it. Roku's sockets have no TLS, so an https
  server's stream isn't followed.
- Home asks for its rows again only after something played, when they are
  a minute old (for what other devices played or marked meanwhile, when
  the event stream can't say), after an event, or when Home is picked in
  its menu. The task makes again only the
  rows whose JSON changed; the others stay as they are, scrolled where
  they were, the focus stays on the same row and title, and the menu
  changes only with the libraries.
- A title marked watched or unwatched, played, put in My List or taken
  out, or taken out of Continue Watching is noted on the scene (its latest
  30 changes, `uiNoteChange`): the screen where it happened follows at
  once, and the screens it covers as they show again, Home asking for its
  rows and grids drawing the title's tile again, without asking the server
  for the whole list. My List as Home last saw it says which of Add and
  Remove the Options key offers.
- A title's page comes with its cast, crew and extras, built in the task
  (`build: "detail"`); a show's seasons, More Like This and its collections
  are asked for beside it, and each row shows as it comes, in that order.
  Down past the buttons slides the page up to the rows, Up from the first
  row (or Back) slides it down again.
- "Who's watching?", as the other TVs ask it: as the app starts on a
  server whose household had more than one profile when last asked (Home
  asks `GET /api/oauth/profiles` once, the registry's `household`
  section keeps the count), and from Settings → Switch profile. OK on a
  profile switches to it with the switch-profile grant (docs/API.md),
  after its PIN (Roku's PIN pad) for one that has one; the profile signed
  in goes on, its PIN asked first at a start, unless a sign-in with a
  code chose it in the browser. A profile that needs its password is
  signed in to with a code instead, chosen in the browser, as apps never
  see passwords. The switch spends the refresh token, so it never runs
  beside a refresh: one asked for while a refresh is under way waits for
  it, and requests meanwhile wait for the new pair. Switched, the app
  starts again at Home, everything as that profile sees it. A sign-in
  from before the app asked for `profiles` says to sign in again.
- Music libraries list their artists (square tiles), an artist their
  albums, an album its tracks. Play, Shuffle or OK on a track hand the
  tracks to the music player (`components/MusicPlayer`, on the scene, as
  `m.global.music`) and open Now Playing: the cover, the track, how far
  along it is, and Previous, Pause, Next and Stop (the remote's play,
  fast forward and rewind too). Each track gets a session of its own
  (`POST /api/playback` with the Roku's capabilities), plays with the
  Audio node (the token in its HTTP agent), reports progress as videos
  do, and its session is closed when another starts; a track that
  doesn't play as it is plays again converted, once, and one that
  doesn't play at all is passed over, said on Now Playing. Back leaves
  Now Playing and the music goes on; Home's menu has Now playing then. A
  video, another profile, sign-in or server stops it. A music playlist
  plays the same way.
- Playlists (Home's menu) are the profile's own, as tiles of their first
  entry's picture. A playlist's page plays its entries one after the
  other from the top, shuffled, or from the entry OK is pressed on: the
  player gets them as its queue, and Up Next brings on the next entry, a
  movie as well as an episode. Rename (the keyboard) and Delete (asked
  first) change it; the Options key on an entry takes it out (not of a
  smart playlist, which its filter fills). The app asks to be allowed
  `library play playlists profiles`, as Kodi; a sign-in from before that
  answers 403 to a change, and the app says to sign in again.
- A library opens on the tab it was left on: Recommended (its rows, `GET
  /api/libraries/{id}/hubs`), Library (a grid a page at a time) or
  Collections (not for music), chips at the top that left, right and OK
  move between. On the Library tab two chips at the right say its sort
  ("Year, newest first") and filters ("Unwatched, Drama"); OK opens their
  choices in a panel at the right: the sorts the web app offers for the
  library's kind (OK on the one checked turns the order around), and
  Show (All titles, Unwatched, Watched), Genre and Year from `GET
  /api/libraries/{id}/filters`, and Clear filters. The tab, sort and
  filters are kept for each server, profile and library (registry
  section `libraryViews`), as the web app keeps them in the browser; the
  profile is the one `GET /api/auth/me` names, which the task asks for
  as a server is chosen signed in and after each sign-in.
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
`bun run smoke` starts a fake Pley server on 127.0.0.1
(`scripts/fixture-server.mjs`, answers written from docs/API.md) and walks
the app with the remote: sign-in with a code and its QR code, Home, a
title's details after a refused token, Back while the player starts (the
session that comes after is closed), the player with picture subtitles to
draw, its menu, subtitle sync (shown, moved 0.3 s and remembered), a
chapter and the playback's details, Find subtitles… (the results, the
first added and saved as the pick, the stream opened again without the
pictures drawn), Automatic's speed test before the first play (8 MiB
fetched, measured and told, the answer kept for the next plays), Home asking again after it, the
Options key's Mark as watched, a library's second page, a library's
Recommended and Collections tabs and its sort, a show's page, its
episodes and a person's page, music (an artist, an album, its tracks
played and stopped), Search, Home not asking again after it, a playlist
(an entry taken out, then played), Settings, the quality, sorting the
libraries (Movies put after TV Shows, then the server's order again, Home
asking again), subtitles changed to Always (the languages sent as
`null`), "Who's watching?" and another profile, the server gone and
Retry, then signing out and in again on a server without QR codes; then
it starts the app again, signed in, in a second process (brs-node runs
one app per process). It fails on any BrightScript error, on a request
without the User-Agent, a token in an address or sent for a QR code, a
refresh token used twice, two refreshes at once or a profile switch
beside one, and saves each screen in `out/smoke/`.
brs-node is a simulator: it draws with its own fonts, plays no video (so
Skip Intro and Up Next have only their unit tests), fetches every address
with a Node process of its own, one at a time, numbers transfers at random
below 80 (the API task keeps those under way apart), drops a remote key now
and then, and can't stand in for a Roku's speed.

`bun run images` draws the watched mark and the buttons' focus again
(`scripts/make_images.py`, Python's standard library only): nine-patch
pictures, so the app's buttons are focused in its accent, as its lists
are, not in Roku's own grey. The home screen icons (540 × 405 and
290 × 218, the P and the wordmark on Pley's tile, as the Android TV app's
banner) and the splash screens (the logo on the app's background) are
rendered from Pley's logo with the other apps' icons:
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
   password, upload the zip and press Install. Pley appears on the Home
   screen as a developer app; installing again replaces it.

`bunx bsc --deploy --host <address> --password <password>` packages and
installs in one go. The Roku's debug console (`telnet <address> 8085`) shows
what the app prints and its errors.

The manifest asks for SceneGraph 1.2, which every Roku OS since 9 runs.
Roku's store certification asks for 1.3 since October 2026, which needs Roku
OS 15; the app uses nothing of 1.3, so the switch is one line when it goes
to the store.

## Not yet

- Playback speed and an audio delay, which the Video node doesn't offer; a choice of language in Find subtitles….

- Hearing what other devices change at once from an https server: Home
  asks again once its rows are a minute old.
- Audio other than the file's default played as it is: reporting
  `audio_track_switching` and starting the Video node on the server's
  `audio_index`, once tried on a Roku.
- The Show screen asks for its season's episodes and its next episode
  again each time it shows; Home's row fingerprints would spare that when
  nothing changed.
- Seek previews in Roku's own bar, which takes BIF files; rating
  titles; putting a playlist's entries in another order;
  "Always start as"; a music queue to change.
- Roku's channel store: a developer account, certification, SceneGraph 1.3
  and the launch beacons' order.
