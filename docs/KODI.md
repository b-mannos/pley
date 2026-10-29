# Pley for Kodi

A Kodi add-on, `plugin.video.pley`, for Kodi 20 (Nexus) and newer on any
platform Kodi runs on: Windows, macOS, Linux, LibreELEC and CoreELEC,
Android TV boxes. It shows a Pley server in Pley's own screens, drawn as
the Android TV app draws them: the rail, Home's hero and rows, a title's
page, every library, collections, playlists and search. It plays with
Kodi's own player: files as they are whenever Kodi can, which is nearly
always, and the server's conversion otherwise.

It is free software under the GPL (version 2 or later, as Kodi), since it
runs inside Kodi; it talks to the server only over HTTP, like the other
apps (LICENSING.md). Its pictures are drawn from Lucide's icons, whose
notice is in `NOTICES.txt` beside `LICENSE.txt`.

The add-on is in `apps/kodi/plugin.video.pley`, its tests in
`apps/kodi/tests`; file names below are the add-on's own.

## Installing

No add-on repository carries it yet. Pack it and install the zip:

```sh
python apps/kodi/package.py        # dist/plugin.video.pley-<version>.zip
```

Its icon (512 square, solid to the corners, as Kodi's add-on checker
wants) and fanart (1920 × 1080) are rendered from Pley's logo with the
other apps' icons: `bun scripts/render-icons.mjs` at the repository's
root.

In Kodi: Settings → System → Add-ons → Unknown sources on, then
Settings → Add-ons → Install from zip file, and pick the zip (copy it to
the device first, or reach it over the network).

## Signing in

Open the add-on and choose a server: Pley servers on the local network
answer a broadcast (docs/API.md; `pley?` on UDP port 7539), and any other
is typed in (`192.168.1.5`, `nas.local:7539`, `https://pley.example.com`).
Kodi then shows a code, typed on the server's `/link` page from a phone
or computer where someone is signed in, and the same as a QR code the
server draws, on the Android TV app's link screen (`signin.py`,
`resources/skins/default/1080i/pley-signin.xml`), whose Change server
goes back to the servers. The link page names
this Kodi by what its User-Agent says (`pleyKodi/0.1.0 (Kodi 21.3;
Windows)`), so two of them can be told apart. The
add-on is one of Pley's own apps (`https://hhaidar.github.io/pley/oauth/kodi.json`):
it signs in only with a code and gets a viewer's rights, whoever allows it:
`library play playlists profiles` (browse, play, make and change the
profile's playlists, and switch to the household's other profiles). A Kodi
signed in before it asked for `playlists` is told by the server to sign in
again when it adds to a playlist; one signed in before `profiles`, choosing
Switch profile, is asked to sign in again.
Settings has Switch profile, Sign out, which asks "Sign out?" first, and
Choose a server.

**Who's watching?** A Kodi signed in to a household of more than one
profile asks it once each time Kodi starts, as the TV apps do (not right
after signing in: whoever approved the code chose the profile): the
household's faces in a row, without the rail, the one in use in focus and
a lock on those with a PIN. Another one is switched to at once; one with a
PIN asks it in Kodi's number dialog (hidden, never kept), and going on as a
profile with a PIN at the start asks its PIN too. A wrong PIN, too many of
them, a locked PIN or a profile the app can't use says so under the faces;
nothing is tried again by itself. A profile that needs its password is
signed in to again with a code, chosen on the phone that approves it. Back
leaves Pley. The profile's menu (the rail's avatar) has Switch profile
too, and the add-on's settings for Kodi's lists. The service switches
(the switch-profile grant, docs/API.md) under the same lock as its
refreshes, keeps which profile it acts as in the token file, and opens the
event stream again as that profile.
Choosing another server keeps the sign-in there until the new one is made,
then ends it on its server.
The same server at another address is signed in to again: anything on the
network can answer the broadcast with a server's id, and the tokens would
go to it.

## Pley's look

Opened from Kodi's add-ons (or a favourite), Pley shows its own screens
over Kodi, as its Android TV app does (apps/android/ui/tv, its screenshots
side by side with Kodi's were the reference):

- **The rail** on the left: the profile, Search, Home, the first three
  libraries, Playlists, Collections and Settings (the add-on's). With more
  than three libraries, Libraries takes Collections' place, as on Android
  TV (collections stay on each library's Collections tab). Collapsed it
  shows icons, the current page's marked; Left opens it over the page with
  each entry's name. The profile's menu has Switch profile, Choose a
  server, Audio and subtitles (below), Sort libraries and Sign out. The
  libraries come in the profile's order: the profile's menu has Sort
  libraries (a library, then where it goes), a library's
  menu key Move up, Move down and Use the server's order, each saved at
  once for every device of the profile's (`PUT /api/me/libraries/order`);
  sorted elsewhere, the rail follows.
- **Libraries** (TvLibrariesScreen): every library as a card, three to a
  row: its kind's icon, its name, how much it holds and "Scanning…" while
  the server scans it, with Sort libraries at the top. A card's menu key
  has Open, Move up, Move down and Use the server's order. A library opened
  from here marks its own rail entry when it is among the first three,
  else Libraries.
- **Home**: the hero for what to watch next (the first of Continue
  Watching, else the newest) over its backdrop, with Resume or Play, From
  start, Info and Watched; under it the server's rows. Down goes into the
  rows: the hero gives way to an info block about the tile focus rests on
  (its title, its line of year, rating, runtime and quality badges, and
  two lines of summary), and the backdrop follows the tile too, once focus
  has rested 0.4 s. Rows keep the focused tile at the page's edge, the one
  before peeking out under the rail; a library's rows end with See all.
- **A title's page**: its backdrop, title, line (a show's seasons and
  years, a film's year and runtime, the content rating and quality
  badges), what is left, genres, summary and credits, then Resume (a
  show's "Resume S1 · E3"), From start, Watched, My List, Trailer, Add to
  playlist and More
  (the full summary, the show or season of an episode, and Your rating:
  five stars, halves allowed, or Not rated). Then a show's
  seasons as tabs and their episodes, Extras, Cast & Crew and More Like
  This; the info block says which episode focus is on.
- **Music** (the web's artist and album pages): an artist's page has its
  albums and tracks counted, Play and Shuffle, then its albums, newest
  first; an album's its artist, year, tracks and length, Play, Shuffle and
  More (Go to artist), then its tracks, numbered with their length. A
  track plays the album on from it. They play in Kodi's music playlist,
  each asked of the server as Kodi comes to it (the plugin's play route),
  so each is reported as it plays.
- **Playlists**: Add to playlist and a library's Save as… make and fill
  them. A title's menu in a playlist has Move up, Move down and Remove
  from playlist (a smart playlist fills itself: not those), then Rename
  playlist and Delete playlist, which asks first; a playlist's menu on
  Playlists has the last two. Kodi's lists have Rename and Delete on a
  playlist and Remove from playlist on its titles. A music playlist's
  tracks play on from the one chosen.
- **A library**: Recommended (its rows), Library (every title in a grid,
  more loading as focus nears the end) and Collections. It opens on the
  tab it was last left on, as on the other TVs: the add-on remembers that
  for each server, profile and library in its data folder
  (`remembered.json`), and opens on Library the first time. Library's chips
  sort it and filter it: Filter (all, unwatched or watched titles, the
  genre, the year, Clear filters, and Save as…, which makes a smart
  playlist of the filters), then Unwatched, Genre and Year, a check on
  the ones on; the count says "9 of 12 movies" while filtered. **Search** shows what it found by kind:
  movies, shows, episodes, people and music. Playlists, collections and a
  person's titles are grids.
- Kodi plays, with the service's Skip Intro and Up Next; back from playing,
  the page shows what changed (where the title got to, the next episode).
  Changes from other devices show as they happen: the service tells the
  window (`NotifyAll`) as it refreshes listings. A tile's menu (the
  remote's menu key) marks it watched, adds it to My List or a playlist,
  takes it out of Continue Watching or goes to its show.

How it is drawn: one window (`window.py`, `layout.WINDOW`) holds every
screen, each a group shown alone under the one rail and backdrop, so going
from a page to another moves no window; Back goes to the pages under it as
they were. `apps/kodi/skin.py` writes the window's XML from
`resources/lib/pley/layout.py`, which the window code reads too (sizes are
the Android app's dp doubled: it lays out 960 by 540, Kodi's skins 1920 by
1080): `python apps/kodi/skin.py` after changing them, and the tests fail
when the XML is behind. The pictures (`resources/skins/default/media`)
are rendered by `bun scripts/render-kodi-skin.mjs`: the Android app's own
Lucide icons (read from its `PleyIcons.kt`), its buttons, tile masks and
focus ring, the backdrop's gradients and the artwork placeholders, which
pick their colour from the title as the web's do. A mask keeps to the
picture's place (`scalediffuse` off), so round photos stay round and
cropped posters keep their corners. Text is in Estuary's
fonts (Kodi lends an add-on's windows the skin's fonts, by name): with
another skin, Pley's screens use its default font.

Settings → Screens → Kodi's lists shows the add-on as folders in the skin's
own views instead (below), which skins' widgets and favourites always get.

## How it works

- **The service** (`service.py`) starts with Kodi. It keeps the tokens
  (`addon_data/plugin.video.pley/tokens.json`, readable by Kodi's own user
  only), and it alone renews them:
  each refresh token works once, and two renewals with the same one end
  the sign-in.
- **The proxy.** Pley takes an app's token in the `Authorization` header
  only, never in an address, and an access token lasts an hour, shorter
  than a film. Kodi's player and artwork cache only know addresses, so the
  service runs a proxy on `127.0.0.1` and everything Kodi asks for goes
  through it (`http://127.0.0.1:<port>/<secret>/api/…`): it adds the
  current token, renews it when the server refuses it, and passes ranges,
  playlists and segments through. The secret keeps other programs and web
  pages on the device out; the port and secret are kept, so artwork stays
  cached across restarts.
- **The plugin** (`default.py`) opens Pley's window, lists Pley's
  screens as folders (for Kodi's lists, widgets and favourites) and asks
  to play
  (`POST /api/playback`) with a report of what this Kodi plays
  (`resources/lib/pley/caps.py`): every container and codec, audio tracks
  switched by Kodi, every kind of subtitle, the receiver formats Kodi's
  audio settings pass through, HDR10 and HLG always (Kodi tone-maps them on
  SDR screens), and Dolby Vision where Kodi says the screen shows it
  (profiles 5 and 8; 7 as well on CoreELEC). The quality setting (below)
  asks the server for a lower one, converted. Text subtitles come beside what
  plays: all of them beside a converted stream, which carries none, and
  beside a file played as it is the files next to it (Kodi reads the
  file's own tracks itself, and has no folder to look in for an address
  over HTTP). They are named as Kodi names its own (`3.eng.forced.vtt`),
  so Kodi lists them by language and says which are forced: flagged so,
  or only named so ("English (Forced)", "Signs & Songs"), as the server
  counts them.
  Kodi's own Subtitle offset (Subtitles in the player's controls) moves
  them, and Kodi remembers it for each file played as it is; it counts the
  other way from Pley's other apps, +0.3 s showing them earlier.
- **Audio and subtitles** (`tracks.py`) are the profile's, not Kodi's
  language settings. The audio is the server's choice for this Kodi: of
  the profile's language (the choice remembered for the show or the
  movie, else the profile's defaults), the track it plays best, by what
  its audio settings pass through and its speakers' channels (the
  playback's `audio_index`). The subtitles are the file's
  `selected_subtitle`, by the same rule. The service switches
  Kodi to them once Kodi lists the streams, and turns subtitles off when
  the choice is none. Pley names tracks by their index in the file, Kodi
  by its place in its lists: Kodi lists the subtitle files it was given
  first, in that order, then the file's own (it adds the files as it opens
  the address, before it reads the file), and a file's audio in its
  order; a converted stream has only the audio the server put in. Should
  Kodi list fewer than the file has, a track is found by its language
  when only one is in it. Picture subtitles (PGS, VobSub) chosen for a
  stream the server converts are drawn onto the video, as they are when
  they are a file next to the media, which Kodi isn't given. Audio or
  subtitles changed in Kodi's player (subtitles turned off too) are saved
  once the change has held 2 seconds, or as playback ends
  (`PUT /api/items/{id}/streams`), so the next episode starts with
  matching tracks on every device; subtitles changed alone leave the
  audio picked before as it was. The defaults themselves (Audio
  language, Subtitles: Off, Only for foreign dialogue, When the audio is
  in another language or Always, and the subtitle language for the last
  two) are Audio and subtitles in the profile's menu and in the add-on's
  settings (Playback), saved at once (`PUT /api/me/track-defaults`); a
  server that doesn't let apps change them says to change them on the
  web.
- **Find subtitles** (`subtitles.py`): the add-on is one of Kodi's
  subtitle services too (`xbmc.subtitle.module` in `addon.xml`), so the
  player's Subtitles → Download lists "pley" beside any other service
  installed, and Kodi's own dialog is the search's: Pley has no menu in
  Kodi's player, and this is where Kodi's people look for subtitles.
  Kodi asks the plugin (`?action=search`); for a title Pley plays (the
  service's `pley.playing`; anything else lists nothing) it lists what
  the server found on OpenSubtitles for the file playing, in the
  profile's subtitle language (`GET /api/items/{id}/subtitles/search`,
  no `language`: the server takes the profile's, English without one;
  Kodi's own language setting isn't used), by the language's name with
  the release under it, its flag, stars for how often it was downloaded
  (five for one made for this very file, also marked as in sync), and
  hearing impaired where it is. The one picked (`?action=download`) is
  added on the server (`POST /api/items/{id}/subtitles`), its WebVTT
  fetched through the proxy into Kodi's temp folder
  (`pley-subtitles/10001.eng.vtt`) and handed to Kodi, which loads it
  into the player and shows it; it is saved as the profile's choice for
  the show or the movie (`PUT /api/items/{id}/streams`), so the next
  episode and every other device start with it. Kodi lists the new track
  among the playback's where the service's layout doesn't know it, so
  the service stops following track changes for the rest of that
  playback (`pley.subtitle_added`). A server without OpenSubtitles set up
  (503) has Kodi say "Subtitle search isn't set up on this server. An
  administrator sets it up on the web, in Settings → Server."; one from
  before it (404), that finding subtitles needs a newer pley. Subtitles
  added on any device are ordinary text tracks here, beside a converted
  stream or a file played as it is (they are files next to it, to the
  server).
- **Listings** carry what Kodi shows of a title, since it asks the plugin
  for nothing more: synopses of every kind (`summaries=1`, docs/API.md),
  and artwork as Kodi's own library has it. An episode's still is its
  `thumb` and its show's poster its `tvshow.poster`: skins show an
  episode's own `poster` in place of its still. Only in a row of films and
  episodes (Home's) is the show's poster the episode's `poster` too, so
  poster views stay posters.
- **Quality** (docs/QUALITY.md; `autoquality.py`, `playback.ask`): the
  add-on's settings (Playback → Quality) offer the menu in its order,
  Original, Automatic, 30 Mbps 4K, 15 Mbps 4K, 20 Mbps 1080p and down to
  2 Mbps 480p, Automatic unless another was chosen. The setting is one
  for every title, so a quality is sent as a cap (`max_bitrate`,
  `max_height`, `mode: "auto"`): the server converts a file over it and
  plays one under it as it is, which is what the other apps do by
  offering a file only the qualities that make it smaller. A quality
  chosen before Automatic came is carried over the first time the
  service or a playback reads the setting; Original can't be told from
  the setting never changed, and becomes Automatic.
  Kodi's player has no quality menu of Pley's, so the choice is made in
  the settings, before playing.
- **Automatic** plays the original and watches it from the service, once
  a second while it plays (not paused, seeking, fast-forwarding or
  rewinding, nor in the seconds after a seek or the start). Kodi's own
  HLS player takes one variant of a playlist and keeps it, so the add-on
  doesn't ask for transcodes in several sizes (`hls.variants`,
  docs/QUALITY.md) and steps down itself:
  - *Waits*: Kodi buffering (`Player.Caching`), or the position not
    moving while it plays. One of 2 seconds or more, or two shorter ones
    within a minute, is a play issue.
  - *Delivery*: where Kodi's player keeps a cache (a file played as it
    is, and in Kodi 21 a converted stream too), JSON-RPC's
    `cachepercentage` says how much is read ahead; video
    arriving at under 1.1 times real time for 15 seconds with under 10
    seconds ahead and falling is a play issue before it waits.
  - *Decoding*: Kodi tells add-ons nothing of dropped frames (only its
    debug overlay shows them), so a device that can't keep up isn't seen;
    the server converts what this Kodi's caps say it can't decode.

  On a play issue it steps down by the spec's rule: the bitrate playing
  (the file's, the quality's for a conversion) times the delivery seen
  (else what the proxy saw arrive over the last 20 s) times 0.75, never
  above the server's own quality for the file (`auto_quality`), always
  a quality lower, never under 2 Mbps 480p, and not within 20 s of the
  last change. The service asks for the title again where it is
  (`playback.prepare` with `mode: "transcode"` and the quality's
  `max_bitrate` and `max_height`), plays it in place from that position,
  and says "Quality lowered to 8 Mbps 1080p" in Kodi's
  notification once it plays. The old session ends with the usual last
  report. The ceiling is kept for 30 minutes for each server in the
  add-on's data (`remembered.json`); a title started in Automatic
  meanwhile, Up Next's included, asks for the quality under it as a cap
  (a file under it still plays as it is).

  **Where it starts.** The server remembers what this Kodi's network carries
  too (`GET /api/playback/network`, docs/API.md): a day after a step down, a
  week after a speed test. Before a title starts in Automatic the plugin
  asks it (1.5 s at most), and its ceiling takes the place of the one kept
  here; without an answer (out of reach, or a server from before it, 404)
  the one kept here counts, as before. Where the server asks for one (a
  network away from it that it knows nothing of), a speed test runs first
  under Kodi's busy circle: 32 MiB of `GET /api/playback/speedtest`, read as
  it comes through the proxy and let go 2 s after its first byte, its speed
  told to the server (`probe_kbps`, and `probe_complete` when all of it came
  in time), whose answer is the ceiling (70 % of the speed when it can't
  keep it). Under 256 KiB measures nothing and the original starts. The
  asking and the test hold the play back 3.5 s at most. The service's proxy
  keeps the server's answer 10 minutes for each sign-in
  (`proxy.NetworkMemo`, a 404 too) and asks for it as Pley's window opens,
  so a play on the home network doesn't wait for it. Every ceiling the
  service keeps here is told to the server as well (`PUT`, through the
  proxy, whose kept answer it replaces): a step down's, a step up's to a
  quality (its bitrate, now kept here too), and none back at Original or
  with Automatic chosen again in the settings. Original and the chosen
  qualities ask nothing.

  **Back up** needs a measure of the network, which only the proxy has:
  every segment and file passes through it, and it times each answer
  from its first byte to its last, Kodi's taking of them included, so a
  measure is never more than the network carried. After 3 minutes
  without an issue, and 5 after a step down, when the third fastest
  answer of 256 KB or more since the last change or issue carried
  1.6 times the next quality up (the file's bitrate for Original), it
  steps up one quality, silently, at most every 3 minutes; back at
  Original the ceiling is forgotten. Kodi reads a stream only as fast as
  its buffers empty once they are full, so answers carry that much only
  while it fills them (after a start, a seek or a wait); on a Kodi whose
  buffers stay full it stays at the quality it went down to for the rest
  of the title, as the spec allows.
- **Progress**: the service tells the server where playback is every 10
  seconds while it plays and every 30 while it is paused (the server lets a
  player it hasn't heard from for 90 seconds go, and its conversion with
  it), and when it stops, then ends the session, so resume points, watched
  marks and the server's dashboard follow Kodi.
- **Other devices**: the service keeps the server's event stream open
  (`events.py`, `GET /api/events`). When the watch state, My List or a
  library changes, here or on another device, the Pley window or listing
  on screen is refreshed once: 0.3 s after the first event of a burst, at most every
  2 seconds, and never behind a playing video, whose end refreshes it. A
  stream stopped from elsewhere (an administrator, or another of the
  person's devices) ends at once with what they wrote; without the stream,
  the next progress report hears it (`410`), whether Kodi plays the file as
  it is or the server converts it.
- **Skip Intro, Skip Credits and Up Next**, by the web player's rules
  (`prompts.py`): a button over the video while it is full screen
  (`overlay.py`, `resources/skins/default/1080i/pley-overlay.xml`), which
  doesn't pause it, from a marker's start until a second before its end and
  for 10 seconds at most, since it holds the remote while it shows; an
  episode's credits bring up the next one, which plays after 10 seconds
  unless it is cancelled, also when the episode ends on its own. Up Next
  says which on a dark card, since a shadow alone is lost on a bright
  picture.
- **Without the server** (switched off, or another network), every screen
  says "Can't reach <server>" with Try again and Choose a server. Pley
  needs no internet: sign-in, artwork and playback all come from the
  server, and nothing else is ever asked.

## Kodi's player: speed, audio offset and the subtitles' look

Pley plays with Kodi's own player, whose controls have what the other
apps' players offer Pley's way, so the add-on adds none of them (it has
no menu in Kodi's player, and changes none of Kodi's settings):

- **Playback speed**: the player's settings (Estuary's gear) have Play speed,
  and the keyboard's Alt + Left and Right (`PlayerControl(TempoDown)`,
  `TempoUp`) step it: 0.8 to 1.5 times, the sound's pitch kept, for a file
  played as it is and a converted stream alike. Kodi's 2× and up are fast
  forward, without sound. So 0.5, 0.75 and 2 aren't there with sound, and
  nothing is offered while Kodi syncs to the audio (its default).
- **Audio offset**: the player's audio settings (Audio offset), which
  Kodi keeps for each file played as it is, like the subtitle offset
  above; a converted stream's address is its session's, so there it
  lasts the playback.
- **The subtitles' look**: Settings → Player → Language: font, size,
  style, colour, opacity, border, background (its kind, colour and
  opacity), shadow, blur, where they go (the bottom or top of the video
  or the screen) and the vertical margin, as Kodi 21.3's settings list
  them. They apply to the text subtitles Pley hands Kodi (WebVTT, the
  added ones too) and to the file's own (Override ASS/SSA subtitles
  styles for styled ones). The other apps keep their subtitle look on
  the device too (the web's in the browser, Android's in its settings),
  so there is nothing of the profile's to follow here.

## How fast

Measured on Kodi 21.3 for Windows, against a server on the same computer:

- Pley's window is up 0.2 s after the add-on is opened: Kodi reads the
  window's XML (1.3 MB, about 150 ms), and Home's rows, the profile and
  the libraries are asked for together (30 to 40 ms). A title's page
  opens with its details, seasons, episodes and More Like This asked for
  at once. Play is ready in 16 to 26 ms, from Kodi's own player.
- A listing takes 5 to 60 ms asked through Kodi's JSON-RPC, 17 to 34 ms
  at the median (Home, whose rows are asked for together, the longest):
  Kodi keeps the plugin's Python between calls (`reuselanguageinvoker`),
  where loading Python's modules again took about 550 ms of each. The
  first after Kodi starts takes 200 to 450 ms more: Kodi's Python on
  Windows has its standard library as source only,
  compiled as it loads, so the plugin loads little (`json` and
  `urllib.parse`); its requests to the proxy go over a socket of its own,
  without `http.client`, `ssl` and `email`.
- Play is ready in 7 to 25 ms (about 120 ms when it is the first thing
  asked after Kodi starts): the server's answer and the item's details are
  asked for together, the resume point and the file's type come with the
  listing, so Kodi doesn't ask first, and what this Kodi is (its version
  and platform, for the User-Agent) is asked once: each such question
  waits for Kodi's GUI, which is busy for whole frames while playback
  starts (110 to 480 ms when it was asked every time). Kodi shows a file's
  first frame 0.4 s after Play (a file without sound: the computer had no
  audio device, and Kodi held files with sound back 5 to 7 s looking for
  one).
- The service starts with Kodi, in the background, loading in about 0.4 s
  (1 s while Python's `http.server` still read the Windows registry's MIME
  types as it loaded, which the proxy never uses).
- Marked watched on another device, a title leaves Continue Watching on
  screen 0.44 to 0.48 s later; stopped from elsewhere, playback ends in
  0.02 to 0.05 s; stopped here, the listing shows the new place 0.18 s
  later.
- Seeking through the proxy takes as long as from the disk (1.24 to 1.36 s
  against 1.24 to 1.27 s for this file).
- A server that is off is said within 4 seconds.

## Checking it

```sh
cd apps/kodi/tests
python -m unittest discover -s . -p "test_*.py"
```

runs with any Python 3.8 or newer and nothing installed: Kodi's modules are
stood in for (`tests/stubs`). The proxy, the token rules, the sign-in, the
capability report, discovery, the listings, Pley's window (its tiles,
wording, rows and lines) and its XML (as `skin.py` writes it, with every
control the code moves) are tested there.
`test_e2e.py` runs the add-on's code against a real server (sign-in with a
code approved through the server's own endpoints, browsing, a file played
by ranges, a converted HLS stream, progress, a renewed token and signing
out); it needs a built server and FFmpeg:

```sh
PLEY_BIN=target/debug/pley PLEY_FFMPEG_DIR=/usr/bin python -m unittest test_e2e
```

`test_subtitles.py` walks Kodi's subtitle dialog through the plugin against
a stand-in server: the results, search not set up, nothing for what Pley
doesn't play, and the one picked added, saved and written for Kodi.

CI runs both. By hand, a portable Kodi (`kodi.exe -p` on Windows) with the
add-on copied into `portable_data/addons` and Kodi's JSON-RPC on
(Settings → Services → Control → Allow remote control from applications
on this system) can be driven over TCP port 9090: `Files.GetDirectory` on
`plugin://plugin.video.pley/…` lists the add-on's screens and `Player.Open`
plays. Kodi 21.3 on Windows has been through sign-in, every listing, direct
play with seeking, pause and resume, a converted stream, restarts, Skip
Intro, Up Next to the next episode, and the server going away and coming
back; Pley's window through every screen, playing from it and coming back.
Kodi only draws what changed: its screenshot of a still screen can show an
earlier frame, so a portable Kodi taking them wants
`<advancedsettings><gui><algorithmdirtyregions>0</algorithmdirtyregions></gui></advancedsettings>`
in `portable_data/userdata/advancedsettings.xml`.

## Not yet

- Find subtitles before playing (a title's page): Kodi's subtitle dialog
  is only in its player, and Kodi lists subtitles added on any device.
  Subtitle search in another language than the profile's: Kodi's dialog
  passes its own languages, which aren't used.

- What the Android app's pages have below their rows: a title's Details
  (plays, versions, media info) and the library's letter bar; a person's
  page with their photo (theirs is a grid of titles).
- In Kodi's lists, a title's information page shows its synopsis, not its
  cast, genres or crew: lists don't carry them.
- Downloads to watch without the server (the Android app's): Kodi plays
  from the server only.
- Playing a video playlist through, as the web's queue does: a title in
  one opens its page (a music playlist's tracks play on from the one
  chosen).
- Picture subtitles (PGS, VobSub) picked in Kodi's player during a
  converted stream: only text subtitles come beside it (the profile's
  choice is drawn onto the video from the start). Files played as they
  are show every kind.
- Automatic's "Automatic · 8 Mbps 1080p" and "Lowered for your connection":
  Kodi's player has no menu or details of Pley's to show them in; the
  notification says it once, and Kodi's own codec info (the `o` key)
  shows the stream it plays.
- `cachepercentage` and `Player.CacheLevel` read for the converted
  stream as for the file (Kodi caches HLS too). A restart holds the last
  picture under Kodi's busy circle for 3 to 6 seconds and plays on from
  the stream's keyframe, up to a second back; the notification's
  sentence is longer than Kodi's box and scrolls across once in its 5
  seconds. A step back up is unseen so far: how often a Kodi's segment
  reads prove the network carries more.
- "Who's watching?", music's pages and editing playlists have been through
  the tests only, not a running Kodi yet.
