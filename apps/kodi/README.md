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
or computer where someone is signed in, as on pley's Android TV app. The
add-on is one of pley's own apps (`https://hhaidar.github.io/pley/oauth/kodi.json`):
it signs in only with a code and gets a viewer's rights, whoever allows it.
Settings has Sign out and Choose a server.

## How it works

- **The service** (`service.py`) starts with Kodi. It keeps the tokens
  (`addon_data/plugin.video.pley/tokens.json`), and it alone renews them:
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
  server for a lower one, converted.
- **Progress**: the service tells the server where playback is every 10
  seconds, on pause and when it stops, then ends the session, so resume
  points, watched marks and the server's dashboard follow Kodi. A stream
  an administrator stops ends in Kodi with their message.
- **Skip Intro, Skip Credits and Up Next**, by the web player's rules
  (`prompts.py`): a button over the video (`overlay.py`,
  `resources/skins/default/1080i/pley-overlay.xml`), which doesn't pause
  it, from a marker's start until a second before its end; an episode's
  credits bring up the next one, which plays after 10 seconds unless it is
  cancelled, also when the episode ends on its own.
- **Without the server** (switched off, or another network), every screen
  says "Can't reach <server>" with Try again and Choose a server. pley
  needs no internet: sign-in, artwork and playback all come from the
  server, and nothing else is ever asked.

## How fast

Measured on Kodi 21.3 for Windows, against a server on the same computer:

- A listing takes 20 to 45 ms (the first after Kodi starts about 240 ms):
  Kodi keeps the plugin's Python between calls (`reuselanguageinvoker`),
  where loading Python's modules again took about 550 ms of each.
- Play is ready in 14 to 47 ms: the server's answer and the item's
  details are asked for together, and the resume point comes with the
  listing. The file's type comes with it too, so Kodi doesn't ask first.
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
