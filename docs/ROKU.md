# Pley for Roku

A native SceneGraph app for Roku players and Roku TVs, in `apps/roku`
(paths below are relative to it). It is not in Roku's channel store: it is
sideloaded in developer mode.

The app talks only to the Pley server, so it works without internet:
pictures come from the server and the fonts are the Roku's. The client ID
(`https://hhaidar.github.io/pley/oauth/roku.json`) is only a name the
server knows; nothing fetches it.

## Installing on a Roku

1. Turn on developer mode: on the Roku remote press Home three times, Up
   twice, then Right, Left, Right, Left, Right. Note the address shown,
   choose a password and let the Roku restart.
2. `bun run package` (in `apps/roku`) writes `out/pley-roku.zip`. It
   includes BrighterScript's runtime (`source/bslib.brs`, MIT), which bsc
   always adds; its notice is in `licenses/bslib.txt`.
3. Open the Roku's address in a browser, sign in as `rokudev` with that
   password, upload the zip and press Install. Pley appears on the Home
   screen as a developer app; installing again replaces it.

`bunx bsc --deploy --host <address> --password <password>` packages and
installs in one go. The debug console (`telnet <address> 8085`) shows what
the app prints and its errors.

The manifest asks for SceneGraph 1.2 (every Roku OS from 9). Roku's store
certification asks for 1.3 from October 2026, which needs Roku OS 15; the
app uses nothing of 1.3, so it is a one-line change for the store.

## What it does

It finds Pley servers on the local network (or takes an address), signs in
with a code shown on the TV and approved on the server's `/link` page (or
its QR code), and asks "Who's watching?" at start when the household has
more than one profile.

- **Home's rows**; each library with the web app's tabs (Recommended,
  Library sorted and filtered, Collections).
- **A title's page:** Resume, Play from start, a show's next episode and
  episodes, Mark as watched, My List, Go to show, and rows of seasons, cast
  and crew, extras, More like this and collections; people's and
  collections' pages.
- **Music:** artists, albums and tracks, in a player that keeps playing
  while the app is used. Now Playing has Previous, Pause, Next and Stop
  (the remote's play, fast forward and rewind too); Home's menu shows Now
  playing while music plays.
- **Live TV**, in Home's menu after Home while the server has channels
  this profile may watch ([below](#live-tv)).
- **Playlists:** Play, Shuffle, Rename, Delete, remove an entry.
- **Search** (titles and people, and the Live TV channels found as a row
  above them: OK watches one, up and down going through what was found).
- **Settings:** audio and subtitle defaults, Quality, Play the next episode
  by itself, Watch history, Switch profile, Sort libraries (OK lifts a
  library, up and down move it, OK drops it, Back cancels; "Use the
  server's order" resets), Switch server, Sign out.
- **The Options key (\*)** on a title: Mark as watched or unwatched, Add to
  or Remove from My List, Add to playlist (or a new one), Remove from
  Continue Watching, Go to show. In Watch history it removes a play; on a
  playlist entry it removes the entry (not in a smart playlist).
- **The player** plays files as they are when the Roku can decode them,
  HLS otherwise. It offers Skip intro and Skip credits, Up Next at an
  episode's credits (or a playlist's next entry), and Pley's menu.

Not offered: playback speed and audio delay, which the Video node lacks.
Subtitles look as the Roku's caption settings say (Settings → Accessibility
→ Captions style); the app never sets `captionStyle`, which would override
them for the whole Roku.

## Playback

### Pley's menu

Down while a title plays opens it at the right, over the video, which
plays on. Back goes back in it, then closes it. Subtitles and audio stay in
Roku's own options (the Options key). The menu has:

- **Quality** ("Automatic · Original"; [below](#quality)).
- **Subtitle sync**, for a file with text subtitles (below).
- **Chapters**, for a file that has them.
- **Find subtitles…** (below).
- **Playback details:** how it plays ("Direct play", "Direct stream",
  "Transcoding") and why, video and audio in and out (`GET
  /api/playback/{session}`), bitrate, HDR, burned-in subtitles, and how far
  ahead the conversion is.

### Errors

A stream stopped by an administrator (`410`) leaves the player and says
why; any other break restarts where it was with the current token. A file
that fails before its first picture, or with a media error later, plays
again converted at the server's quality, once.

### Subtitles and audio

- **Text subtitles** go to the Video node as WebVTT. **Picture subtitles**
  (PGS, VobSub, DVB) are drawn onto the video by the server
  (`subtitle_index`).
- **Captions:** Roku shows a subtitle track only with captions on, a
  Roku-wide setting. When Pley picks a track it turns captions on for that
  playback and restores them after, unless the person changed them.
- **Track names:** a subtitle track's title, else its language (one of
  34), "(Forced)" added if the title doesn't say it.
- **Subtitle sync:** left and right (or rewind and fast forward) move
  subtitles 0.1 s, up to 10 s either way, shown as "+0.3 s"; Reset, Done,
  Back. The server shifts the cues (`offset_ms` on the track's address,
  [API.md](API.md)) and the stream reopens where it was. The offset is
  kept per file per server (registry section `subtitleSync`, the latest 50
  files).
- **Find subtitles…** (`components/lib/SubtitleSearch.brs`): OpenSubtitles
  results in the profile's subtitle language (`GET
  /api/items/{id}/subtitles/search?file_id=`; English without one), those
  made for this file first, with the name, "Made for this file", "Hearing
  impaired", "Forced" and downloads for the one in focus. OK adds it on
  the server (`POST /api/items/{id}/subtitles`), saves it as the pick, and
  shows it ("Subtitles added: English"). Without OpenSubtitles set up
  (`503`, `setup: false`) the menu says an administrator sets it up on the
  web in Settings → Server; an older server (`404`) needs a newer Pley. To
  search another language, change the profile's subtitle language.
- **Audio:** the server picks the track this Roku plays best in the
  profile's language (the show's or movie's saved choice, else the
  defaults). Any audio but the file's default makes the file a stream with
  its video copied.
- **Picks in Roku's options** (captions on or off, a subtitle track, an
  audio track of a file played as it is) are saved for the show or movie
  (`PUT /api/items/{id}/streams`, `null` subtitles for off), so the next
  episode starts with matching tracks. What shows in the first 2 s, or
  after subtitle sync reopens the stream, isn't saved. Subtitles changed
  alone are saved without the audio. Roku's audio tracks are matched to
  the file's only when the count and languages agree.
- **Defaults** (Settings): Audio language (the file's default or one of 34),
  Subtitles (Off, Only for foreign dialogue, When the audio is in another
  language, Always) and Subtitle language, saved at once (`PUT
  /api/me/track-defaults`). A server that refuses (`403`, `404`, `405`)
  restores the choice and points to the server's `/account` page.

### Quality

The menu, Original and Automatic follow [QUALITY.md](QUALITY.md#the-menu);
the code is `components/lib/AutoQuality.brs`. Roku specifics:

- **Settings → Quality** is kept on the Roku (registry section
  `playback`); Automatic unless another was chosen. One chosen in the
  player's menu becomes the setting, and the title reopens at it.
- **Several sizes:** in an adaptive session (`PlaybackInfo.adaptive`,
  [QUALITY.md](QUALITY.md#several-sizes-adaptive-bitrate)) the Video node
  moves between sizes itself. The original, remuxes and small
  conversions come in one size, so the app steps down.
- **Signals:** a wait is the Video node's `buffering` after at least 2 s
  of playing, not after a seek or pause, and only when
  `bufferingStatus.isUnderrun` says it is for want of data. Roku reports
  neither read-ahead nor dropped frames. Network speed is the HLS
  segments' sizes over their download times in the last 20 s
  (`downloadedSegment`); without segments, half the bitrate playing.

Roku follows the rules for
[players that go black between streams](QUALITY.md#players-that-go-black-between-streams):

- **A conversion whose bitrate can change** (`PlaybackInfo.live_bitrate`)
  is lowered in place (`PUT /api/playback/{session}/bitrate`): "Quality
  lowered to 4 Mbps 1080p" shows for 4 s; the menu says "Automatic · 4
  Mbps 1080p". A refusal leaves the stream as one whose bitrate can't
  change.
- **The original, or a fixed-bitrate conversion,** moves only after a wait
  of 2 s or more, to a new session at least one quality lower, never above
  the server's quality for the file (`auto_quality`): a 4K file goes 30
  Mbps 4K, 15 Mbps 4K, then 12 Mbps 1080p. "Quality lowered to 8 Mbps
  1080p" shows once it plays.
- **Memory:** each ceiling is kept 30 minutes per server in the registry
  and told to the server.
- **Back up:** through the menu's bitrates, then the stream's own;
  otherwise the next title starts a quality higher.

**Where Automatic starts:** at a title's first opening in Automatic the app
asks the server's network memory
([API.md](API.md#network-memory-and-speed-test), 1.5 s at most). If the
server asks for a speed test, the app fetches 8 MiB of `GET
/api/playback/speedtest` to `tmp:` and times it, giving up after about 3 s;
under 256 KiB measures nothing and the original starts. The answer is kept
10 minutes per server. Original and fixed qualities ask nothing.

### Skip markers and Up Next

Skip intro and Skip credits show from a marker's start until a second
before its end. An episode's credits bring up Up Next instead; it counts
down only while Settings → Play the next episode by itself is on
(otherwise it waits for Play now). While the button shows: OK skips, play
pauses, fast forward and rewind move 10 s, Back leaves the player. During
the Up Next countdown, Back, play, fast forward and rewind cancel it.

## Live TV

The web app's Live TV ([API.md](API.md#live-tv)), for watching, laid out
as TiviMate does; sources, the lineup and the settings stay in the web
app. The code is `components/screens/LiveTvScreen`, `LivePlayerScreen`,
`components/lib/Live.brs` and the API task's `live…` operations.

A provider's lineup runs to tens of thousands of channels in hundreds of
groups, so nothing asks for all of them: the categories come from `GET
/api/live/groups`, channels 50 at a time from `GET /api/live/browse`, and
programmes only for those 50 (`GET /api/live/guide?ids=`).

- **Categories** down the left, with their channel counts, under Search
  pinned above them (it never scrolls away): Favourites, Recently
  watched, All channels, then "Regions" in
  alphabetical order (a region with more than one group, "UK · 12
  groups": OK opens it to its groups, closing the one open) and "More
  groups", the rest in the provider's order (a region of one group with
  them). With fewer than two regions, the groups are one plain list.
  OK on a category shows its channels and goes to them; Back or Left
  goes back to the categories. The category and the tab are kept on the
  Roku per server and profile (registry section `liveViews`); a group
  gone, or favourites none any more, shows every channel. Rewind and fast
  forward in the categories move a screen (13 rows) up or down.
- **Search** (OK on it, or the **Replay** key from anywhere in Live TV:
  Roku remotes have no search key for apps) opens Roku's keyboard. It
  looks through every channel (`browse?category=all&q=`: a name, a
  number's beginning, or the title on now), shown as "Results for …" in
  the guide or the cards; the groups whose names match head the
  categories as links. Clear goes back to every channel. A search isn't
  kept for the next visit.
- **Guide** (the tab above the categories): Roku's TimeGrid, a page of
  50 channels down and two hours across, the programme in focus
  described above it (On now, its times and time left, episode, Live and
  New, what it is). Past the page's last row (or before its first) comes
  the next page (or the one before), round the ends of the category.
  Programmes come six hours at a time as the guide is scrolled, two days
  at most, from half an hour before the current half hour. Rewind and
  fast forward move two hours earlier or later, Play goes back to now
  (the TimeGrid's own trick-play keys are off, so Replay can open
  Search). A channel
  without a guide shows its name across it. OK on a programme on now (or
  on time without a guide) watches the channel; on another, it says what
  it is, with Watch and the favourite.
- **Channels** (the other tab): the category's channels as cards (logo,
  number, a star for a favourite, what's on with how far along, what's
  next), 50 more as the focus nears the end.
- **Channel names:** in a group, without the region the group already
  says ("Sky Sports 1" in UK · Sport).
- **The Options key** on a programme or a card: Watch, Add to or Remove
  from favourites (`PUT`/`DELETE …/favorite`).
- **Back from the player** lands on the channel watched: the page around
  it (`browse?around=`), its row in focus.

### The live player

- **Tuning:** `POST …/watch` with the player's `client_id` (the one
  playback uses), so the server ends the channel watched before; then the
  playlist once from the API task, which the server answers once the
  channel's first segments are made (50 s at most) or with why not; then
  the Video node, `Live = true`, the token in its agent. The channel's
  logo shows meanwhile, the spinner centred under it, then its name and
  what's on.
- **Up and down** change channel (Up: the next in the list) within the
  category the player was opened from, the page around the channel
  (`browse?around=`), the next or last page at its ends, round the ends
  of the category; Recently watched keeps the order it had as the player
  opened. The channel's logo and name show at once and it is tuned 0.7 s
  after the last press.
- **Left** opens the category's channels (number, logo, name, what's
  on), the page around the channel playing; up or down past its ends
  brings the page before or after. The Options key in it adds to or
  takes out of the favourites.
- **Right**, **OK** or the **Options key** (\*) show the banner: the
  channel, what's on with its times, time left and progress, what's
  next, and the Live chip (a plain badge). It never shows by itself as a
  channel starts or changes (the tuning card says which channel it is);
  asked for, it goes after 5 s.
- **Live is live:** no pause, rewind or skip. The Video node's own
  controls are off (`enableUI` false), and Play, Rewind, Fast forward and
  Replay do nothing in the player. A wait for data shows a spinner once
  it has lasted 0.7 s.
- **All connections in use** (`409`): the server's message, then each
  connection's channel and who holds it ("You, on another device", the
  names to administrators, or "Someone else" / "3 people"). Where the
  server says it may be taken over (`can_take_over`), **Watch here
  instead** (naming the channel when there are several) asks again with
  `take_over`; then Try again, Other channels and Back.
- **Troubles:** a channel gone (`404`), Live TV closed (`403`), a
  connection taken over by someone else or a channel stopped by an
  administrator (`410`, the server's message saying which) and one not
  playing (`503`) say so, with what can be done. A stream that breaks is checked with its playlist and tuned
  again, three times a minute at most.
- **Leaving** sends `DELETE /api/live/sessions/{session}`, as does
  changing channel.
- **Radio:** a channel with sound and no picture (the Video node's
  `videoFormat` empty 3 s into playing) keeps its card with what's on.

## How the app works

| Path | What |
|---|---|
| `source/main.brs` | Starts the scene |
| `components/MainScene.*` | The screen stack, the HTTP agent for pictures, sign-in when the server ends it |
| `components/MusicPlayer.*` | The music queue on the scene's Audio node (`m.global.music`) |
| `components/screens/` | One component per screen |
| `components/shared/` | Screen helpers (`Ui.brs`), title actions (`Titles.brs`) |
| `components/items/` | Tiles, rows, the player's skip button |
| `components/tasks/ApiTask` | The only code that talks to the server |
| `components/tasks/DiscoveryTask` | `pley?` to UDP port 7539 (`apps/server/src/discovery.rs`) |
| `components/tasks/EventsTask` | The server's event stream, on a socket |
| `components/lib/` | Plain BrightScript (no SceneGraph): the logic the unit tests cover |
| `tests/` | Unit tests of `components/lib` |
| `scripts/` | Test runner, smoke run, preview, bench, images |
| `licenses/` | Notices for bundled third-party code |

Rules the code keeps:

- **All HTTP goes through `ApiTask`**; the render thread never makes a
  request. Lists are built into ContentNodes in the task.
- **The access token** goes only in `Authorization: Bearer`, never in an
  address, including the HTTP agents of pictures and the Video node.
- **Refreshes** run one at a time per server, and the new token pair is
  saved before use (a refresh token works once). A refresh is waited for
  up to two minutes; if the person signed out or signed in again
  meanwhile, its pair is revoked. A token refused just after a refresh, or
  expired with nothing to refresh it, ends the sign-in.
- **User-Agent:** `pleyRoku/<version> (Roku OS <version>; <model>)`; the
  server's `/link` page shows it.
- **"Who's watching?"** uses `GET /api/oauth/profiles` (the count kept in
  registry section `household`) and the switch-profile grant
  ([API.md](API.md)), with Roku's PIN pad for a PIN. A profile that needs
  its password signs in with a code instead. A sign-in made before the app
  asked for `profiles` says to sign in again.
- **Scopes:** `library play playlists profiles`; an older sign-in that
  gets `403` on a playlist change is told to sign in again.
- **Unreachable server:** "Can't reach …" with Try again and Switch server.
- **Event stream** (`GET /api/events`, http:// servers only, since Roku's
  sockets have no TLS): read on a socket by `EventsTask`
  (`components/lib/Events.brs`), reconnecting from 2 s up to a minute.
  Watch state, My List and library changes refresh Home; a stream stopped
  from elsewhere ends at once.
- **Home's rows** refresh after playback, when a minute old, after an
  event, or from Home's menu; only rows whose JSON changed are rebuilt,
  keeping scroll and focus.
- **Local changes** (watched, My List, Continue Watching) are noted on the
  scene (`uiNoteChange`) so covered screens update without refetching.
- **Library views** (tab, sort, filters) are kept per server, profile and
  library (registry section `libraryViews`). Filters come from `GET
  /api/libraries/{id}/filters`.
- **Sort libraries** saves each move (`PUT /api/me/libraries/order`); an
  older server says sorting needs a newer Pley.
- **Music:** each track is its own session (`POST /api/playback`) on the
  Audio node; one that won't play as it is plays converted once, else is
  skipped. A video, another profile, sign-in or server stops the music.
- **Launch beacons:** `AppLaunchComplete` once the first screen is usable;
  `AppDialogInitiate` / `AppDialogComplete` around a sign-in at launch.

## Checking it on a computer

In `apps/roku`, with Bun and Node 22 or newer. Bun runs the scripts but
the tools (bsc, bslint) run on Node: bslint's plugin loader needs Node's
`module.require`, so `apps/roku` has no `bunfig.toml`.

```powershell
bun install
bun run check     # bsc's compile check, bslint, the unit tests
bun run smoke     # the whole app in brs-node, against a fake server
bun run bench     # the list work timed in brs-node
bun run preview   # the app in a browser, the keyboard as its remote
bun run images    # the nine-patch focus and watched-mark images
```

- **Preview** serves the app on http://127.0.0.1:8086/. Arrows, Enter
  (OK), Esc or Backspace (Back), Space (play/pause), `[` and `]` (rewind,
  fast forward) and `*` (options) are the remote. It uses the smoke run's
  fake server, which approves the code itself; `--server
  http://<address>:7539` uses a real one (approve the code on its `/link`
  page); `--port` changes the port.
- **Unit tests** run `components/lib` and `tests/*.test.brs` in brs-node;
  each `sub test_…()` is a test.
- **Smoke run** starts a fake server (`scripts/fixture-server.mjs`) and
  walks every screen with the remote, saving each in `out/smoke/`. It
  fails on a BrightScript error, a request without the User-Agent, a token
  in an address or sent for a QR code, a refresh token used twice, or two
  refreshes (or a refresh and a profile switch) at once.
- **brs-node limits:** its own fonts, no video (Skip intro and Up Next
  have only unit tests), one fetch at a time, the odd dropped key, and no
  measure of a Roku's speed.
- **Images:** `bun run images` runs `scripts/make_images.py` (Python
  standard library). The home screen icons and splash screens come from
  `bun scripts/render-icons.mjs` at the repository root.

## Not yet

- Playback speed and audio delay; another language in Find
  subtitles….
- Live updates from an https server: Home refreshes once a minute.
- Audio other than the file's default played as it is.
- The Show screen refetches its episodes each time it shows.
- Seek previews (BIF files); rating titles; reordering a
  playlist; "Always start as"; an editable music queue.
- Roku's channel store: a developer account, certification, SceneGraph
  1.3 and the launch beacons' order.
