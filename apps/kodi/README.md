# pley for Kodi

A Kodi add-on, `plugin.video.pley`, for Kodi 20 (Nexus) and newer on any
platform Kodi runs on: Windows, macOS, Linux, LibreELEC and CoreELEC,
Android TV boxes. It shows a pley server's Home rows (Continue Watching,
My List, Recently Added), every library, collections, playlists and
search, and plays with Kodi's own player: files as they are whenever Kodi
can, which is nearly always, and the server's conversion otherwise.

It is free software under the GPL (version 2 or later, as Kodi), since it
runs inside Kodi; it talks to the server only over HTTP, like the other
apps (LICENSING.md).

## Installing

No add-on repository carries it yet. Pack it and install the zip:

```sh
python apps/kodi/package.py        # dist/plugin.video.pley-<version>.zip
```

In Kodi: Settings → System → Add-ons → Unknown sources on, then
Settings → Add-ons → Install from zip file, and pick the zip (copy it to
the device first, or reach it over the network).

## Signing in

Open the add-on and choose a server: pley servers on the local network
answer a broadcast (docs/API.md; `pley?` on UDP port 7539), and any other
is typed in (`192.168.1.5`, `nas.local:7539`, `https://pley.example.com`).
Kodi then shows a code, typed on the server's `/link` page from a phone
or computer where someone is signed in, as on pley's Android TV app, and
the same as a QR code the server draws (`signin.py`,
`resources/skins/default/1080i/pley-signin.xml`). The link page names
this Kodi by what its User-Agent says (`pleyKodi/0.1.0 (Kodi 21.3;
Windows)`), so two of them can be told apart. The
add-on is one of pley's own apps (`https://hhaidar.github.io/pley/oauth/kodi.json`):
it signs in only with a code and gets a viewer's rights, whoever allows it.
Settings has Sign out and Choose a server. Choosing another server keeps
the sign-in there until the new one is made, then ends it on its server.
The same server at another address is signed in to again: anything on the
network can answer the broadcast with a server's id, and the tokens would
go to it.

## How it works

- **The service** (`service.py`) starts with Kodi. It keeps the tokens
  (`addon_data/plugin.video.pley/tokens.json`, readable by Kodi's own user
  only), and it alone renews them:
  each refresh token works once, and two renewals with the same one end
  the sign-in.
- **The proxy.** pley takes an app's token in the `Authorization` header
  only, never in an address, and an access token lasts an hour, shorter
  than a film. Kodi's player and artwork cache only know addresses, so the
  service runs a proxy on `127.0.0.1` and everything Kodi asks for goes
  through it (`http://127.0.0.1:<port>/<secret>/api/…`): it adds the
  current token, renews it when the server refuses it, and passes ranges,
  playlists and segments through. The secret keeps other programs and web
  pages on the device out; the port and secret are kept, so artwork stays
  cached across restarts.
- **The plugin** (`default.py`) lists pley's screens and asks to play
  (`POST /api/playback`) with a report of what this Kodi plays
  (`resources/lib/pley/caps.py`): every container and codec, audio tracks
  switched by Kodi, every kind of subtitle, the receiver formats Kodi's
  audio settings pass through, HDR10 and HLG always (Kodi tone-maps them on
  SDR screens), and Dolby Vision where Kodi says the screen shows it
  (profiles 5 and 8; 7 as well on CoreELEC). The quality setting asks the
  server for a lower one, converted. Subtitles beside a file are named as
  Kodi names its own (`3.eng.forced.vtt`), so Kodi lists them by language
  and picks a forced one as it picks those.
- **Progress**: the service tells the server where playback is every 10
  seconds while it plays and every 30 while it is paused (the server lets a
  player it hasn't heard from for 90 seconds go, and its conversion with
  it), and when it stops, then ends the session, so resume points, watched
  marks and the server's dashboard follow Kodi.
- **Other devices**: the service keeps the server's event stream open
  (`events.py`, `GET /api/events`). When the watch state, My List or a
  library changes, here or on another device, the pley listing on screen
  is refreshed once: 0.3 s after the first event of a burst, at most every
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
  unless it is cancelled, also when the episode ends on its own.
- **Without the server** (switched off, or another network), every screen
  says "Can't reach <server>" with Try again and Choose a server. pley
  needs no internet: sign-in, artwork and playback all come from the
  server, and nothing else is ever asked.

## How fast

Measured on Kodi 21.3 for Windows, against a server on the same computer:

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
capability report, discovery and the listings are tested there.
`test_e2e.py` runs the add-on's code against a real server (sign-in with a
code approved through the server's own endpoints, browsing, a file played
by ranges, a converted HLS stream, progress, a renewed token and signing
out); it needs a built server and FFmpeg:

```sh
PLEY_BIN=target/debug/pley PLEY_FFMPEG_DIR=/usr/bin python -m unittest test_e2e
```

CI runs both. By hand, a portable Kodi (`kodi.exe -p` on Windows) with the
add-on copied into `portable_data/addons` and Kodi's JSON-RPC on
(Settings → Services → Control → Allow remote control from applications
on this system) can be driven over TCP port 9090: `Files.GetDirectory` on
`plugin://plugin.video.pley/…` lists the add-on's screens and `Player.Open`
plays. Kodi 21.3 on Windows has been through sign-in, every listing, direct
play with seeking, pause and resume, a converted stream, restarts, Skip
Intro, Up Next to the next episode, and the server going away and coming
back.

## Not yet

- "Who's watching?" and switching profiles (the `profiles` scope).
- Making and editing playlists; people's pages.
- Picture subtitles (PGS, VobSub) with a converted stream: only text
  subtitles come beside it. Files played as they are show every kind.
