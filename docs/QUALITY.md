# Playback quality

What every Pley player offers in its Quality menu, and how Automatic
chooses. The web app, the Android app (phones and TVs), the Samsung and LG
apps, Pley for Kodi and Pley for Roku all follow this page; each app's doc
covers only where it differs.

## The menu

The same on every app, in this order:

| id | Label | Video at most |
|---|---|---|
| `original` | Original | The file as it is |
| `auto` | Automatic | Adjusts to conditions; a transcode in several sizes the player moves between |
| `30m` | 30 Mbps 4K | 30 Mbps, 2160 lines |
| `15m` | 15 Mbps 4K | 15 Mbps, 2160 lines |
| `20m` | 20 Mbps 1080p | 20 Mbps, 1080 lines |
| `12m` | 12 Mbps 1080p | 12 Mbps, 1080 lines |
| `8m` | 8 Mbps 1080p | 8 Mbps, 1080 lines |
| `4m` | 4 Mbps 720p | 4 Mbps, 720 lines |
| `2m` | 2 Mbps 480p | 2 Mbps, 480 lines |

When the player knows the file:

- Original and Automatic are always offered. Other qualities are offered
  only if they make the file smaller: fewer lines, or at most 80 % of its
  bitrate.
- A quality taller than the file takes the file's lines in its name: a
  720p file at 12 Mbps is offered "8 Mbps 720p", not "8 Mbps 1080p".
- Two qualities that come to the same bitrate and lines show once. The menu
  is then sorted by lines, then bitrate, so a 1920×800 film doesn't list
  "15 Mbps 1080p" above "20 Mbps 1080p". A 4K file keeps the table's order.
- Automatic's steps follow the menu the file is offered.

A quality chosen with no file in sight (the default quality in Settings,
Kodi's and Roku's settings) keeps the table's names. Automatic is the
default until a quality is chosen; after that the chosen one (Original
included) is kept.

What the menu shows:

- While Automatic plays, its label says what plays: "Automatic · Original"
  or "Automatic · 8 Mbps 1080p".
- Under its name, Original shows the file's size and bitrate ("1080p · 8.2
  Mbps"), and Automatic what it plays now ("Now 8 Mbps 1080p · Lowered for
  your connection"). Other qualities have no second line.
- The menu doesn't say how a choice plays: an Original the server converts
  still reads "Original", and burned-in picture subtitles carry no mark.
  The player's details say that: how it plays, the server's reasons, what
  goes in and out, and why Automatic went down ("Lowered for your
  connection" or "Lowered for this device").

## Original

The file as it is: direct play, or a remux when the container doesn't play
on the device, as the server chooses (`mode: "auto"`). Only what the device
can't play is converted (a codec it doesn't decode, a decoder that fails or
can't keep up), at the server's own quality for the file (`auto_quality`).
Original never lowers the quality for the network: a slow network waits.

## Automatic

Original, unless it doesn't play well; then the highest quality that does.

### Starting

Before playing, the player asks the server what it knows of the current
network ([memory](#memory)). If that network is slower than the file, it
starts at the highest quality at or below what the network carries, never
below 2 Mbps 480p, and says "Lowered for your connection". Otherwise it
starts as Original.

On an unknown network away from home, it first runs a short speed test (at
most 2 s of data from the server, during the usual loading). It starts at
70 % of the measured speed, or as Original if the whole test arrived in
time. There is never a test on the server's own network.

### Watching

While the video plays (not paused, not seeking), each of these is a *play
issue*:

| Signal | Play issue when |
|---|---|
| Rebuffering | A wait for data of 2 s or more, or two shorter waits within 60 s |
| Delivery (seconds of video arriving per second, over the last 20 s) | Under 1.1 for 15 s, with less than 10 s buffered and falling |
| Decoding | Dropped frames or a failing decoder, as each app counts them |

Playing the Original as the file is (direct play or a remux), the network
counts only once the picture has stopped: a wait for data of 2 s or more.
Two short waits and slow delivery don't. A player keeps only seconds of a
big file loaded ahead (a browser about 10 s or 50 MB of it), so they say
little of the network, and a transcode would play a worse picture for the
rest of the title. An Original the server converts (the device can't play
the file) counts them all, as any quality does.

Even that wait counts only when the network is to blame for it:

- **Measured network.** If what the player measured over the last 2 minutes
  carries 1.5 times the file's bitrate or more, the server is why the
  picture waits: a slow disk, a remux catching up, the server busy. A
  transcode reads the same file and does more work, so it would only wait
  longer. The Original plays on.
- **Below that**, the wait counts. The measurement is taken again as the
  wait goes on and when it ends, so a slow download that arrives during
  the wait counts too (not on Samsung and LG, whose figures come only
  while the video plays).
- **Nothing measured.** Where the player measured nothing, the second such
  wait within 5 minutes counts.
- **Data loaded ahead** (web app): a wait that began with 1 s or more loaded
  ahead is the browser's decoder, not the network.
- **A conversion the server falls behind on.** The Android app's warning
  that the server converts slower than the video plays doesn't count at
  the Original either: a remux reads as much of the file as a transcode
  would, and does less.

Roku measures the network only from HLS segments, and Kodi from whole
answers through the add-on's proxy, so a direct play there is one that
nothing measured.

### Stepping down

On a play issue, the new ceiling is:

> bitrate playing now (the file's for Original, the quality's for a
> transcode) × delivery seen × 0.75

capped at what the server would choose for the file (`auto_quality`, at the
file's lines or fewer). Automatic plays the highest quality at or below the
ceiling, never below 2 Mbps 480p. A decoding issue at Original goes to the
server's own quality for the file.

The session restarts where it was (`mode: "transcode"` with the quality's
`max_bitrate` and `max_height`), and the player says so once: "Quality
lowered to 8 Mbps 1080p".

- The web app and the Android app prepare the new session while the old one
  plays (`keep_previous`), request its first segment, and switch on the old
  one's last picture with no spinner. They switch the same way for a
  quality, audio or picture-subtitle change picked in their menus.
- The Samsung and LG apps do the same, requesting the segment with `HEAD`
  ([TV_PLATFORMS.md](TV_PLATFORMS.md#quality)). Their players can't hold the last
  picture, so the screen is black while the TV's player opens the new
  stream, and their loading ring waits 2.5 s.

Further play issues step down again, at least one quality, but not within
20 s of the last change.

### Stepping up

After 3 minutes without a play issue, and not within 5 minutes of a step
down, Automatic steps up one quality if the network measured over that time
carries 1.6 times the next quality up (or the file, for Original). At most
one step every 3 minutes, silently.

A title that started lower for what was remembered of the network
([Starting](#starting)) needs only 20 s without a play issue (web app and
Android app). It then goes in one step to the highest quality the network
carries 1.6 times, the Original included: the memory may come from a
slower moment, or from a step down the server caused. A play issue or a
step first, and the usual rules apply.

Players measure the network from what they download: an HLS segment's size
over the time it took from its first byte (the server sends a segment once
it is made, so the wait before is the transcoder's, not the network's), or
a direct play's bytes read while filling its buffer. A player that can't
measure stays where it went down to.

### Settling

A step up that a step down undoes within 10 minutes was one too far:

- Automatic doesn't step up to that quality again during the title.
- The calm it waits for before any other step up doubles each time (3
  minutes, then 6, then 12).
- The network memory keeps the settled step, so the next title starts
  there.

Once it finds the highest quality that plays smoothly, it stays.

### Memory

The server remembers what a network carries, per profile, device and
network:

| What | Kept for |
|---|---|
| The bitrate of the last step | 24 hours |
| A speed test result | 7 days |

The next title in Automatic (Up Next included), or the same device back on
that network later, starts at most at that bitrate. A step down or up to a
quality updates it. A step back up to Original, or picking Automatic in the
menu, clears it, and no speed test runs on that network for 7 days. The
server stores a keyed hash of the network address, never the address.

If the server can't be reached for this, the device keeps its own memory:
for 30 minutes, per device and server, the bitrate the network carried at
the last step down.

Picking a quality in the menu, Automatic included, starts afresh from that
choice.

## Players that go black between streams

Some players can't switch streams without a moment of black screen:
Samsung's AVPlay and Kodi's player close one stream before opening the
next, and Roku's Video node replaces its content the same way. LG's player
is treated the same until a TV shows otherwise.

On these players Automatic never switches streams while one plays well.
It changes the stream as it is sent instead:

- **Start**: as above, from the speed test or the network memory.
- **A conversion changes its bitrate, not the stream.** The player asks the
  server (`PUT /api/playback/{id}/bitrate`), and the transcoder makes the
  segments not yet fetched at the new bitrate, same size, same playlist, so
  playback goes on without a pause. The rules above decide when (a play
  issue steps down, calm minutes step up, it settles).
- **A stream keeps its size** (4K stays 4K, 1080p stays 1080p), with a
  bitrate floor: 8 Mbps for 4K, 3 Mbps for 1080p, 1.5 Mbps for 720p, 1 Mbps
  below. Under the floor, the network waits.
- **Steps**: on Samsung and LG a step down goes to the ceiling above in
  whole Mbps ("Automatic · 4 Mbps 1080p"); a step up goes to the next of
  the file's qualities' bitrates, then the stream's own. Kodi and Roku step
  down the same way, and up through the menu's bitrates (2, 4, 8, 12, 15,
  20, 30 Mbps), then the stream's own. None goes above the stream's own
  bitrate.
- **A stream that can't go higher** without a new stream (already at its own
  bitrate, or one whose bitrate can't change) stays while it plays well. On
  Kodi and Roku, when the calm minutes find the network carrying 1.6 times
  the next quality up, the network memory goes up a quality instead, so the
  next title starts there; at the original it is cleared.
- **Playing the original** (direct play or remux), it moves to a conversion
  only once the picture has stopped anyway, as on every player ([Watching](#watching)):
  a wait for data of 2 s or more the network is to blame for, or a failing
  decoder. Dropped frames
  move it only in a title's first minute (a sign the device can't keep up
  with the file); later they lower where the next title starts. On Kodi and
  Roku, a conversion whose bitrate can't change (an older server or
  transcoder) moves the same way, short waits and slow delivery not
  counting.
- **One size per stream on Samsung and LG** (they don't send
  `caps.hls.variants`): a size change inside a stream may blank their
  screens too. Roku's Video node switches between the sizes of one stream
  without a gap, so Roku keeps them.

The web app and the Android app switch streams on the last picture (see
[Stepping down](#stepping-down)), so the earlier rules apply to them.

## Several sizes (adaptive bitrate)

For a player that switches between sizes by itself (it sends
`caps.hls.variants`: hls.js, the browsers' own HLS, Media3, Roku's Video
node), a transcode comes in several sizes. Kodi's player and the Samsung
and LG apps don't, and step as described above.

- The session's `master.m3u8` lists the transcode's own size first. A
  quality with more bits than its size usually gets (20 Mbps 1080p, 30 Mbps
  4K) comes at that size again with the usual bits (8 Mbps for 1080p, 20
  for 4K). Then come 1080p, 720p, 540p and 360p below it, at 8, 4, 2 and 0.7
  Mbps, never more than the size above. One of these with more than two
  thirds of the bits of the one above is left out.
- A picked quality is the top size; nothing goes higher.
- Every size is cut into the same segments, each starting with a keyframe at
  the same time, so the player can switch at any segment without a pause, a
  new session or a sound glitch.
- The transcoder makes only the size the player requests. The others stop
  when it switches; what they made is kept in case it comes back. A size the
  transcoder can't make in real time arrives slower than it plays, and the
  player moves down as it would for a slow network.
- The players' own ABR moves up only with room to spare, so it doesn't
  swing between two sizes: hls.js counts half the measured bandwidth for a
  move up (`abrBandWidthUpFactor` 0.5), and Media3 waits for 20 s of buffer
  (`minDurationForQualityIncreaseMs`).

While a session has several sizes (`PlaybackInfo.adaptive`), the player's
ABR moves between them and Automatic watches only for what sizes can't
solve: dropped frames, and a stall long enough to ask the server. Both
still restart the session lower. The network's other play issues only
start the calm over: in the web app and the Android app, Automatic steps
back up from such a session as [above](#stepping-up), to the next quality
or to the Original, so a step down from the Original, which is what creates
such a session, doesn't last the title.

The web app's Automatic plays no size bigger than the player is on screen;
a picked quality plays its own size whenever the connection allows. The
menu shows the size playing ("Automatic · 720p"), and the dashboard which
of the sizes ("size 2 of 4").

## The server

| Player asks for | Request |
|---|---|
| Original | `mode: "auto"` |
| A quality | `mode: "transcode"` with `max_bitrate` and `max_height` |

A quality is converted at its bitrate, less 192 kbps for the sound. It
never goes above half again the file's own bitrate, nor above the highest
quality of its lines (an app of the compatibility API asking for far more
gets that). A file with fewer bits still gets the lowest quality
of its lines, as the server's own quality does.

The response's `auto_quality` says what the server would choose for the
file, and `adaptive` whether the stream comes in several sizes. The network
memory is read with `GET /api/playback/network`, the speed test is
`GET /api/playback/speedtest`, and both are reported with
`PUT /api/playback/network` ([API.md](API.md), "Network memory and speed
test").
