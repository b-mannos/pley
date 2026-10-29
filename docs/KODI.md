# Pley for Kodi

A Kodi add-on, `plugin.video.pley`, for Kodi 20 (Nexus) and newer on every
platform Kodi runs on: Windows, macOS, Linux, LibreELEC and CoreELEC, and
Android TV boxes. It shows a Pley server in Pley's own screens, laid out
like the Android TV app's, and plays with Kodi's own player: files as they
are whenever Kodi can, and the server's conversion otherwise.

It is GPL (version 2 or later, as Kodi is), since it runs inside Kodi, and
talks to the server only over HTTP ([LICENSING.md](../LICENSING.md)). Its
pictures come from Lucide's icons (notice in `NOTICES.txt`, beside
`LICENSE.txt`). The add-on is in `apps/kodi/plugin.video.pley`, its tests
in `apps/kodi/tests`.

## Installing

No add-on repository carries it yet. Pack it and install the zip:

```sh
python apps/kodi/package.py        # dist/plugin.video.pley-<version>.zip
```

1. In Kodi, Settings → System → Add-ons → Unknown sources: on.
2. Settings → Add-ons → Install from zip file, and pick the zip (copied to
   the device, or over the network).

The icon (512 square, solid to the corners, as Kodi's add-on checker
wants) and fanart (1920 × 1080) are rendered with the other apps' icons:
`bun scripts/render-icons.mjs` at the repository's root.

## Signing in

1. Open the add-on and choose a server. Servers on the local network answer
   a broadcast (`pley?` on UDP port 7539, [API.md](API.md#discovery)); any
   other is typed in (`192.168.1.5`, `nas.local:7539`,
   `https://pley.example.com`).
2. Kodi shows a code and a QR code. Enter the code on the server's `/link`
   page from a phone or computer where someone is signed in. Change server
   goes back to the list.

The link page names this Kodi by its User-Agent (`pleyKodi/0.1.0 (Kodi
21.3; Windows)`). The add-on is one of Pley's own apps
(`https://hhaidar.github.io/pley/oauth/kodi.json`): it signs in only with a
code and gets a viewer's rights, `library play playlists profiles`
(browse, play, edit the profile's playlists, switch profiles).

Settings has Switch profile, Sign out (asks first) and Choose a server.
Choosing another server ends the old sign-in once the new one is made. The
same server found at another address must be signed in to again, since
anything on the network can answer the broadcast with a server's id.

**Who's watching?** In a household of more than one profile, Kodi asks
once each time it starts (not right after signing in: the approver chose
the profile). A profile with a PIN asks for it in Kodi's number dialog
(never kept); errors (wrong PIN, too many tries, a locked PIN, a profile
the app can't use) show under the faces. A profile that needs its password
is signed in to again with a code. Back leaves Pley. The profile's menu
(the rail's avatar) and the add-on's settings also have Switch profile.

## Screens

Opened from Kodi's add-ons (or a favourite), Pley shows its own screens
over Kodi, after the Android TV app's.

- **The rail**: the profile, Search, Home, the first three libraries,
  Playlists, Collections and Settings. With more than three libraries,
  Libraries takes Collections' place. Left opens it with names.
- **The profile's menu**: Switch profile, Choose a server, Audio and
  subtitles, Sort libraries and Sign out.
- **Library order** is the profile's, saved for all its devices
  (`PUT /api/me/libraries/order`): Sort libraries, or a library's menu key
  (Move up, Move down, Use the server's order).
- **Libraries**: every library as a card, with how much it holds and
  "Scanning…" while the server scans it.
- **Home**: a hero for what to watch next (Resume or Play, From start, Info,
  Watched) over the server's rows. In the rows, an info block and the
  backdrop follow the focused tile.
- **A title's page**: Resume, From start, Watched, My List, Trailer, Add to
  playlist and More (full summary, the show or season, Your rating: five
  stars, halves allowed). Then seasons and episodes, Extras, Cast & Crew and
  More Like This.
- **Music**: artist and album pages with Play and Shuffle. A track plays
  the album on from it in Kodi's music playlist, each track reported as it
  plays.
- **Playlists**: Add to playlist and a library's Save as… make and fill
  them. A title's menu in a playlist has Move up, Move down and Remove from
  playlist (not in smart playlists), Rename playlist and Delete playlist.
- **A library**: Recommended, Library (a grid with Filter, Unwatched, Genre
  and Year chips; Save as… makes a smart playlist of the filters) and
  Collections. It reopens on the tab last used (kept per server, profile
  and library in `remembered.json`).
- **Search** groups results by kind: movies, shows, episodes, people and
  music.
- **A tile's menu** (the remote's menu key): mark watched, My List, add to a
  playlist, remove from Continue Watching, go to the show.
- **Playing**: Kodi plays, with Skip Intro, Skip Credits and Up Next. Pages
  update when playback ends and when other devices change something.

**Kodi's lists.** Settings → Screens → Kodi's lists shows the add-on as
folders in the skin's own views instead. Skins' widgets and favourites
always get these folders.

## Audio and subtitles

Tracks follow the profile, not Kodi's language settings (`tracks.py`).

- The server picks the audio (`audio_index`) and subtitles
  (`selected_subtitle`) for this Kodi: the profile's language, the choice
  remembered for the show or movie, and what Kodi's audio settings pass
  through. The service switches Kodi to them once it lists the streams.
- Tracks changed in Kodi's player are saved once they have held 2 seconds,
  or when playback ends (`PUT /api/items/{id}/streams`), for every device.
- The defaults (Audio language; Subtitles: Off, Only for foreign dialogue,
  When the audio is in another language, Always; subtitle language) are
  Audio and subtitles in the profile's menu and in the add-on's settings
  (Playback), saved at once (`PUT /api/me/track-defaults`).
- Text subtitles are handed to Kodi as WebVTT files named the Kodi way
  (`3.eng.forced.vtt`): all of them beside a converted stream, the
  external ones beside a file played as it is.
- Picture subtitles (PGS, VobSub) chosen for a converted stream, or kept
  as a file next to the media, are drawn onto the video by the server.
- Kodi's own Subtitle offset counts the other way from Pley's other apps:
  +0.3 s shows them earlier.

### Find subtitles

The add-on is also a Kodi subtitle service (`xbmc.subtitle.module`,
`subtitles.py`): in the player, Subtitles → Download lists "pley". For a
title Pley plays, it lists what the server finds on OpenSubtitles in the
profile's subtitle language (`GET /api/items/{id}/subtitles/search`;
Kodi's own language setting isn't used), five stars for one made for this
very file. The one picked is added on the server
(`POST /api/items/{id}/subtitles`), loaded into the player and saved as the
profile's choice. A server without OpenSubtitles set up (503) says
"Subtitle search isn't set up on this server. An administrator sets it up
on the web, in Settings → Server."; an older one (404), that it needs a
newer pley.

## Quality

The add-on's settings (Playback → Quality) offer
[the menu](QUALITY.md#the-menu): Original, Automatic (the default),
30 Mbps 4K down to 2 Mbps 480p. Kodi's player has no Pley quality menu, so
the choice is made before playing. One setting serves every title, so a
quality is a cap: the server converts a file above it and plays one below
it as it is.

[Automatic](QUALITY.md#automatic) follows QUALITY.md's rules for
[players that go black between streams](QUALITY.md#players-that-go-black-between-streams),
since Kodi's player closes one stream before opening the next. Kodi's own
part (`autoquality.py`):

- It asks for one size only (no `hls.variants`): Kodi's HLS player keeps
  the first variant it takes.
- Waits are `Player.Caching` or a position that doesn't move; delivery is
  JSON-RPC's `cachepercentage` (for a file played as it is, and on Kodi 21
  a converted stream too). Kodi reports no dropped frames, so that rule
  waits for a Kodi that does.
- A conversion's bitrate is lowered as it is sent
  (`PUT /api/playback/{session}/bitrate`) in whole Mbps, above the
  floors of #620; a server that refuses it (400, 404) is treated as one
  whose bitrate can't change.
- When Kodi's player gives up on the original (`onPlayBackError`), it plays
  again converted at the server's quality, once, from where it was; Kodi's
  "Playback failed" box shows meanwhile.
- A step down shows in Kodi's notification ("Quality lowered to 8 Mbps
  1080p"). The "Automatic · 8 Mbps 1080p" label has nowhere to show; Kodi's
  codec info (the `o` key) shows the stream.
- Where the next title starts: each step down is kept 30 minutes per server
  in `remembered.json`, and sent to the server's network memory
  ([API.md](API.md#network-memory-and-speed-test)), which is asked first.
  Steps up to a quality are kept too. A speed test, when the server
  asks for one, runs under Kodi's busy circle.
- Only the proxy can measure the network for a step up. Kodi reads a full
  buffer only as fast as it plays, so on a Kodi whose buffers stay full
  nothing measures a way up. A stream that can't go higher raises where
  the next title starts instead.

## Kodi's player

The add-on adds no controls to Kodi's player and changes none of Kodi's
settings. Kodi's own:

- **Playback speed**: Play speed in the player's settings, or Alt + Left
  and Right on a keyboard: 0.8 to 1.5 times, pitch kept. Kodi offers it only
  with Settings → Player → Videos → Sync playback to display on (off by
  default). 0.5, 0.75 and 2 aren't available with sound.
- **Audio offset** in the player's audio settings: kept per file played as
  it is; for a converted stream, for that playback.
- **The subtitles' look**: Settings → Player → Language (font, size, colour,
  border, background, position and more). It applies to the subtitles Pley
  hands Kodi and to the file's own (Override ASS/SSA subtitles styles for
  styled ones).

## How it works

- **The service** (`service.py`) starts with Kodi. It keeps the tokens in
  `addon_data/plugin.video.pley/tokens.json` (readable by Kodi's user only)
  and alone renews them, since each refresh token works once.
- **The proxy**: Pley takes a token only in the `Authorization` header, and
  Kodi's player and artwork cache know only addresses. So the service runs
  a proxy on `127.0.0.1` (`http://127.0.0.1:<port>/<secret>/api/…`) that
  adds and renews the token. The secret keeps other programs on the device
  out; the port and secret are kept so artwork stays cached.
- **The plugin** (`default.py`) opens Pley's window, lists screens as
  folders for Kodi's lists, and asks to play (`POST /api/playback`) with
  what this Kodi plays (`resources/lib/pley/caps.py`): every container,
  codec and subtitle kind, the formats its audio settings pass through,
  HDR10 and HLG, and Dolby Vision where Kodi says the screen shows it
  (profiles 5 and 8; 7 too on CoreELEC). Listings carry synopses
  (`summaries=1`) and artwork as Kodi's library has it.
- **Progress** goes to the server every 10 seconds while playing, every 30
  while paused, and on stop.
- **Other devices**: the service keeps the event stream open (`events.py`,
  `GET /api/events`) and refreshes what is on screen, never behind a
  playing video. A stream stopped from elsewhere ends at once.
- **Skip Intro, Skip Credits and Up Next** follow the web player's rules
  (`prompts.py`, `overlay.py`): a button over the video for 10 seconds at
  most; Up Next plays the next episode after 10 seconds unless cancelled.
- **Without the server**, every screen says "Can't reach <server>" with Try
  again and Choose a server. Pley needs no internet.
- **The window**: one window (`window.py`) holds every screen.
  `apps/kodi/skin.py` writes its XML from `resources/lib/pley/layout.py`
  (sizes are the Android app's dp doubled); run `python apps/kodi/skin.py`
  after changing them, or the tests fail. The pictures
  (`resources/skins/default/media`) are rendered by
  `bun scripts/render-kodi-skin.mjs` from the Android app's icons and
  shapes. Text uses Estuary's fonts; other skins get their default font.

## Checking it

Unit tests, with any Python 3.8 or newer and nothing installed (Kodi's
modules are stubbed in `tests/stubs`):

```sh
cd apps/kodi/tests
python -m unittest discover -s . -p "test_*.py"
```

`test_e2e.py` runs the add-on against a real server (sign-in, browsing,
direct play, a converted stream, progress, token renewal, sign-out). It
needs a built server and FFmpeg:

```sh
PLEY_BIN=target/debug/pley PLEY_FFMPEG_DIR=/usr/bin python -m unittest test_e2e
```

`test_subtitles.py` walks Kodi's subtitle dialog through the plugin
against a stand-in server. CI runs all of them.

By hand: a portable Kodi (`kodi.exe -p` on Windows) with the add-on copied
into `portable_data/addons` and JSON-RPC on (Settings → Services → Control
→ Allow remote control from applications on this system) can be driven
over TCP port 9090: `Files.GetDirectory` on `plugin://plugin.video.pley/…`
lists screens and `Player.Open` plays. Kodi redraws only what changed, so
for screenshots put this in `portable_data/userdata/advancedsettings.xml`:

```xml
<advancedsettings><gui><algorithmdirtyregions>0</algorithmdirtyregions></gui></advancedsettings>
```

## Not yet

- Finding subtitles before playing, or in another language than the
  profile's.
- A title's Details (plays, versions, media info), the library's letter
  bar, and a person's page with their photo.
- Cast, genres and crew on a title's information page in Kodi's lists.
- Downloads: Kodi plays from the server only.
- Playing a video playlist through: a title in one opens its page (music
  playlists play through).
- Picture subtitles picked in Kodi's player during a converted stream (the
  profile's choice is drawn onto the video from the start). Files played
  as they are show every kind.
- Not yet seen on a running Kodi: Automatic stepping back up, a bitrate
  lowered as it is sent, and Kodi's player giving up on the original.
