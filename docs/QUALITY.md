# Playback quality

What every pley player offers in its Quality menu, and how Automatic
chooses. The web, Android (phones and TVs), Samsung and LG, Kodi and Roku
follow this page; each app's doc says only where it differs.

## The menu

In this order, the same on every app:

| id | Label | Video at most |
|---|---|---|
| `original` | Original | the file as it is |
| `auto` | Automatic | Original, lower only when it doesn't play well |
| `30m` | 30 Mbps 4K | 30 Mbps, 2160 lines |
| `15m` | 15 Mbps 4K | 15 Mbps, 2160 lines |
| `20m` | 20 Mbps 1080p | 20 Mbps, 1080 lines |
| `12m` | 12 Mbps 1080p | 12 Mbps, 1080 lines |
| `8m` | 8 Mbps 1080p | 8 Mbps, 1080 lines |
| `4m` | 4 Mbps 720p | 4 Mbps, 720 lines |
| `2m` | 2 Mbps 480p | 2 Mbps, 480 lines |

The two 4K qualities come first, then 1080p and below. As today, a file is
offered only the qualities that make it smaller (Original and Automatic
always). Automatic is the default for a player where no quality was ever
chosen; a quality chosen before, Original included, is kept.

While Automatic plays, the menu says what it plays: "Automatic · Original"
or "Automatic · 8 Mbps 1080p". The player's details say why when it went
down: "Lowered for the network" or "Lowered: the device fell behind".

## Original

The file as it is: direct play, or a remux when the container doesn't play
here, both chosen by the server (`mode: "auto"`). Only what can't play on
the device is converted (a codec it doesn't decode, a decoder that fails or
can't keep up), at the server's own quality for the file (`auto_quality`).
Original never lowers the quality because of the network: a slow network
waits.

## Automatic

Original, unless it doesn't play well; then the quality that does.

**It starts as Original**, with one exception: a network found too slow in
the last 30 minutes on this device and server (below). Then it starts at
the quality that network carried, if that is below the file's.

**It watches** while the video plays (not paused, not seeking):

- *Rebuffering*: the picture stops for want of data. A wait of 2 s or
  more, or two shorter ones within 60 s, is a play issue.
- *Delivery*: how many seconds of video arrive per second of time, over
  the last 20 s. Under 1.1 for 15 s with less than 10 s buffered ahead and
  falling is a play issue before it rebuffers.
- *Decoding*: the device dropping frames or a decoder failing, as each
  app counts them today, is a play issue too.

**On a play issue it steps down.** The new ceiling is the bitrate playing
now (the file's for Original, the quality's for a transcode) times the
delivery seen times 0.75, and never above what the server would choose for
the file (`auto_quality`'s quality, at the file's lines or fewer). It plays
the highest quality of the menu at or below that ceiling, and never below
2 Mbps 480p. A decoding issue at Original goes to the server's own quality
for the file, as today. The session starts again where it was
(`mode: "transcode"` with that quality's `max_bitrate` and `max_height`),
and the player says so once, briefly: "Playing at 8 Mbps 1080p for a
smoother picture."

**More issues step down again**, one quality at least, but not within 20 s
of the last change.

**It steps back up** after 3 minutes without a play issue, and not within
5 minutes of a step down, when the network measured during that time
carries 1.6 times the next quality up (or the file, for Original): one
step at a time, at most one every 3 minutes, silently. Players measure the
network from what they download: an HLS segment's size over the time it
took, or a direct play's bytes read while it fills its buffer. A player
that can't tell its network's speed stays where it went down to.

**It remembers** for 30 minutes, per device and server, the bitrate the
network last carried when it stepped down; the next title in Automatic
starts at most at that (Up Next included). A step back up to Original
forgets it.

Picking a quality in the menu, Automatic again included, starts from that
choice afresh.

## The server

Nothing changes on the server: Automatic asks for Original
(`mode: "auto"`) or for a quality (`mode: "transcode"`, `max_bitrate`,
`max_height`), as the menu's other choices do, and reads `auto_quality`
for what it would choose for the file.
