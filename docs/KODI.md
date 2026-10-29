# Pley for Kodi

A Kodi add-on, `plugin.video.pley`, for Kodi 20 (Nexus) and newer on every
platform Kodi runs on: Windows, macOS, Linux, LibreELEC and CoreELEC, and
Android TV boxes. It shows a Pley server in Pley's own screens, laid out
like the Android TV app's, and plays with Kodi's own player: files as they
are whenever Kodi can, which is nearly always, and the server's conversion
otherwise.

It is free software under the GPL (version 2 or later, as Kodi is), since
it runs inside Kodi. It talks to the server only over HTTP, like the other
apps ([LICENSING.md](../LICENSING.md)). Its pictures are drawn from
Lucide's icons, whose notice is in `NOTICES.txt` beside `LICENSE.txt`.

The add-on is in `apps/kodi/plugin.video.pley` and its tests in `apps/kodi/tests`;
file names below are the add-on's own.

## Installing

No add-on repository carries it yet. Pack it and install the zip:

```sh
python apps/kodi/package.py        # dist/plugin.video.pley-<version>.zip
```

In Kodi:

1. Settings → System → Add-ons → Unknown sources: on.
2. Settings → Add-ons → Install from zip file, and pick the zip (copy it to
   the device first, or reach it over the network).

The icon (512 square, solid to the corners, as Kodi's add-on checker
wants) and fanart (1920 × 1080) are rendered from Pley's logo with the
other apps' icons: `bun scripts/render-icons.mjs` at the repository's root.

## Signing in

1. Open the add-on and choose a server. Pley servers on the local network
   answer a broadcast (`pley?` on UDP port 7539, [API.md](API.md#discovery));
   any other is typed in (`192.168.1.5`, `nas.local:7539`,
   `https://pley.example.com`).
2. Kodi shows a code and a QR code (`signin.py`,
   `resources/skins/default/1080i/pley-signin.xml`, laid out like the
   Android TV app's link screen). Type the code on the server's `/link`
   page from a phone or computer where someone is signed in. Change server
   goes back to the servers.

The link page names this Kodi by its User-Agent (`pleyKodi/0.1.0 (Kodi
21.3; Windows)`), so two of them can be told apart.

The add-on is one of Pley's own apps
(`https://hhaidar.github.io/pley/oauth/kodi.json`). It signs in only with a
code and gets a viewer's rights, whoever allows it: `library play playlists
profiles` (browse, play, make and change the profile's playlists, switch to
the household's other profiles). A Kodi signed in before the add-on asked
for `playlists` is told to sign in again when it adds to a playlist; one
signed in before `profiles` is asked to sign in again when it chooses
Switch profile.

Settings has Switch profile, Sign out (which asks "Sign out?" first) and
Choose a server. Choosing another server keeps the sign-in until the new
one is made, then ends it on its server. The same server found at another
address is signed in to again: anything on the network can answer the
broadcast with a server's id, and the tokens would go to it.

### Who's watching?

In a household of more than one profile, Kodi asks "Who's watching?" once
each time it starts, as the TV apps do (not right after signing in:
whoever approved the code chose the profile).

- The household's faces in a row, without the rail, the profile in use in
  focus and a lock on those with a PIN.
- Another profile is switched to at once. One with a PIN asks for it in
  Kodi's number dialog (hidden, never kept); going on as a profile with a
  PIN asks for its PIN too.
- A wrong PIN, too many of them, a locked PIN or a profile the app can't
  use is said under the faces; nothing is retried by itself.
- A profile that needs its password is signed in to again with a code,
  chosen on the phone that approves it.
- Back leaves Pley.

The profile's menu (the rail's avatar) has Switch profile too, and so do
the add-on's settings for Kodi's lists. The service switches (the
switch-profile grant, [API.md](API.md)) under the same lock as its token
refreshes, keeps the profile in the token file, and opens the event stream
again as that profile.

## Pley's look

Opened from Kodi's add-ons (or a favourite), Pley shows its own screens
over Kodi, after the Android TV app's (`apps/android/ui/tv`).

- **The rail** on the left: the profile, Search, Home, the first three
  libraries, Playlists, Collections and Settings (the add-on's). With more
  than three libraries, Libraries takes Collections' place, as on Android
  TV (collections stay on each library's Collections tab). Collapsed, it
  shows icons, the current page's marked; Left opens it over the page with
  each entry's name.
- **The profile's menu**: Switch profile, Choose a server, Audio and
  subtitles, Sort libraries and Sign out.
- **Library order** is the profile's. Sort libraries (in the profile's
  menu and at the top of Libraries) picks a library, then where it goes; a
  library's menu key has Move up, Move down and Use the server's order.
  Each is saved at once for every device of the profile
  (`PUT /api/me/libraries/order`); sorted elsewhere, the rail follows.
- **Libraries** (as `TvLibrariesScreen`): every library as a card, three
  to a row, with its kind's icon, its name, how much it holds and
  "Scanning…" while the server scans it. A card's menu key has Open, Move
  up, Move down and Use the server's order. A library opened from here
  marks its own rail entry when it is among the first three, else
  Libraries.
- **Home**: a hero for what to watch next (the first of Continue Watching,
  else the newest) over its backdrop, with Resume or Play, From start,
  Info and Watched; under it the server's rows. Down goes into the rows:
  the hero gives way to an info block about the focused tile (title; year,
  rating, runtime and quality badges; two lines of summary), and the
  backdrop follows the tile once focus has rested 0.4 s. Rows keep the
  focused tile at the page's edge, the one before peeking out under the
  rail; a library's rows end with See all.
- **A title's page**: backdrop, title, a line (a show's seasons and years,
  a film's year and runtime, the content rating and quality badges), what
  is left, genres, summary and credits. Then Resume (a show's "Resume S1 ·
  E3"), From start, Watched, My List, Trailer, Add to playlist and More
  (the full summary, the show or season of an episode, and Your rating:
  five stars, halves allowed, or Not rated). Then a show's seasons as tabs
  with their episodes, Extras, Cast & Crew and More Like This; the info
  block says which episode has focus.
- **Music** (as the web's artist and album pages): an artist's page counts
  its albums and tracks, has Play and Shuffle, then its albums, newest
  first. An album's page has its artist, year, tracks and length, Play,
  Shuffle and More (Go to artist), then its tracks, numbered, with their
  length. A track plays the album on from it, in Kodi's music playlist;
  each track is asked of the server as Kodi comes to it (the plugin's play
  route), so each is reported as it plays.
- **Playlists**: Add to playlist and a library's Save as… make and fill
  them. A title's menu in a playlist has Move up, Move down and Remove from
  playlist (not in a smart playlist, which fills itself), then Rename
  playlist and Delete playlist (which asks first); a playlist's menu on
  Playlists has the last two. Kodi's lists have Rename and Delete on a
  playlist and Remove from playlist on its titles. A music playlist's
  tracks play on from the one chosen.
- **A library**: Recommended (its rows), Library (every title in a grid,
  more loading as focus nears the end) and Collections. It opens on the tab
  it was last left on, as on the other TVs, remembered per server, profile
  and library in the add-on's data folder (`remembered.json`); the first
  time, on Library. Library's chips sort and filter it: Filter (all,
  unwatched or watched titles, the genre, the year, Clear filters, and
  Save as…, which makes a smart playlist of the filters), then Unwatched,
  Genre and Year, with a check on those that are on. While filtered, the
  count says "9 of 12 movies".
- **Search** shows what it found by kind: movies, shows, episodes, people
  and music. Playlists, collections and a person's titles are grids.
- **A tile's menu** (the remote's menu key) marks it watched, adds it to My
  List or a playlist, takes it out of Continue Watching or goes to its
  show.
- **Playing**: Kodi plays, with Skip Intro and Up Next (below). Back from
  playing, the page shows what changed (where the title got to, the next
  episode). Changes from other devices show as they happen.

**Kodi's lists.** Settings → Screens → Kodi's lists shows the add-on as
folders in the skin's own views instead. Skins' widgets and favourites
always get these folders.

## Audio and subtitles

Audio and subtitles follow the profile, not Kodi's language settings
(`tracks.py`).

- **Audio** is the server's choice for this Kodi: of the profile's language
  (the choice remembered for the show or movie, else the profile's
  defaults), the track it plays best given what Kodi's audio settings pass
  through and its speakers' channels (the playback's `audio_index`).
- **Subtitles** are the file's `selected_subtitle`, by the same rule. The
  service switches Kodi to them once Kodi lists the streams, and turns
  subtitles off when the choice is none.
- **Changes** made in Kodi's player (subtitles turned off too) are saved
  once they have held 2 seconds, or when playback ends
  (`PUT /api/items/{id}/streams`), so the next episode starts with matching
  tracks on every device. Subtitles changed alone leave the audio as it
  was.
- **Defaults** (Audio language; Subtitles: Off, Only for foreign dialogue,
  When the audio is in another language or Always; and the subtitle
  language for the last two) are Audio and subtitles in the profile's menu
  and in the add-on's settings (Playback), saved at once
  (`PUT /api/me/track-defaults`). A server that doesn't let apps change
  them says to change them on the web.
- **Text subtitles** come beside what plays: all of them beside a converted
  stream (which carries none), and beside a file played as it is, the
  subtitle files next to it (Kodi reads the file's own tracks, and over
  HTTP has no folder to look in). They are named as Kodi names its own
  (`3.eng.forced.vtt`), so Kodi lists them by language and marks forced
  ones: those flagged forced, or only named so ("English (Forced)",
  "Signs & Songs"), as the server counts them. Subtitles added on any
  device are ordinary text tracks here.
- **Picture subtitles** (PGS, VobSub) are drawn onto the video when chosen
  for a stream the server converts, and when they are a file next to the
  media (Kodi isn't given those).
- **Subtitle offset**: Kodi's own (Subtitles in the player's controls)
  moves them, remembered by Kodi for each file played as it is. It counts
  the other way from Pley's other apps: +0.3 s shows them earlier.

How tracks are matched: Pley names tracks by their index in the file, Kodi
by their place in its lists. Kodi lists the subtitle files it was given
first, in that order, then the file's own (it adds the files as it opens
the address, before reading the file); a file's audio comes in its order,
and a converted stream has only the audio the server put in. If Kodi lists
fewer tracks than the file has, a track is found by its language when only
one track has it.

### Find subtitles

The add-on is also one of Kodi's subtitle services (`xbmc.subtitle.module`
in `addon.xml`, `subtitles.py`), so the player's Subtitles → Download lists
"pley" beside any other service installed. Pley has no menu in Kodi's
player, and this is where Kodi users look for subtitles.

- Kodi asks the plugin (`?action=search`). For a title Pley plays (the
  service's `pley.playing`; anything else lists nothing) it lists what the
  server found on OpenSubtitles for the file, in the profile's subtitle
  language (`GET /api/items/{id}/subtitles/search` with no `language`: the
  server takes the profile's, else English; Kodi's own language setting
  isn't used).
- Each result shows the language's name with the release under it, its
  flag, stars for how often it was downloaded (five for one made for this
  very file, also marked as in sync), and hearing impaired where it is.
- The one picked (`?action=download`) is added on the server
  (`POST /api/items/{id}/subtitles`), its WebVTT fetched through the proxy
  into Kodi's temp folder (`pley-subtitles/10001.eng.vtt`) and handed to
  Kodi, which loads and shows it. It is saved as the profile's choice for
  the show or movie (`PUT /api/items/{id}/streams`), so the next episode
  and every other device start with it.
- Kodi lists the new track where the service's track layout doesn't expect
  it, so the service stops following track changes for the rest of that
  playback (`pley.subtitle_added`).
- A server without OpenSubtitles set up (503) has Kodi say "Subtitle search
  isn't set up on this server. An administrator sets it up on the web, in
  Settings → Server."; a server too old for it (404), that finding
  subtitles needs a newer pley.

## Quality

The add-on's settings (Playback → Quality) offer
[the menu](QUALITY.md#the-menu) in its order: Original, Automatic, 30 Mbps
4K, 15 Mbps 4K, 20 Mbps 1080p and down to 2 Mbps 480p. Automatic is the
default. Kodi's player has no Pley quality menu, so the choice is made in
the settings, before playing.

- The setting is one for every title, so a quality is sent as a cap
  (`max_bitrate`, `max_height`, `mode: "auto"`): the server converts a file
  above it and plays one below it as it is. The other apps get the same
  result by offering a file only the qualities that make it smaller.
- A quality saved by an older add-on is carried over the first time the
  service or a playback reads the setting. An old Original can't be told
  from a setting never changed, and becomes Automatic.
- [Automatic](QUALITY.md#automatic) works as on the other apps, within
  Kodi's limits ([Automatic on Kodi](#automatic-on-kodi)).

## Kodi's player: speed, audio offset and the subtitles' look

Kodi's player has its own controls for what the other apps' players offer,
so the add-on adds none (it has no menu in Kodi's player and changes none
of Kodi's settings):

- **Playback speed**: the player's settings (Estuary's gear) have Play
  speed, and the keyboard's Alt + Left and Right
  (`PlayerControl(TempoDown)`, `TempoUp`) step it: 0.8 to 1.5 times, pitch
  kept, for a file played as it is and a converted stream alike. Kodi
  offers it only with Settings → Player → Videos → Sync playback to display
  on (not Kodi's default). Kodi 21.3 reported `Player.TempoEnabled` false
  with it off and true with it on, and took `Player.SetTempo` 0.8, 1.25 and
  1.5 but refused 0.5, 0.75 and 2. Kodi's 2× and up are fast forward,
  without sound.
- **Audio offset**: the player's audio settings (Audio offset). Kodi keeps
  it for each file played as it is; a converted stream's address is its
  session's, so there it lasts the playback.
- **The subtitles' look**: Settings → Player → Language: font, size, style,
  colour, opacity, border, background (kind, colour and opacity), shadow,
  blur, position (bottom or top of the video or the screen) and vertical
  margin, as Kodi 21.3 lists them. They apply to the text subtitles Pley
  hands Kodi (WebVTT, added ones too) and to the file's own (Override
  ASS/SSA subtitles styles for styled ones). The other apps also keep their
  subtitle look on the device, so there is no profile setting to follow.

## How it works

### The service and the proxy

- **The service** (`service.py`) starts with Kodi. It keeps the tokens
  (`addon_data/plugin.video.pley/tokens.json`, readable by Kodi's own user
  only) and alone renews them: each refresh token works once, and two
  renewals with the same one end the sign-in.
- **The proxy.** Pley takes an app's token only in the `Authorization`
  header, never in an address, and an access token lasts an hour, shorter
  than a film. Kodi's player and artwork cache only know addresses, so the
  service runs a proxy on `127.0.0.1` and everything Kodi asks for goes
  through it (`http://127.0.0.1:<port>/<secret>/api/…`). It adds the
  current token, renews it when the server refuses it, and passes ranges,
  playlists and segments through. The secret keeps other programs and web
  pages on the device out. The port and secret are kept, so artwork stays
  cached across restarts.

### Playing

- **The plugin** (`default.py`) opens Pley's window, lists Pley's screens
  as folders (for Kodi's lists, widgets and favourites) and asks to play
  (`POST /api/playback`).
- **What this Kodi plays** (`resources/lib/pley/caps.py`) goes with the
  request: every container and codec, audio tracks switched by Kodi, every
  kind of subtitle, the receiver formats Kodi's audio settings pass
  through, HDR10 and HLG always (Kodi tone-maps them on SDR screens), and
  Dolby Vision where Kodi says the screen shows it (profiles 5 and 8; 7 as
  well on CoreELEC).
- **Listings** carry everything Kodi shows of a title, since Kodi asks the
  plugin for nothing more: synopses of every kind (`summaries=1`,
  [API.md](API.md)), and artwork as Kodi's own library has it. An episode's
  still is its `thumb` and its show's poster its `tvshow.poster`, since
  skins show an episode's own `poster` in place of its still. Only in a row
  of films and episodes (Home's) is the show's poster also the episode's
  `poster`, so poster views stay posters.
- **Progress**: the service reports the position every 10 seconds while
  playing, every 30 while paused, and when playback stops, then ends the
  session. The server lets a player it hasn't heard from for 90 seconds go,
  with its conversion. Resume points, watched marks and the server's
  dashboard follow Kodi.
- **Skip Intro, Skip Credits and Up Next** follow the web player's rules
  (`prompts.py`): a button over the full-screen video (`overlay.py`,
  `resources/skins/default/1080i/pley-overlay.xml`) that doesn't pause it,
  from a marker's start until a second before its end, for 10 seconds at
  most since it holds the remote while it shows. An episode's credits
  bring up the next one, which plays after 10 seconds unless cancelled,
  also when the episode ends on its own. Up Next sits on a dark card, as a
  shadow alone is lost on a bright picture.

### Other devices and the server going away

- **Other devices**: the service keeps the server's event stream open
  (`events.py`, `GET /api/events`). When the watch state, My List or a
  library changes, here or elsewhere, the Pley window or listing on screen
  is refreshed once (the service tells the window with `NotifyAll`): 0.3 s
  after the first event of a burst, at most every 2 seconds, and never
  behind a playing video, whose end refreshes it.
- **Stopped from elsewhere** (an administrator, or another of the person's
  devices), a stream ends at once with their message. Without the event
  stream, the next progress report hears it (`410`), whether Kodi plays
  the file as it is or the server converts it.
- **Without the server** (switched off, or another network), every screen
  says "Can't reach <server>" with Try again and Choose a server. Pley
  needs no internet: sign-in, artwork and playback all come from the
  server, and nothing else is ever asked.

### The window

- One window (`window.py`, `layout.WINDOW`) holds every screen, each a
  group shown alone under the one rail and backdrop, so moving between
  pages opens no new window; Back returns to the pages under it as they
  were.
- `apps/kodi/skin.py` writes the window's XML from
  `resources/lib/pley/layout.py`, which the window code reads too. Sizes
  are the Android app's dp doubled (it lays out 960 × 540, Kodi's skins
  1920 × 1080). Run `python apps/kodi/skin.py` after changing them; the
  tests fail when the XML is behind.
- The pictures (`resources/skins/default/media`) are rendered by
  `bun scripts/render-kodi-skin.mjs`: the Android app's Lucide icons (read
  from its `PleyIcons.kt`), its buttons, tile masks and focus ring, the
  backdrop's gradients and the artwork placeholders, which pick their
  colour from the title as the web's do. A mask keeps to the picture's
  place (`scalediffuse` off), so round photos stay round and cropped
  posters keep their corners.
- Text is in Estuary's fonts (Kodi lends an add-on's windows the skin's
  fonts, by name); with another skin, Pley's screens use its default font.

### Automatic on Kodi

`autoquality.py` and `playback.ask` follow
[Automatic quality](QUALITY.md#automatic), including its rules for
[players that go black between streams](QUALITY.md#players-that-go-black-between-streams):
Kodi's player closes one stream before it opens the next. Kodi's HLS player
also takes one variant of a playlist and keeps it, so the add-on doesn't
ask for [several sizes](QUALITY.md#several-sizes-adaptive-bitrate)
(`hls.variants`) and steps by itself. What is Kodi's own:

**What it watches**: once a second, from the service, while playing (not
paused, seeking, fast-forwarding or rewinding, nor in the seconds after a
seek or the start).

- *Waits*: Kodi buffering (`Player.Caching`), or the position not moving
  while it plays.
- *Delivery*: JSON-RPC's `cachepercentage` (how much is read ahead), where
  Kodi's player keeps a cache: a file played as it is, and in Kodi 21 a
  converted stream too.
- *Decoding*: Kodi tells add-ons nothing of dropped frames (only its debug
  overlay shows them), so a device that can't keep up isn't seen; the
  server converts what this Kodi's capabilities say it can't decode. The
  one decoder failure Kodi reports is its player giving up on the original
  (`onPlayBackError`). The dropped-frames rule waits for a Kodi that
  reports them.

**Stepping down a conversion whose bitrate can change**
(`PlaybackInfo.live_bitrate`):

- When the delivery isn't known, the step uses what the proxy saw arrive
  over the last 20 s (no more than the bitrate), else half the bitrate.
- The service asks the server from its worker, with its token
  (`PUT /api/playback/{session}/bitrate`, `service.Player.set_bitrate`),
  and Kodi plays on. Kodi's notification says "Quality lowered to 4 Mbps
  1080p" at once.
- A refusal (an older server or transcoder, 400; a session gone, 404)
  leaves the stream as it was, and from then on it moves as one whose
  bitrate can't change.

**Moving from the original, or from a conversion whose bitrate can't
change**:

- A wait of 2 s or more steps down. When the delivery isn't known, the
  step uses what the proxy saw arrive over the last 20 s. Two short waits
  and the delivery warning restart the calm before a step up, but don't
  move it.
- Kodi's player giving up on the original (`onPlayBackError`,
  `service.Player.fall_back`) plays it again converted at the server's own
  quality, once, from where it was; Kodi's own "Playback failed" box shows
  meanwhile.
- The service asks for the title again (`playback.prepare` with
  `mode: "transcode"` and the quality's `max_bitrate` and `max_height`, or
  neither for the server's own) and plays it in place from that position.
  Once it plays, Kodi's notification says "Quality lowered to 8 Mbps
  1080p". The old session ends with the usual last report; until the new
  one plays, nothing playing doesn't end it.

**Where the next title starts**:

- The device's own memory: each step down's ceiling, kept 30 minutes per
  server in the add-on's data (`remembered.json`). A title started in
  Automatic meanwhile, Up Next's included, asks for the quality under it
  as a cap (a file under it still plays as it is).
- The server's memory (`GET /api/playback/network`,
  [API.md](API.md#network-memory-and-speed-test)), asked before a title
  starts in Automatic (1.5 s at most), replaces it. Without an answer (out
  of reach, or an older server, 404) the device's own counts.
- A speed test, where the server asks for one, runs under Kodi's busy
  circle: 32 MiB of `GET /api/playback/speedtest`, read through the proxy
  and let go 2 s after its first byte, its speed sent to the server
  (`probe_kbps`, and `probe_complete` when all of it came in time). Under
  256 KiB measures nothing and the original starts.
- Asking and testing hold the play back 3.5 s at most. The proxy keeps the
  server's answer 10 minutes per sign-in (`proxy.NetworkMemo`, a 404 too)
  and asks for it as Pley's window opens, so a play on the home network
  doesn't wait.
- Every ceiling kept here is also sent to the server (`PUT`, through the
  proxy, replacing its kept answer): a step down's, a step up's to a
  quality (its bitrate), and none when
  back at Original or with Automatic chosen again in the settings.
  Original and the chosen qualities ask nothing.

**Stepping back up**: only the proxy can measure the network. Every segment
and file passes through it, and it times each answer from its first byte
to its last, Kodi's reading included, so a measure is never more than the
network carried.

- A step up needs the third fastest answer of 256 KB or more since the last
  step to have carried 1.6 times the step up.
- A bitrate lowered as it is sent climbs one menu bitrate at a time, then
  to the stream's own. A stream that can't go higher raises where the next
  title starts instead.
- Once its buffers are full, Kodi reads a stream only as fast as it plays,
  so answers carry more only while it fills them (after a start, a seek or
  a wait). On a Kodi whose buffers stay full, nothing measures a way up, as
  QUALITY.md allows.
- Settling also covers a raise of where the next title starts that a step
  down undoes within 10 minutes.

## How fast

Measured on Kodi 21.3 for Windows, against a server on the same computer:

- Pley's window is up 0.2 s after the add-on is opened: Kodi reads the
  window's XML (1.3 MB, about 150 ms), and Home's rows, the profile and
  the libraries are asked for together (30 to 40 ms). A title's page asks
  for its details, seasons, episodes and More Like This at once. Play is
  ready in 16 to 26 ms, from Kodi's own player.
- A listing takes 5 to 60 ms through Kodi's JSON-RPC, 17 to 34 ms at the
  median (Home, whose rows are asked for together, the longest). Kodi keeps
  the plugin's Python between calls (`reuselanguageinvoker`), saving about
  550 ms of module loading each time. The first listing after Kodi starts
  takes 200 to 450 ms more: Kodi's Python on Windows ships its standard
  library as source only, compiled as it loads, so the plugin loads little
  (`json` and `urllib.parse`) and talks to the proxy over a socket of its
  own, without `http.client`, `ssl` and `email`.
- Play is ready in 7 to 25 ms (about 120 ms when it is the first thing
  asked after Kodi starts). The server's answer and the item's details are
  asked for together; the resume point and the file's type come with the
  listing; and what this Kodi is (version and platform, for the
  User-Agent) is asked once, since each such question waits for Kodi's
  GUI, busy for whole frames while playback starts.
- Kodi shows a file's first frame 0.4 s after Play (measured with a file
  without sound: the computer had no audio device, and Kodi held files
  with sound back 5 to 7 s looking for one).
- The service starts with Kodi, in the background, in about 0.4 s.
- Marked watched on another device, a title leaves Continue Watching on
  screen 0.44 to 0.48 s later. Stopped from elsewhere, playback ends in
  0.02 to 0.05 s. Stopped here, the listing shows the new place 0.18 s
  later.
- Seeking through the proxy takes as long as from the disk (1.24 to 1.36 s
  against 1.24 to 1.27 s for the test file).
- A server that is off is reported within 4 seconds.

## Checking it

Unit tests, with any Python 3.8 or newer and nothing installed (Kodi's
modules are stood in for in `tests/stubs`):

```sh
cd apps/kodi/tests
python -m unittest discover -s . -p "test_*.py"
```

They cover the proxy, the token rules, the sign-in, the capability report,
discovery, the listings, Pley's window (its tiles, wording, rows and lines)
and its XML (as `skin.py` writes it, with every control the code moves).

`test_e2e.py` runs the add-on's code against a real server: sign-in with a
code approved through the server's own endpoints, browsing, a file played
by ranges, a converted HLS stream, progress, a renewed token and signing
out. It needs a built server and FFmpeg:

```sh
PLEY_BIN=target/debug/pley PLEY_FFMPEG_DIR=/usr/bin python -m unittest test_e2e
```

`test_subtitles.py` walks Kodi's subtitle dialog through the plugin against
a stand-in server: the results, search not set up, nothing for what Pley
doesn't play, and the one picked added, saved and written for Kodi.

CI runs both.

**By hand**, run a portable Kodi (`kodi.exe -p` on Windows) with the add-on
copied into `portable_data/addons` and JSON-RPC on (Settings → Services →
Control → Allow remote control from applications on this system). Drive it
over TCP port 9090: `Files.GetDirectory` on `plugin://plugin.video.pley/…`
lists the add-on's screens and `Player.Open` plays.

Kodi only draws what changed, so its screenshot of a still screen can show
an earlier frame. A portable Kodi taking screenshots needs this in
`portable_data/userdata/advancedsettings.xml`:

```xml
<advancedsettings><gui><algorithmdirtyregions>0</algorithmdirtyregions></gui></advancedsettings>
```

Tried on Kodi 21.3 for Windows: sign-in, every listing, direct play with
seeking, pause and resume, a converted stream, restarts, Skip Intro, Up
Next to the next episode, the server going away and coming back, and
Pley's window through every screen, playing from it and coming back. Kodi
21.3 listed the add-on among its subtitle services (`Addons.GetAddons` of
`xbmc.subtitle.module`) and, with its dialog opened while a file played,
ran the plugin with `?action=search&languages=English`.

## Not yet

- Finding subtitles before playing (on a title's page): Kodi's subtitle
  dialog is only in its player. Subtitle search in another language than
  the profile's: Kodi's dialog passes its own languages, which aren't used.
- What the Android app's pages have below their rows: a title's Details
  (plays, versions, media info) and the library's letter bar; a person's
  page with their photo (here a person is a grid of titles).
- In Kodi's lists, a title's information page shows its synopsis, not its
  cast, genres or crew: lists don't carry them.
- Downloads to watch without the server (as in the Android app): Kodi plays
  from the server only.
- Playing a video playlist through, as the web's queue does: a title in one
  opens its page (a music playlist's tracks play on from the one chosen).
- Picture subtitles (PGS, VobSub) picked in Kodi's player during a
  converted stream: only text subtitles come beside it (the profile's
  choice is drawn onto the video from the start). Files played as they are
  show every kind.
- Automatic's "Automatic · 8 Mbps 1080p" and "Lowered for your
  connection": Kodi's player has no Pley menu or details to show them in.
  The notification says it once, and Kodi's codec info (the `o` key) shows
  the stream it plays.
- Automatic, partly seen on Kodi 21.3 for Windows with a relay slowing the
  server's side: a 16 Mbps 1080p file played as it is, slowed to 12 Mbps,
  stepped down to 8 Mbps 1080p as its read-ahead ran out, before any wait
  (`Player.Caching` stayed off). That converted stream, slowed to 4 Mbps,
  waited 2.6 seconds with `Player.Caching` on and stepped down to 2 Mbps
  480p. `cachepercentage` and `Player.CacheLevel` read for the converted
  stream as for the file (Kodi caches HLS too). A restart holds the last
  picture under Kodi's busy circle for 3 to 6 seconds and plays on from the
  stream's keyframe, up to a second back. The notification's sentence is
  longer than Kodi's box and scrolls across once in its 5 seconds. Not seen
  on a running Kodi yet: a step back up (how often a Kodi's segment reads
  prove the network carries more), a bitrate lowered as the stream is sent
  (only the tests and a real server's answer to the add-on's `PUT`), and
  Kodi's player giving up on the original.
- Kodi's whole subtitle dialog against a server with subtitle search.
- "Who's watching?", music's pages and editing playlists have been through
  the tests only, not a running Kodi.
