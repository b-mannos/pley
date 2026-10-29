# pley-transcoder

`pley-transcoder` is the part of Pley that runs FFmpeg. It:

- probes media files
- transcodes and remuxes video into HLS, and converts whole files
- tone maps HDR10, HLG and Dolby Vision to SDR, or keeps HDR in HEVC Main 10
- burns in and extracts subtitles
- makes thumbnails, seek previews and embedded cover art
- indexes keyframes

It is a separate program. The Pley server runs
it as a child process, or connects to one running elsewhere, and talks to it
only through the HTTP API described here. The request and response types live
in the `pley-transcoder-api` crate (`packages/transcoder-api`).

## Running

```
pley-transcoder [OPTIONS]
```

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `--listen <ADDR>` | `PLEY_TRANSCODER_LISTEN` | `127.0.0.1:7540` | Address to listen on. Port 0 picks a free port. |
| `--token <TOKEN>` | `PLEY_TRANSCODER_TOKEN` | none | Shared secret, at least 16 characters. Required unless `--listen` is a loopback address. Prefer the env var: every account can read a command line in the process list. |
| `--work-dir <DIR>` | `PLEY_TRANSCODER_WORK_DIR` | `pley-transcoder` in the user's cache folder | Segments and caches (see [Work folder](#work-folder)). |
| `--ffmpeg <PATH>` | `PLEY_FFMPEG` | `ffmpeg` from `PATH` | FFmpeg binary. |
| `--ffprobe <PATH>` | `PLEY_FFPROBE` | `ffprobe` from `PATH` | FFprobe binary. |
| `--max-jobs <N>` | `PLEY_TRANSCODER_MAX_JOBS` | 8 | Concurrent FFmpeg transcodes (transcode slots). |
| `--max-hw-jobs <N>` | `PLEY_TRANSCODER_MAX_HW_JOBS` | 8 | Concurrent hardware encoder sessions. Extra jobs fall back to software encoding. |
| `--probe-concurrency <N>` | – | 8 | Concurrent probes and extractions. |
| `--announce` | – | off | Print `{"event":"listening","addr":"…"}` on stdout once ready. |
| `--exit-with-parent` | – | off | Exit when stdin closes (managed mode). |
| `--redetect` | – | off | Ignore the cached capability detection. |
| `--log <FILTER>` | `PLEY_LOG` | `info` | Log filter, e.g. `info` or `debug`. |
| `--license` | – | – | Print the license notice and exit. |

Other environment variables:

- `PLEY_VAAPI_DEVICE`: the VAAPI render node to use (default: the first Intel
  or AMD one).

### Managed and external mode

- **Managed (default).** The server starts
  `pley-transcoder --listen 127.0.0.1:0 --announce --exit-with-parent` from
  the folder that holds the server binary. It passes a random token in the
  environment, reads the announced address, and restarts the transcoder with
  backoff if it dies. The transcoder exits as soon as the server does.
- **External.** Start the transcoder yourself (for example on a machine with
  a GPU) with its token in `PLEY_TRANSCODER_TOKEN`. Point the server at it
  with `transcoder_url = "http://host:7540"` and the same `transcoder_token`
  (or `PLEY_TRANSCODER_TOKEN` in the server's environment). Media paths must
  be identical on both machines, for example through the same network mount.

### Work folder

Default: `%LOCALAPPDATA%\pley-transcoder` on Windows,
`~/Library/Caches/pley-transcoder` on macOS, `$XDG_CACHE_HOME/pley-transcoder`
or `~/.cache/pley-transcoder` on Linux.

| Path | Holds | Pruned |
|---|---|---|
| `jobs/<id>/` | a job's segments on disk | see [Housekeeping](#housekeeping) |
| `conversions/` | converted files | see [Conversions](#conversions) |
| `keyframes/` | keyframe indexes | unused 90 days, then past 256 MB |
| `subtitles/` | converted and extracted subtitles, fonts attached to files | unused 60 days, then past 512 MB |
| `trickplay/` | seek-preview sheets | unused 1 day, then past 1 GB |
| `libplacebo/` | libplacebo's compiled shaders | unused 30 days, then past 128 MB |
| `capabilities.json` | the cached encoder and filter detection | see [Encoders](#encoders) |

- The folders the transcoder keeps here are private to its user. So is the
  work folder itself when the transcoder creates it or it is the default. A
  folder that already exists (such as `/tmp`) keeps its permissions.
- The caches are pruned an hour apart, the first time two minutes after
  start-up. Entries unused past their age limit go first, then the least
  recently used until the folder fits its budget. Reading an entry counts as
  using it.
- Temporaries left by a crash go after 3 hours untouched. A subtitle pass that
  runs longer touches its outputs every 10 minutes.
- A set of seek-preview sheets is also deleted as soon as every sheet in it has
  been fetched: the server stores the sheets it fetches.

### Authentication

- With a token, every endpoint except `GET /health` requires
  `Authorization: Bearer <token>`.
- Without a token (loopback addresses only), the transcoder answers only
  requests whose `Host` is `localhost`, `127.0.0.1` or `[::1]`, with any port.
  This refuses DNS rebinding: a web page whose own name points at 127.0.0.1
  sends that name.
- Other accounts on the same computer can still reach a transcoder without a
  token. Give it one where that matters.

### Errors

Errors are `{"code": "...", "message": "..."}`:

| Status | Code | Meaning |
|---|---|---|
| 400 | `bad_request` | invalid request |
| 401 | `unauthorized` | missing or wrong token |
| 403 | `forbidden` | no token, and the request is addressed to another host than this computer |
| 404 | `job_not_found` | unknown job (expired, deleted, or the transcoder restarted) |
| 404 | `not_found` | unknown file, or segment out of range |
| 500 | `ffmpeg_failed` | ffmpeg or ffprobe could not process the file |
| 500 | `internal` | anything else (e.g. ffprobe could not be started) |
| 503 | `busy` | every transcode slot is in use |
| 504 | `timeout` | the operation took too long |

## Endpoints

| Method | Path | What |
|---|---|---|
| GET | `/health` | `ok`. Needs no token. |
| GET | `/v1/info` | `TranscoderInfo`: version, capabilities, limits |
| GET | `/v1/status` | `TranscoderStatus`: counters, processes, jobs (see [Metrics](#metrics)) |
| GET | `/metrics` | Prometheus text format (see [Metrics](#metrics)) |
| POST | `/v1/probe` | `MediaInfo` of a file |
| POST | `/v1/thumbnail` | one frame, `image/jpeg` |
| POST | `/v1/cover` | embedded cover art |
| POST | `/v1/trickplay` | make seek-bar preview sheets |
| GET | `/v1/trickplay/{id}/{n}.jpg` | preview sheet `n` |
| POST | `/v1/subtitles` | a text subtitle track as `text/vtt` |
| POST | `/v1/subtitles/mks` | a subtitle track copied into a Matroska file of its own |
| POST | `/v1/keyframes` | index a file's keyframes |
| POST | `/v1/prefetch` | read ahead what a playback reads first |
| POST | `/v1/hold` | pause background work while a direct play starts |
| POST | `/v1/fingerprint` | Chromaprint values of part of the audio |
| POST | `/v1/frames` | darkness and contrast of key frames, for credits detection |
| POST | `/v1/jobs` | create an HLS job |
| GET | `/v1/jobs` | every job's `JobStatus` |
| GET | `/v1/jobs/{id}` | one job's `JobStatus` |
| DELETE | `/v1/jobs/{id}` | `204`: kill its FFmpeg, delete its segments |
| POST | `/v1/jobs/{id}/touch` | `204`: keep a paused session alive |
| POST | `/v1/jobs/{id}/bitrate` | change a transcode's video bitrate as it plays |
| GET | `/v1/jobs/{id}/index.m3u8` | HLS VOD playlist |
| GET | `/v1/jobs/{id}/master.m3u8` | multivariant playlist |
| GET | `/v1/jobs/{id}/{n}.ts` | MPEG-TS segment `n` |
| GET | `/v1/jobs/{id}/v{k}.m3u8` | rendition `k`'s playlist |
| GET | `/v1/jobs/{id}/v{k}-{n}.ts` | rendition `k`'s segment `n` |
| POST | `/v1/conversions` | convert a whole file |
| GET | `/v1/conversions/{id}` | `ConversionStatus` |
| DELETE | `/v1/conversions/{id}` | `204`: stop its FFmpeg, delete the file |
| GET | `/v1/conversions/{id}/file` | the converted file |

`?` marks an optional field. The full types, with every response field, are in
`packages/transcoder-api/src/lib.rs`.

### Info and capabilities

`GET /v1/info` returns `TranscoderInfo`:

| Field | Meaning |
|---|---|
| `name`, `version`, `api_version` | the program and its API version |
| `commit` | the build's short hash, when known |
| `license` | `LicenseRef-Restricted` |
| `source` | where to get the source |
| `started_at_ms` | start time |
| `capabilities` | what this transcoder and its FFmpeg can do (below) |
| `limits` | `max_jobs`, `max_hw_jobs` |
| `memory` | `{available, reason?, path?, budget_bytes}`: whether segments can be kept in memory (see [Segments in memory](#segments-in-memory)) |

`capabilities`:

| Field | Meaning |
|---|---|
| `ffmpeg_available`, `ffprobe_available`, `ffmpeg_path`, `ffprobe_path`, `ffmpeg_version` | the binaries found |
| `encoders` | video encoders that passed a test encode, best first; `main10` on HEVC ones that passed in 10 bits too |
| `hwaccels` | decoding hwaccels `ffmpeg -hwaccels` reports |
| `tonemappers` | working tone mappers, best first: `libplacebo`, `opencl`, `zscale` |
| `dolby_vision` | Dolby Vision profile 5 renders with correct colors (needs libplacebo) |
| `vulkan_decode` | codecs the GPU decodes straight into Vulkan memory, for libplacebo |
| `vulkan_encode` | `h264_vulkan` encodes libplacebo's output on the GPU |
| `gpu_pipelines` | decode → scale → encode pipelines that keep frames on the GPU, with the input codecs each passed its test for |
| `vaapi_tonemappers` | tone mappers whose frames reach a VAAPI encoder (Linux; see [VAAPI encoders](#vaapi-encoders)) |
| `vaapi_vulkan_decode` | `vulkan_decode` on the Vulkan device made from the VAAPI one |
| `subtitle_burn_image`, `subtitle_burn_text` | picture and text subtitles can be burned in |
| `readrate` | `-readrate` is supported (see [Throttling](#throttling)) |
| `readrate_catchup` | `-readrate_catchup` is supported |
| `chromaprint` | `POST /v1/fingerprint` works |
| `hls_master_playlist` | jobs have `master.m3u8`, and remuxes honor `dolby_vision` and `strip_hdr10plus` |
| `hdr_transcode` | a 10-bit HEVC encoder works: transcodes can keep HDR |
| `subtitle_matroska` | `POST /v1/subtitles/mks` works |
| `conversions` | `POST /v1/conversions` works |
| `conversions_mp4` | conversions can make MP4 |
| `hls_variants` | jobs make lower renditions (`variants`) |
| `live_bitrate` | `POST /v1/jobs/{id}/bitrate` works |

### Media

`POST /v1/probe` takes `{path}` and returns `MediaInfo`, cached per file
version (path, size, modification time).

`POST /v1/thumbnail` returns `image/jpeg`:

| Field | Meaning |
|---|---|
| `path` | the file |
| `time_ms?` | position of the frame (default: 20 % into the file) |
| `width?` | output width in pixels (default 640) |
| `duration_ms?` | the file's duration, to compute the default position |
| `hdr` | tone map HDR sources (see [Frame grabs](#frame-grabs)) |

`POST /v1/cover` takes `{path}` and returns the embedded cover art
(`image/jpeg` or `image/png`).

`POST /v1/trickplay` makes seek-bar previews:

| Field | Meaning |
|---|---|
| `path` | the file |
| `interval_ms?` | time between thumbnails (default 2 to 10 s, by the file's length) |
| `width?` | thumbnail width in pixels (default 320) |

It returns `TrickplayInfo {id, interval_ms, width, height, columns, rows,
count, sheets}`. Thumbnail `k` shows the video at `k × interval_ms`; each
sheet holds `columns × rows` of them, left to right, top to bottom.

- Only keyframes are decoded: seconds for a film.
- HDR and Dolby Vision are tone mapped with libplacebo when available.
- A set is cached per file version until every sheet has been fetched, or for
  a day. Asking again meanwhile returns at once.
- `GET /v1/trickplay/{id}/{n}.jpg` returns sheet `n` (`0` to `sheets - 1`).
  Once every sheet has been fetched the set is deleted: fetch them all in one
  go and keep them.

### Subtitles

`POST /v1/subtitles` returns `text/vtt` (see
[Subtitle extraction](#subtitle-extraction)):

| Field | Meaning |
|---|---|
| `path` | the file |
| `stream_index?` | the embedded text track. Without it, `path` is itself a subtitle file. |
| `partial?` | embedded tracks: answer within a few seconds (below) |

With `partial`, the answer is the whole track when it is done within a few
seconds (as a rule when it can be read through the file's Cues). Otherwise it
is the cues read so far, with `X-Pley-Complete: 0`, and the extraction goes on
in the background.

`POST /v1/subtitles/mks` takes the same `{path, stream_index?}` and returns
`video/x-matroska`: the track (or the subtitle file, without `stream_index`)
copied as it is into a Matroska file of its own.

- For players that show picture subtitles (PGS, VobSub with its palette, DVB)
  next to an HLS stream.
- Times are the original's, from the start of the file.
- Anything but subtitles fails (`-vn -an -dn`).
- It reads the whole file, as one of the
  [passes over whole files](#ffmpeg-fallback). It is not cached: the server
  caches it.

### Background work

| Endpoint | Body | Response |
|---|---|---|
| `POST /v1/keyframes` | `{path}` | `200` if indexed, `202` if queued (see [Remux](#remux)) |
| `POST /v1/prefetch` | `{path, start_ms?}` | `Prefetched` (see [Reading ahead](#reading-ahead)) |
| `POST /v1/hold` | `{ms}` | `204`: background work waits, and its FFmpeg runs pause, for `ms` (a minute at most) while a direct play starts (see [Background work and playback](#background-work-and-playback)) |
| `POST /v1/fingerprint` | `{path, stream_index?, start_ms, duration_ms}` | `Fingerprint` (see [Audio fingerprints](#audio-fingerprints)); `400` when FFmpeg lacks Chromaprint |
| `POST /v1/frames` | `{path, start_ms, duration_ms}` | `Frames` (see [Key frames](#key-frames)) |

### Jobs

`POST /v1/jobs` takes a `JobRequest` and returns `201` with `JobCreated {id,
duration_ms, segment_seconds, segment_count, video, audio, warnings,
variants}`. See [How HLS jobs work](#how-hls-jobs-work).

`JobRequest`:

| Field | Meaning |
|---|---|
| `input` | absolute path of the file, as the transcoder sees it |
| `media?` | `MediaInfo` the caller has, to skip a probe (ignored without a duration; probed again when the duration is implausible) |
| `label?`, `owner?` | title and user, for status pages |
| `video?` | `VideoRequest`; none drops the video |
| `audio?` | `AudioRequest`; none drops the audio |
| `subtitle?` | `{stream_index?, external_path?}`: an embedded track or a subtitle file (`.srt`, `.ass`, `.sup`, …) to burn in |
| `segment_seconds?` | segment length, 1 to 10 s (default 4) |
| `start_ms?` | start transcoding here right away (playback start or resume) |
| `options` | `TranscodeOptions` |
| `variants` | lower renditions, highest first: `[{max_width?, max_height?, max_bitrate_kbps?}]` (see [Renditions](#renditions)) |
| `container?` | conversions only: `matroska` (default) or `mp4` |

`VideoRequest`:

| Field | Meaning |
|---|---|
| `stream_index?` | default: the first video stream |
| `mode` | `transcode` (default) or `copy` (remux; falls back to transcode when it can't be done, and the plan says so) |
| `codec` | `h264` (default) or `hevc` |
| `max_width?`, `max_height?` | size limit |
| `max_bitrate_kbps?` | bitrate cap (default: by the output size, see [Bitrates](#bitrates)) |
| `dynamic_range` | transcodes of HDR: `sdr` (default), `hdr10` or `hlg` (see [HDR kept](#hdr-kept)) |
| `dolby_vision` | remuxes: `copy` (default), `signal` or `to81` (see [Dolby Vision in remuxes](#dolby-vision-in-remuxes)) |
| `strip_hdr10plus` | remuxes: leave out HDR10+ metadata |

`AudioRequest`:

| Field | Meaning |
|---|---|
| `stream_index?` | default: the default audio stream |
| `mode` | `transcode` (default) or `copy` |
| `codec` | `aac` (default), `ac3` or `eac3` |
| `channels?` | output channels (default 2), never more than the source |
| `bitrate_kbps?` | see [Audio](#audio) for the defaults |

`TranscodeOptions`:

| Field | Default | Meaning |
|---|---|---|
| `hwaccel` | `auto` | `auto`, `none`, `nvenc`, `qsv`, `amf`, `vaapi`, `videotoolbox` or `vulkan` |
| `hardware_decoding` | on | decode on the GPU paired with the encoder |
| `tonemap` | `auto` | `auto`, `libplacebo`, `opencl`, `zscale` or `none` |
| `preset` | `veryfast` | x264 preset for software encodes, or `auto`: chosen and changed by the transcoder from its load and its lead on the player |
| `throttle` | on | hold FFmpeg to real time after a burst (see [Throttling](#throttling)) |
| `in_memory` | on | keep segments in memory when possible (see [Segments in memory](#segments-in-memory)) |

`JobStatus` reports the job's state (`starting`, `running`, `idle`, `finished`,
`failed`), its video and audio plans, its FFmpeg process's progress, restarts,
bytes served, `warnings` and `last_error`; for renditions `rendition` and
`variants`; for memory `in_memory` and `memory_bytes`.

The segment and playlist endpoints:

- `GET /v1/jobs/{id}/{n}.ts` waits for the segment, or starts FFmpeg at it (see
  [Segments on demand](#segments-on-demand)).
- `GET /v1/jobs/{id}/master.m3u8` lists the job's variant (`index.m3u8`), then
  its smaller renditions (`v1.m3u8`…), each with `CODECS`,
  `SUPPLEMENTAL-CODECS`, `RESOLUTION`, `FRAME-RATE`, `VIDEO-RANGE` and its
  bandwidth (see [Dolby Vision in remuxes](#dolby-vision-in-remuxes)).
  Capability `hls_master_playlist`.
- `GET /v1/jobs/{id}/v{k}.m3u8` and `v{k}-{n}.ts`: rendition `k`'s playlist
  and segments. Capability `hls_variants`.

`POST /v1/jobs/{id}/bitrate` takes `{max_bitrate_kbps}` and returns
`{bitrate_kbps}`, the bitrate the transcode goes on at:

- Kept between 1 Mbps and the job's own bitrate.
- The segments past the last one its player asked for, and the next, are
  deleted and made again at the same size and layout. The player plays on from
  the same playlist.
- A running process restarts there at once.
- `400` for a remux. Capability `live_bitrate`.

### Conversions endpoints

`POST /v1/conversions` takes a `JobRequest` and returns `ConversionStatus {id,
state, progress, duration_ms, size?, error?, video?, audio?}` (see
[Conversions](#conversions)).

- Asked again for the same input and request, the same conversion (or its
  file) answers. A failed one starts again.
- `GET /v1/conversions/{id}`: `state` is `queued`, `running` (with `progress`,
  0 to 1), `done` (with `size`) or `failed` (with `error`).
- `GET /v1/conversions/{id}/file`: `video/x-matroska` or `video/mp4`, once
  done. One range (`bytes=a-`, `bytes=a-b`) as asked; `416` past the end.
- Capabilities `conversions`, and `conversions_mp4` for MP4.

## Conversions

A conversion makes a `JobRequest` into one Matroska file in a single FFmpeg
run. It is what a download is when the device that keeps it can't play the
original (sound it doesn't decode, a picture too big for it).

- **Streams.** The same plan as a job's segments (encoder, tone mapping, AAC),
  made at full speed (`throttle` is ignored) from the start (`start_ms` is
  ignored too).
- **Subtitles.** Every subtitle track of the input, in its order, with its
  language and name, so a player finds a track by its place as in the
  original. MP4's text is made SRT; teletext (which Matroska can't hold) is
  left out.
- **Dolby Vision and HDR10+.** A remux whose segments would be edited (Dolby
  Vision converted, HDR10+ left out) has its video encoded instead: a file
  can't be edited the way segments are.
- **Short output.** An FFmpeg run that ends without error but short of the
  media's end (a share that went away) fails rather than keeping half a film.
- **MP4** (`container: "mp4"`), for players that open no Matroska
  (QuickTime):
  - text subtitles made MP4's own (`mov_text`), in their order; picture
    subtitles left out;
  - HEVC tagged `hvc1` (QuickTime plays no `hev1`);
  - its index ahead of the media (`+faststart`), so it plays as soon as it
    opens;
  - a separate conversion from the Matroska one of the same request (its id
    says so), named `<id>.mp4`.

Scheduling:

- One conversion runs at a time, in a transcode slot like a job's process: a
  hardware session when one is free, else the software encoder. While every
  slot is in use it waits.
- It runs at low process priority, so playbacks keep the processor.
- Its id comes from the input (path, size, modification time) and the
  request's video, audio and subtitle parts: the same file asked for twice is
  made once.
- A queued conversion nobody asks for or about (`GET /v1/conversions/{id}`)
  for an hour is not made when its turn comes. Asking again starts it again.

Files:

- Kept in `conversions/` in the work folder. The janitor deletes those unused
  for a week, then the least recently used past 64 GB.
- One the janitor deleted is made again when asked for.
- After a restart, the transcoder knows the files it made by their names.
  Half-made ones are deleted.

## How HLS jobs work

Quality menus, Automatic quality and when players switch are explained from
the user's side in [QUALITY.md](QUALITY.md). This section is how the
transcoder serves them.

### Segment layout

When a job is created its segment layout is fixed, and the whole VOD playlist
is known up front, so players can seek anywhere.

- Segments are `segment_seconds` long (1 to 10 s, 4 s by default). The Pley
  server asks for 2 s: nothing plays before FFmpeg has written a whole
  segment, so shorter ones start playback, seeks and quality switches sooner.
- **Transcodes:** keyframes are forced on that grid (a keyframe every segment).
- **Remuxes** (`video.mode = copy`): segments start on the source's keyframes,
  as many as make at least the segment length, taken from the keyframe index.
  Keyframe times are made relative to the container's start time
  (`MediaInfo.start_ms`, often far from zero in MPEG-TS), because `-ss` and the
  output timeline use it.
- The layout is deterministic: if the transcoder restarts, the server
  recreates the job and the same segment numbers still line up.
- Limits:
  - At most 200 000 segments per job, so a damaged or crafted file can't claim
    a length that costs gigabytes of memory. Long files get longer segments
    instead of more.
  - Durations beyond about 9 days are not believed. The probe then uses the
    streams' own durations (Matroska `DURATION` tags included), or the file
    size over the streams' bitrates. A file with nothing plausible is refused.
  - Media info sent with a job is probed again when its duration is
    implausible.

### Segments on demand

A segment request is answered from disk (or memory) if the segment exists.
Otherwise:

- **Wait** when a running FFmpeg process is about to produce it: at most 12 s
  of segments ahead of the process, never behind it.
- **Restart instead of waiting** when the process is held to the read rate
  (see [Throttling](#throttling)) and waiting would take longer than a
  restart. Past its burst, a process makes a 2 s segment in about a second,
  so a seek a few segments ahead would wait seconds. A new process (with a
  burst of its own) starts at the requested segment when all of these hold:
  - the process goes at the read rate, not slower (slower means the encoder is
    the limit, and a new process would be no quicker);
  - the wait, counted from halfway through the segment being made, is longer
    than this job's last start-to-first-segment time plus stopping the old
    process;
  - no other request waits for a segment the old process is about to make.
- **Otherwise start a new process** exactly at that segment, with `-ss`,
  `-output_ts_offset`, `-segment_times` and `-segment_start_number`, so
  timestamps stay on the absolute timeline. `avoid_negative_ts` is disabled,
  so B-frame delay can't shift one process's segments against another's.
- **Remux seeks** go a little past the boundary keyframe:
  - With B-frames, FFmpeg moves a seek back by 3/23 s (130 ms) in every
    container that doesn't seek by presentation time (all but MP4 and MOV). A
    seek right at the keyframe would land on the one before, and every
    segment of the process would hold the GOP before its own.
  - The seek goes 140 ms past the boundary there (at most halfway to the next
    keyframe), 1 ms elsewhere.
  - Remuxes seek with `-noaccurate_seek`, so converted audio starts at the
    keyframe like the copied video. Starting it at the seek point would leave
    a hole where an older process's segment plays into the new one's.
- A new process stops right before the next segment that already exists, so
  segments being served are never rewritten.
- A process started less than 3 s ago is not replaced while another request
  is waiting for it, so two requests far apart can't keep restarting each
  other. When that request goes away (served, or aborted by its player), the
  one held back looks again at once.

### Completion tracking

- FFmpeg's segment muxer writes a line to stdout as each segment finishes. That
  marks it ready and wakes waiting requests at once, with no polling. The
  process is first noted as past that segment, so a woken request never sees
  it still owing it.
- A remux whose video needs editing has the segment edited first (see
  [Dolby Vision in remuxes](#dolby-vision-in-remuxes)).
- FFmpeg's `-progress` output on stderr supplies the live metrics: fps, speed,
  bitrate and position.

### Throttling

Applies when the request's `throttle` is on (`throttle_transcodes` in the
server, on by default). The read rate needs FFmpeg support (`readrate`).

- **Burst, then real time.** A process makes its first 120 s as fast as it
  can, then reads at real time (`-readrate 1`). It stays as far ahead of the
  player as the burst took it, at a steady 1×. This saves CPU, GPU and disk.
- **Catch-up.** A read that falls behind (a slow disk or share) catches up at
  2× where FFmpeg can (`-readrate_catchup`, FFmpeg 8).
- **Pause far ahead.** Once a process is 90 s ahead of the last segment the
  player asked for, it is suspended (`NtSuspendProcess` on Windows, `SIGSTOP`
  on Linux) until it is less than 60 s ahead, or a request waits for a segment
  it is to make. Players keep their own buffer ahead of the picture, so this
  is minutes ahead of what is shown. Without the pause a process could run to
  the end of the film: work lost when the player stops or seeks, taken from
  other playbacks meanwhile, and segments on the disk. On a small computer the
  pause is what lets two playbacks run at once.
- **Speed.** A process's speed is measured over its last 4 s of progress
  (since it last resumed), because FFmpeg's own `speed` averages in the burst
  and the pauses. A job's status reports that speed, and 1× for a process
  held to the read rate.

### Housekeeping

- A process idle for 60 s (paused player) is stopped.
- A job idle for 20 minutes expires.
- Segments more than 6 minutes behind the playhead are deleted, in small
  batches, off the async runtime. In memory: those more than a minute behind
  it or 3 minutes ahead (see [Segments in memory](#segments-in-memory)).
- On startup, segments left from an earlier run are removed, and in memory
  those of transcoders that are gone (a crash).
- Cache pruning: see [Work folder](#work-folder).
- FFmpeg never outlives the transcoder: on Windows the transcoder runs in a
  kill-on-close job object; on Linux every child gets a parent-death signal.
- On shutdown (a signal, or stdin closed with `--exit-with-parent`) all jobs
  are stopped first, then open requests get 5 s to finish.

### Failure handling

- If FFmpeg fails twice at the same segment, the error is returned instead of
  retrying forever. The job's `last_error` holds FFmpeg's last log lines.
- A process whose hardware encoder fails before its first segment (a driver
  update, a GPU gone, a session refused) hands the job to the software
  encoder, as when hardware sessions run out, with a warning on the job.
- Once that happens to three jobs, `capabilities.json` is deleted, so the next
  start tests the encoders again.

### Renditions

A transcode asked for with `variants` (adaptive bitrate: the player moves
between sizes by itself) also comes in smaller renditions.

- Each variant is the job's request with its own `max_width`, `max_height`
  and `max_bitrate_kbps`.
- A variant is kept only when it is no bigger and has fewer bits than the one
  above, and its segments are laid out as the job's. Transcodes' are, as their
  layout depends on the duration alone. Remuxes and conversions ignore
  `variants`. `JobCreated.variants` says how many were kept.
- They are jobs of their own inside the job, in folders `v1/`…, with segments
  named `v1-{n}.ts`… in their playlists.
- Burned-in text subtitles are extracted once and copied to them.
- `master.m3u8` lists them after the job's own, largest first.
- A process runs only for the rendition the player asked for last. When its
  requests move to another, the others' processes stop (what they made
  stays), and one asked for again starts where it is wanted.
- They expire, go and are tidied with the job. Its status is the playing
  rendition's (`rendition`, `variants`), with what all of them served.

### Bitrates

A transcode's bitrate, unless asked for less, follows its size. Lines are
counted as a 16:9 picture of the same width would have them, so a 1920×800
film gets 1080p's 8 Mbps.

| Lines (16:9) | Bitrate |
|---|---|
| 2000 and up | 20 Mbps |
| 1400 | 12 Mbps |
| 1000 | 8 Mbps |
| 700 | 4 Mbps |
| 500 | 2 Mbps |
| 400 | 1.2 Mbps |
| below 400 | 0.7 Mbps |

### Segments in memory

A job's segments live on disk in `jobs/<id>/`, or in memory when all of these
hold:

- the request's `options.in_memory` is on (`transcode_in_memory` in the
  server, on by default);
- the job is throttled, which bounds its lead on the player;
- the transcoder has room (below).

How it works:

- FFmpeg writes segments as files, so memory is a folder in RAM: `/dev/shm`
  on Linux, when it is a tmpfs.
- The transcoder keeps its own folder there, `pley-transcoder-<pid>/<id>/`
  (renditions in `v1/`…), private to its user and removed at shutdown. One
  left by a transcoder that is gone (a crash) is removed at the next start,
  since a tmpfs holds it in memory until then.
- Each process runs in the folder it writes to, with a copy of the burned-in
  subtitles there. Each segment is served, edited (Dolby Vision) and pruned
  where its process made it.
- Conversions and the caches (keyframes, subtitles, seek previews, shaders)
  stay on disk.

Windows and macOS keep segments on disk (`/v1/info` says
`memory.available: false`, "works on Linux only"). They have no folder in RAM
by default; FFmpeg creates the files, so they can't be marked to stay in the
file cache (`FILE_ATTRIBUTE_TEMPORARY`); a RAM disk needs a driver (Windows)
or outlives the transcoder (macOS's `hdiutil`); and segments sent over HTTP
would need the segment muxer and Dolby Vision editing redone in memory.
Windows' file cache keeps just-written segments in memory anyway.

Memory limits:

- **Per job.** Segments in memory are kept a minute behind the playhead (6 on
  disk), and those more than 3 minutes ahead (left by a seek back) go too. A
  throttled process pauses 90 s ahead of the player.
- **Budget.** The transcoder takes at most a quarter of the computer's memory
  and half of the folder's size (`/v1/info`'s `budget_bytes`). A budget under
  256 MB counts as no memory: Docker's default 64 MB `/dev/shm` is too small,
  so give the container `--shm-size` (deploy/docker's compose file gives
  2 GB).
- **Reservation.** Each job (each rendition) reserves 3 minutes of its
  bitrate: a transcode's plus a quarter, a remux's at the source's, 40 Mbps
  when unknown. 4K at 20 Mbps takes about 700 MB.
- **Admission.** A process goes to memory when its reservation fits the
  budget beside the others' (or what they actually take, when more), the
  folder keeps 64 MB free, and the computer keeps a tenth of its memory free
  (512 MB at least, from `MemAvailable`). Otherwise it goes to disk.
- **Running short.** When memory runs short later (past the budget, or the
  folder or the computer short of room), the job with the most in memory moves
  to disk, one every 5 s. Its reservation goes, and its process starts again
  on disk at the segment it was making (a paused one, far ahead, at the next
  request). What it made stays in memory, served from there until pruned, so
  the player notices nothing.

## Transcoding features

### Encoders

Each candidate encoder is test-encoded at startup, not just listed:

| Encoder | Platforms |
|---|---|
| NVENC, QSV (Quick Sync), AMF | Windows and Linux |
| VAAPI | Linux, on the first Intel or AMD render node (`PLEY_VAAPI_DEVICE` names another) |
| VideoToolbox | macOS |
| libx264, libx265 | everywhere (software fallback) |

- `hwaccel = auto` picks the first hardware encoder that works.
- Forced keyframes are IDR frames (`-forced-idr`), so every segment decodes on
  its own.
- The results are cached in `capabilities.json`, keyed by the FFmpeg binary's
  size and modification time and by the GPUs the system shows (its render
  nodes and their drivers, NVIDIA's driver). A GPU added later is tested.
  `--redetect` ignores the cache.

#### Rate control

Every encoder is capped at the job's bitrate (`-maxrate`, with a buffer of two
seconds of it) and may spend less on what is easy to encode. The start-up test
encodes with the same options.

| Encoder | Rate control |
|---|---|
| NVENC | VBR by quality: `-rc vbr -cq 23` (26 for HEVC) |
| AMF | peak-constrained VBR: `-rc vbr_peak` |
| x264, x265 | CRF 21 and 24 |
| VAAPI | `-rc_mode VBR` |
| Quick Sync | maxrate one bit above the bitrate, and bitrate control per macroblock (`-mbbrc 1`) |
| VideoToolbox | `-maxrate` kept (its `DataRateLimits`, a hard limit over one second) |

- FFmpeg makes Quick Sync and VAAPI CBR when bitrate and maxrate are equal
  (`qsvenc.c`'s `select_rc_mode`, `vaapi_encode.c`'s rate control selection).
  Hence VAAPI's explicit VBR and Quick Sync's extra bit. Quick Sync counts in
  kbit/s, so its cap is unchanged.
- On Mesa's D3D12 VAAPI driver, FFmpeg logs `RC mode: VBR`. A still picture
  comes out 6 % smaller than with CBR (377 against 400 kbit/s for 4000 asked);
  a noisy one the same.
- FFmpeg accepts the options.
- VideoToolbox: some projects leave `-maxrate` out over reported hangs, for
  which FFmpeg's source shows no cause. Pley keeps it because the playlist's
  stated bandwidth relies on the cap.
- AMF HEVC needs no `-header_insertion_mode`: with `-forced_idr 1` FFmpeg asks
  for the VPS, SPS and PPS with every forced keyframe
  (`AMF_VIDEO_ENCODER_HEVC_INSERT_HEADER`, `amfenc.c`), and every segment
  starts on one.

### Hardware decoding

- The decoder is paired with the encoder: `cuda` with NVENC, `d3d11va` with
  QSV or AMF on Windows, `vaapi` with VAAPI, `videotoolbox` with VideoToolbox.
- Frames come back to system memory. FFmpeg falls back to software decoding by
  itself when the GPU doesn't decode the codec.
- A job whose FFmpeg says its hardware decoder failed to start ("Failed setup
  for format …") shows `cpu` as its decoder, with a warning.

#### Scaling on the GPU first

Frames that come to system memory to be processed are first scaled by the GPU
that decoded them, so only the output's pixels are copied (4K to 1080p: a
quarter of them). This covers burned-in subtitles, zimg's and OpenCL's tone
mapping, and x264 or x265 (including the software stand-in when hardware
sessions run out).

- CUDA: `-hwaccel cuda -hwaccel_output_format cuda` and
  `scale_cuda=w=…:h=…:format=nv12,hwdownload,format=nv12`; 10 bits (`p010le`)
  for tone mapping and kept HDR.
- VAAPI: `scale_vaapi`, 8 bits only (its start-up test makes NV12; 10 bits on
  the GPU is tested with CUDA only).
- Used only where the decoder's zero-copy pipeline passed its start-up test
  for the codec (`gpu_pipelines`). A decoder that can't would hand the scaler
  frames in memory.
- Not used for interlaced video (deinterlaced at its own size first) or for
  libplacebo (which scales as it tone maps).
- Mesa's D3D12 VAAPI driver (WSL) crashes in `scale_vaapi`. Its pipeline fails
  the start-up test, and VAAPI frames there come to memory whole.
- SDR pictures are the same either way (luma PSNR 50 and 46.5 dB, encoder
  noise included). HDR is then scaled in PQ rather than linear light, which
  differs where fine bright detail meets dark (PSNR 33 dB on FFmpeg's test
  pattern), as with any tone mapper that scales first (OpenCL).
- Measured (RTX 5070, 24-thread Ryzen, 20 s of 4K HEVC to 1080p H.264, CPU
  time, median of three): burned-in text subtitles with NVENC 10.0 → 3.3 s;
  x264 19.9 → 14.6 s; HDR10 tone mapped by zimg into NVENC 78 → 48 s (wall
  time 12.1 → 9.1 s for 30 s of it, median of five).

#### Vulkan decoding

Vulkan decoding (into libplacebo) counts for a codec only when the start-up
test's frames reach the filters in Vulkan memory (`pixfmt:vulkan` in FFmpeg's
verbose log). libplacebo also takes frames from memory, so a driver without
Vulkan video decoding (AMD's for Vega graphics on Windows, say) would
otherwise pass while the CPU decodes. Those GPUs decode with the encoder's
decoder instead.

#### NVDEC frame copy

NVDEC runs without `-hwaccel_flags +unsafe_output`, which skips the copy of
each decoded frame out of the decoder's surface (`nvdec.c`'s
`av_frame_make_writable`) and is unsafe because it hands those surfaces to the
filters. The copy, inside the GPU, costs nothing measurable (RTX 5070, FFmpeg
9.0.2, 30 s of 4K HEVC to 1080p H.264, median of seven): 1.65 s without the
flag against 1.73 s with it on the GPU, 3.72 s against 3.90 s through memory,
identical frames (framemd5, 8 and 10 bits).

### Tone mapping

HDR10, HLG and Dolby Vision are tone mapped to SDR BT.709 by the best
available tone mapper:

| Order | Tone mapper | Runs on | Notes |
|---|---|---|---|
| 1 | `libplacebo` | GPU (Vulkan) | Also applies Dolby Vision RPU reshaping, so **profile 5** (IPTPQc2) has correct colors. |
| 2 | `tonemap_opencl` | GPU (OpenCL) | |
| 3 | `zscale` + `tonemap` (zimg) | CPU | The first `zscale` scales to the output's size as it makes the picture linear, so the tone mapping (32-bit floats, three planes) works on the output's pixels. |

- When Vulkan runs only on the CPU (Mesa's llvmpipe, no GPU), libplacebo comes
  last: it is several times slower than zimg there. Only profile 5 picks it,
  for its colors.
- Dolby Vision profiles 7 and 8 have an HDR10- or HLG-compatible base layer
  and work with any tone mapper.
- Profile 5 without libplacebo gives a warning. Its frames carry no color
  properties (those come only from the RPU), so they are tagged BT.2020 PQ for
  zimg and OpenCL. The video plays, with off colors.
- The same tagging fills in whatever an HDR stream leaves untagged.

#### Peak detection

- libplacebo tone maps by the stream's own metadata (`peak_detect=0`): HDR10's
  mastering display and content light levels, Dolby Vision's and HDR10+'s per
  scene. Thumbnails and seek previews tone map the same way.
- Why: peak detection follows the brightest pixels seen so far, and each
  FFmpeg process starts with none. A process started by a seek or a size
  switch would render the same frames brighter or darker than the one before
  it: a jump at the seam between segments.
- Measured on a clip whose highlight goes from 200 to 4000 nits every 3 s:
  with detection, a face on a segment's first frame is 121 (of 255) or 128
  depending on where the process started; without it, every process gives the
  same frames bit for bit, the face at 111 (a little darker: the curve follows
  the stream's stated 4000 nits, not the 900 the frames reach).
- `tonemap_opencl` has peak detection that can't be turned off (63 frames, in
  its kernel), so it has the same seams. zimg's `tonemap` has none.

#### VAAPI encoders

Before a VAAPI encoder (Intel and AMD graphics on Linux), the tone mapper's
device is made from VAAPI's:

```
-init_hw_device vaapi=va:<render node> -init_hw_device vulkan=vk@va
-filter_hw_device vk ... hwupload=derive_device=vaapi
```

(`opencl=ocl@va` in place of `vulkan=vk@va` for OpenCL.)

- VAAPI's encoders take frames in VAAPI's memory only. A plain `hwupload`
  uploads to the filters' device (Vulkan's or OpenCL's), and the encoder can't
  start ("Impossible to convert between the formats supported by the filter
  'Parsed_hwupload' and the filter 'auto_scale'"). `derive_device=vaapi` finds
  the VAAPI device the filters' device was made from.
- A Vulkan device made from VAAPI's is the same GPU where VAAPI says which it
  is (VA-API 1.15's PCI id, or AMD's name), and Vulkan's first device
  otherwise.
- An OpenCL device can be made from VAAPI's only by Intel's OpenCL
  (`cl_intel_va_api_media_sharing`), not AMD's.
- At startup each GPU tone mapper is tried this way into `h264_vaapi`
  (`capabilities.vaapi_tonemappers`), and Vulkan decoding too, per codec
  (`capabilities.vaapi_vulkan_decode`).
- Jobs with a VAAPI encoder use only the tone mappers that passed, and zimg.
  Where none is left, or Dolby Vision profile 5 would lose libplacebo's
  colors, the job encodes with x264 instead, with a warning.
- HDR kept in HEVC that libplacebo would convert is tone mapped instead when
  libplacebo's frames don't reach VAAPI's encoder.
- Tested with FFmpeg 9.0.2 and Mesa 26.2 in Debian (Docker on WSL, VAAPI
  through Mesa's D3D12 driver on an RTX 5070, Vulkan on llvmpipe): HLS
  segments with libplacebo (the VA display's PCI id hidden, so Vulkan falls
  back to llvmpipe) and with zimg; an OpenCL device that can't be made from
  VAAPI's fails its start-up test.

#### Shader cache

- libplacebo keeps the shaders it compiles in the work folder's `libplacebo/`
  (made at startup, with an absolute path, since FFmpeg runs in a job's
  folder).
- This saves about 0.2 s of compiling at every start, seek and quality switch
  of a tone-mapped job.
- A work folder whose path holds a character FFmpeg's filter syntax reads
  (`, ; [ ] ' :`, other than a drive's colon) goes without it.

### HDR kept

With `video.codec = hevc` and `video.dynamic_range` `hdr10` or `hlg`, HDR
video is encoded in HEVC Main 10 instead of being tone mapped.

- **Detection.** Encoder detection also test-encodes every working HEVC
  encoder in 10 bits (`-profile:v main10`, P010; x265 with `yuv420p10le` and
  `-dolbyvision 0`) and marks it `main10`. `capabilities.hdr_transcode` says
  one passed.
- **Same range.** Video already in the asked range is decoded, scaled to P010
  and encoded, not tone mapped: HDR10, HDR10+ and Dolby Vision 7 and 8.1 (as
  their HDR10 base layer) for `hdr10`; HLG and Dolby Vision 8.4 for `hlg`.
  Scaling uses `scale_cuda` on the GPU when the CUDA zero-copy pipeline
  handles the codec, else the CPU.
- **Other range.** Other HDR (HLG or Dolby Vision 8.4 to HDR10, PQ to HLG,
  Dolby Vision profile 5 to either) goes through libplacebo with a BT.2020
  target in 10 bits (`format=p010`, `color_trc=smpte2084` or `arib-std-b67`).
  It uses the Vulkan pipeline when the GPU decodes the codec into Vulkan
  memory.
- **Metadata.**
  - The decoded frames' HDR10 metadata (mastering display, content light
    levels) goes through scaling and libplacebo to the encoder. The encoder
    writes it as SEI messages with every frame (NVENC, and x265 with
    `repeat-headers=1`), so every segment starts with it.
  - PQ or HLG, BT.2020 primaries and matrix are set on the encoder
    (`-color_trc`…), so the stream states them whatever the frames were tagged
    with.
  - Dolby Vision RPUs are never written (NVENC doesn't, x265 is told not to):
    Dolby Vision comes out as its base layer, or converted when it has none.
  - HDR10+ dynamic metadata is not kept.
- **Fallback to SDR.** The job tone maps to SDR, with a warning, when H.264 is
  asked for, subtitles are drawn onto the video, there is no `main10` encoder,
  or a change of range (above) needs libplacebo and it is missing.
- When hardware sessions run out, the software fallback for an HDR job is a
  `main10` x265.
- **Output.** The master playlist names the output `hvc1.2.4.LLL.B0` with
  `VIDEO-RANGE=PQ` or `HLG`. The plan's `dynamic_range_out` says `hdr10`,
  `hlg` or `sdr` for transcodes.

### Frame grabs

`/v1/thumbnail` with `hdr`:

- Dolby Vision profile 5 is tone mapped through libplacebo, other HDR through
  zimg.
- If tone mapping fails (a Vulkan device that went away, for example), the
  frame is grabbed as it is rather than not at all.

### Subtitle burn-in

Picture subtitles (PGS, VobSub, DVB, `.sup` files):

- Overlaid on the scaled, tone-mapped frame, their canvas fitted to its width.
  For 4K to 1080p, a quarter of the pixels to blend.
- Blended on the CPU, after the one download in Vulkan pipelines too: about
  three quarters of a Ryzen 9 5900X core for 4K, a quarter of that for 1080p.
- FFmpeg's GPU overlays aren't used. libplacebo makes an output frame for each
  frame of each input, and FFmpeg sends a subtitle frame with every video frame
  (sub2video's heartbeat), doubling them. `overlay_vulkan` takes its two inputs
  in one format only, and gave black frames for `yuva420p` on NVIDIA. The
  integrated GPUs that would gain most also have more CPU than GPU to spare.

Text subtitles (SRT, ASS):

- Rendered by libass from an `.ass` file extracted into the job folder while
  the job is created.
- An embedded track's extraction reads the whole file, so it gives up after 20
  minutes, inside the 25 the server waits for the job. A job folder whose job
  isn't made after all (an error, or the request gone) is removed.
- Drawn with the fonts attached to the file. Styled ASS (anime above all)
  names fonts only the file has; without them libass falls back to a system
  font.
- The fonts are read from a Matroska file's Attachments element alone (which
  its seek head points to) while the track is extracted, so they cost the
  start nothing.
- They are kept in `subtitles/<file key>.fonts/` (256 MB at most per file) for
  the next job of the same file version, and passed to the filter as
  `fontsdir`. Subtitle files next to the video are drawn with the video's
  fonts too.
- Rendering from the source file with the filter's `si=` would read the whole
  file in every FFmpeg process, at each seek; hence the extraction.

### Audio

**Copy.** `audio.mode = copy` copies AAC, MP3, AC-3 and E-AC-3, which MPEG-TS
carries. Anything else asked to be copied is converted, with a warning.

**Conversion.** Converted audio is encoded to `audio.codec` with FFmpeg's own
encoders (in every build):

| `codec` | Encoder | Channels | Default bitrate |
|---|---|---|---|
| `aac` (default) | `-c:a aac` | stereo unless `channels` says more, up to 7.1 | 192 kbit/s stereo, 64 per channel above |
| `eac3` (Dolby Digital Plus) | `-c:a eac3` | at most 5.1 (`-ac 6`) | 192 stereo, 640 above |
| `ac3` (Dolby Digital) | `-c:a ac3` | at most 5.1 (`-ac 6`) | 192 stereo, 640 above |

- `channels` never goes above the source's: a stereo track stays stereo.
- `bitrate_kbps` overrides the default, within 64 to 640.
- Audio not at 44.1 or 48 kHz is resampled to 48 kHz (the AC-3 encoders take
  nothing above 48 kHz).
- The job's audio plan reports the codec as `codec_out`.
- The Pley server asks for E-AC-3 5.1 for players whose output has 6 channels
  or more and that decode E-AC-3 or pass it through, AC-3 5.1 when they take
  only AC-3, and AAC otherwise (see [API.md, Version 2](API.md#version-2)).

**Lossless sources** are decoded only as far as the conversion needs:

- DTS-HD Master Audio converted to 5.1 or fewer channels is decoded from its
  lossy core (`-core_only`).
- TrueHD converted to stereo or 5.1 is decoded from the substream mastered
  with those channels (`-downmix stereo` or `5.1`). Blu-rays carry both inside
  the 7.1; without one FFmpeg decodes the whole.

**Downmix to stereo.** 5.0, 5.1 and 7.1 go through Pley's own matrix
(`DOWNMIX_51`, `DOWNMIX_71` in `plan.rs`) rather than `-ac 2` alone.

- FFmpeg's matrix gives each front channel whole, the center and surrounds at
  -3 dB, and drops the LFE. It is unnormalized for the encoders' float
  samples, so loud passages go past full scale.
- Pley's matrix is the same but for the center, where dialogue is, at 0 dB.
  It ends with a limiter at -2 dBFS (`alimiter`, its look-ahead taken back
  out, so the sound stays in time).
- The layout is made 5.1 (or 7.1) first, so a 5.0 track or a broadcast
  switching to stereo still finds its channels.
- TrueHD decoded to stereo by its own decoder, other layouts and unknown
  layouts keep `-ac 2`.
- The matrix doesn't move where a segment's sound starts (to the sample), from
  0 and after a restart alike.
- Measured on a 30 s 5.1 test mix (speech-band noise bursts in the center,
  noise elsewhere) encoded to AAC, FFmpeg's matrix against Pley's: dialogue
  -29.5 against -26.5 LUFS, the rest -23.0 both; made 12 dB louder (channels
  peaking at -1.3 dBFS), true peak +0.4 against -0.9 dBTP at -10.4 LUFS both.

**Copied sound beside transcoded video.** A process that starts past 0 leaves
out the audio packets before the seek point (`-copypriorss:a 0`). FFmpeg
otherwise copies from wherever the demuxer's seek landed while the decoded
video starts at the seek point: an E-AC-3 track in Matroska restarted at 12 s
would begin 1.57 s early, over the previous segment's sound. With the option
it begins at the first packet from the boundary (13.427 s against the video's
13.400). A remux keeps its sound from the keyframe its video starts on.

### Remux

H.264 and HEVC video is copied into HLS, with segments cut exactly on the
source's keyframes. The keyframe index comes from:

- **Matroska and WebM:** the file's Cues element, read instantly, so remuxing
  works on the first play.
- **MP4, MOV and M4V:** the video track's sample tables (`stss`, `stts`,
  `ctts`) and edit list, or the movie fragments' sample flags in fragmented
  files (CMAF, OBS recordings), read just as quickly.
  - Times match FFmpeg's packet timestamps, edit lists included. Checked
    against ffprobe on files written by FFmpeg and by Windows Media Foundation
    (`moov` at the end, B-frames without an edit list).
  - Files with structures this doesn't model (several edits, sync points in
    sample groups, inconsistent tables) are left to the packet scan, as are
    fragmented files whose fragments take more than 2 s to walk.
- **Other containers:** a packet scan in the background (one file at a time,
  reading packet headers) after `POST /v1/keyframes`, which the server sends
  when an item page is opened. Until the index exists, jobs transcode.

Formats are recognized by content, not by file name.

### Turned pictures

Phones store portrait video as landscape frames with a display matrix that
turns them.

- The probe reads the angle (`MediaStream.rotation`, ffprobe's `rotation`: 90,
  -90, 180).
- A transcode is planned at the size the picture is shown: 1920×1080 frames
  turned a quarter are 1080×1920, 608×1080 within 1080p.
- FFmpeg turns frames it brings to memory before the filters (`-autorotate`,
  its default), never frames on a GPU. So turned video takes the CPU's
  filters, hardware decoded into memory (not the zero-copy, Vulkan or
  GPU-scaled paths).
- MPEG-TS can't say a picture is turned, so a remux asked for is transcoded
  instead, with a warning.
- Media info probed before rotation was read has no `rotation`: such files are
  planned unturned until probed again.

### Frames timed as they are decoded

Some MP4s (FFmpeg muxing a raw H.264 or HEVC stream) give every frame its
decode time as its display time, although B-frames are stored ahead of where
they are shown. Players that trust these times show frames out of turn or drop
them (Chrome drops about one in five), and copying the video keeps them.

- The probe of an MP4 or MOV file reads the times of its first 128 packets
  along with its streams. (Matroska times frames as they are shown; MPEG-TS
  carries both.)
- When a stream that reorders frames (`has_b_frames`) shows every packet read
  at its decode time, its first 48 packets are decoded. If more than one frame
  comes out earlier than the frame before it, the stream is marked
  `decode_order_times`.
- The server then transcodes its video: decoding gives every frame its right
  time.

### Dolby Vision in remuxes

A copied HEVC stream keeps its Dolby Vision metadata (the RPU NAL units, type
62) in the MPEG-TS segments. MPEG-TS can't say so (FFmpeg's MPEG-TS muxer
writes no Dolby Vision descriptor), so `master.m3u8` does, as
`VideoRequest.dolby_vision` asks:

| `dolby_vision` | What `master.m3u8` says |
|---|---|
| `copy` (default) | nothing about Dolby Vision: the base layer's codec only (`hvc1.2.4.L153.B0`) |
| `signal` | named, for a player that decodes it. Profile 5: `dvh1.05.LL` alone. Profile 8: the HEVC codec with `SUPPLEMENTAL-CODECS="dvh1.08.LL/db1p"` (8.1, `VIDEO-RANGE=PQ`), `/db4h` (8.4, `HLG`) or `/db2g` (8.2, `SDR`). Profile 7: `dvh1.07.LL`. |
| `to81` | profile 7 made profile 8.1 and named so (below); other profiles as `signal` |

`to81`:

- FFmpeg can't do it: `dovi_rpu` refuses to write profile 7 RPUs and never
  changes a profile; `dovi_split` (FFmpeg 9) drops the enhancement layer but
  keeps profile 7 RPUs. So FFmpeg copies the stream as it is, and the
  transcoder edits each finished segment before it is ready.
- The enhancement layer's NAL units (type 63) are left out.
- Each RPU is rewritten as `dovi_tool`'s mode 2 does:
  - no residual data (NLQ), no enhancement-layer resampling, one partition;
  - the P8.1 color coefficients in the display management data (BT.2020 YCbCr
    to RGB, RGB to LMS, `signal_color_space` 0);
  - for a full enhancement layer (not a minimal one, whose residual is zero),
    no-op mapping curves, since its curves only make sense with the layer;
  - the extension blocks (levels 1 to 11, 254, 255; content mapping 2.9 and
    4.0) and everything else kept as they are; the CRC computed again.
- The RPU syntax follows FFmpeg's (`libavcodec/dovi_rpudec.c`) and
  `dovi_tool`'s. The code is the transcoder's own (`src/dovi.rs`): nothing of
  theirs is linked or run.
- An RPU that can't be read (or whose CRC is wrong) is left out with a warning
  on the job, and that frame plays as its base layer.
- Profile 7 as MakeMKV writes it (one track) is supported. A disc's dual-PID
  streams carry the RPU in the enhancement-layer PID, which isn't copied.

`strip_hdr10plus`:

- The HDR10+ messages (ITU-T T.35, country 0xB5, provider 0x003C, application
  4) are taken out of each segment's prefix SEI NAL units. Other messages
  (mastering display, content light level, captions) are kept; an SEI unit
  left empty goes.
- For Dolby Vision decoders that show black or freeze with both
  (androidx/media#1895).

Segment editing:

- A segment is read whole, its video PES packets taken apart into NAL units
  and the PES packetized again.
- The first TS packet keeps its adaptation field (PCR, random access),
  continuity counters run on, and other PIDs' packets are left as they were
  (those met in the middle of a PES come right after it).
- A 2 s segment of 4K takes milliseconds.
- The job's video plan reports `dolby_vision` (`copy`, `signal`, `to81`) and
  `hdr10plus_stripped`.

### HDR10+ and profile 7's enhancement layer

- Only a stream's frames say whether it carries HDR10+ (ffprobe's stream side
  data doesn't, in Matroska or MP4). So the probe of a Dolby Vision or PQ
  stream decodes its first frames (a fraction of a second;
  `-read_intervals %+#12 -show_entries frame_side_data`) and records
  `MediaStream.hdr10plus`. A PQ stream with HDR10+ becomes
  `dynamic_range: hdr10plus`.
- For profile 7, the same frames' Dolby Vision metadata says what the
  enhancement layer holds (`MediaStream.dv_el`), as `dovi_tool` tells them
  apart:
  - `mel` (minimal): its residual is zero in every component (no NLQ offset,
    an input maximum of one, no dead zone);
  - `fel` (full): otherwise.
- Both are left out (unknown) when the frames can't be read, and for other
  streams.
- The job's video plan repeats them (`dv_el`, `hdr10plus`) for the input's
  name.

### Subtitle extraction

Text tracks (SubRip, ASS/SSA, plain text and WebVTT) become WebVTT, cached per
file version.

#### Matroska, through the Cues

- mkvmerge and FFmpeg list every subtitle block in the Cues, with its place in
  its cluster. The track asked for is read block by block: one disk seek per
  subtitle instead of reading the whole file.
- A film's track takes milliseconds on a local disk and seconds on a NAS (a
  thousand subtitles of a 77 GB remux on a disk-backed share: 5 to 12 s),
  where reading the file takes minutes.
- Sixteen reads run at once. A file's tracks are read side by side (half of
  `--probe-concurrency` at once in all, 4 by default).
- A cluster's head (its timestamp) is read only where the block can't be told
  without it. One cluster in sixteen is checked; a file whose blocks aren't
  where their cues say has every head read.
- A track's extraction runs on its own. Every request for the track waits for
  the one under way (a `partial` one a few seconds at most, and not at all
  once FFmpeg reads the file for it). It goes on to the cache when they stop
  waiting.
- A track the Cues couldn't give goes straight to FFmpeg next time (the last
  thousand such tracks are remembered).

Limits (the read runs inside the transcoder, unlike FFmpeg's own process):

- What it keeps takes at most 64 MiB: the file's seek heads, info, tracks,
  Cues and tags as it reads them; 256 bytes per block the Cues list for the
  track (counted as the Cues are read, so a file listing millions of tiny
  blocks is turned down before any is read); the blocks and what they inflate
  to; the WebVTT made of them.
- No block may be bigger than 1 MiB or inflate past 10 MB.
- An ASS header may define at most 10,000 styles.
- The read takes at most a minute, finding the Cues and converting included.
- The Cues are looked for in the seek heads and before the first cluster,
  never cluster by cluster. Each block must lie in its cluster, after the one
  before.

Output:

- Byte for byte what FFmpeg's `-c:s webvtt` makes of the same track: the
  SubRip, ASS, plain text and WebVTT decoders and the WebVTT encoder (bold,
  italics, underline and line breaks; quirks included) are followed step by
  step.
- Checked against FFmpeg on real files by an ignored test:

  ```
  PLEY_TEST_MKV=<file> cargo test -p pley-transcoder cues_match_ffmpeg -- --ignored --nocapture
  ```

#### FFmpeg fallback

FFmpeg extracts the tracks instead when anything is out of the ordinary:

- the file isn't Matroska, or its Cues aren't found as above;
- its tracks don't line up with ffprobe's streams;
- its Cues don't list the subtitles, or not with their places;
- mkvmerge's own statistics tags disagree with the Cues on how many blocks a
  track has;
- a cue points at no block, at another track's, or at one that breaks the
  limits above;
- the track is encrypted, compressed with bzip2 or LZO (zlib and header
  stripping are read) or in two ways;
- the track has a timing of its own (a timestamp scale or a codec delay),
  laced blocks, or cues before the start or out of order;
- the track is WebVTT as mkvmerge writes it (`S_TEXT/WEBVTT`; WebM's
  `D_WEBVTT` is read);
- the file's timestamps aren't whole milliseconds and it has a start time (as
  ffprobe gives every file with audio or video). Pley's media info keeps that
  start in whole milliseconds, while FFmpeg takes it off to the microsecond.

How the fallback runs:

- Every text track of the file comes out in a single pass over it. That may
  take 20 minutes, or as long as reading the file at 10 MB/s takes (two hours
  for 70 GB).
- FFmpeg writes each cue as soon as it is read, so a `partial` request is
  answered with the cues found so far.
- Two such passes run at once. The quick FFmpeg runs (thumbnails, cover art,
  subtitle files) never wait for them.
- When a pass fails, one more reads the asked track alone (another track may
  be what FFmpeg couldn't convert), unless it ran out of time.
- A track whose pass failed answers with that error for 30 minutes (the last
  thousand such tracks are remembered): a player asking every few seconds
  hears why, rather than each request reading the whole file again.

#### Subtitle files

- Converted with charset detection (UTF-8, UTF-16 with BOM, or Windows-1252
  as a fallback), judged from their first megabyte.
- A file over 64 MB is not a text subtitle file (a `.sup`, say): it is
  refused (`400`) before it is read.

## Audio fingerprints

`POST /v1/fingerprint` returns `Fingerprint {start_ms, item_seconds, data}`:
raw Chromaprint values of part of the audio. The Pley server compares
episodes' fingerprints to find shared intros and credits.

| Request field | Meaning |
|---|---|
| `path` | the file |
| `stream_index?` | audio stream (default: the first) |
| `start_ms` | where to start, from the beginning of the file |
| `duration_ms` | how much to analyze; the end of the file may stop it earlier |

- Runs FFmpeg's `chromaprint` muxer (`-f chromaprint -fp_format raw`) on the
  audio downmixed to mono 11025 Hz, which is what Chromaprint analyzes.
- Each value covers 1365 samples (0.124 s, `item_seconds`) and is returned as
  a plain number.
- Requests run two at a time, at below-normal process priority, and pause
  while a playback starts or a transcode runs (see
  [Background work and playback](#background-work-and-playback)).
- `capabilities.chromaprint` says whether the FFmpeg build includes
  Chromaprint.

### Key frames

`POST /v1/frames` takes `{path, start_ms, duration_ms}` and returns `Frames
{frames: [{ms, black, ylow, yhigh, ymax, sat}]}`: how dark the key frames of
part of the video are and how much contrast they have. The server looks for
credits in them: text on black.

- Decodes only the key frames of the first video stream (`-skip_frame nokey`),
  shrunk to 320 pixels wide in 8 bits (HDR too).
- Measures each with `blackframe` (`black`: percent of pixels at most 32) and
  `signalstats` (`ylow`, `yhigh`: luma of the darkest and brightest tenth;
  `ymax`: the maximum; `sat`: average saturation). Luma is 8-bit studio range
  (black is 16).
- Shares the fingerprints' limit and priority.

## Reading ahead

`POST /v1/prefetch` reads a file the way a playback is about to, before it
does. The server asks for it when an item's page opens, or a card's Play
button has the pointer or the focus, at most every two minutes per file. On a
network share a cold 4 s of video takes 0.3 to 3 s to read, and a few
milliseconds once it is in the share's or this host's cache.

| Request field | Meaning |
|---|---|
| `path` | the file |
| `start_ms?` | where the playback would start (0 or none: the beginning) |

It reads, in this order, each byte once:

| `part` | What |
|---|---|
| `head` | the first 4 MB (headers, the first clusters or chunks) |
| `index` | the Matroska Cues or the MP4 movie box (`moov`), wherever it is, 64 MB at most. Matroska Cues are found through the seek heads and the elements before the first cluster only, never by walking the clusters. |
| `start` | for `start_ms > 0`: from the keyframe at or before the start of the 4 s segment holding `start_ms` (where a transcode decodes from; a remux or a direct play starts there or later) to the first keyframe 4 s past `start_ms`, 2 to 40 MB |

- The keyframes' places come from the index: the Cues' `CueClusterPosition`,
  or the video track's `stco`/`co64` chunk offsets and `stsc` samples per
  chunk. Files with neither get their head only.
- Ranges are read in 2 MB pieces, four at a time, each reader with its own
  handle.
- A keyframe index for remuxing is kept from the index read.

The answer is `Prefetched {ranges: [{part, start, end}], bytes, elapsed_ms,
skipped?, stopped}` (`end` exclusive).

It is background work:

- One file at a time, one request per file at once (another answers
  `skipped`), ten seconds at most, waiting included.
- None starts within 5 s of a job creation or while a playback starts
  (`skipped`).
- A playback that begins meanwhile stops the reading (`stopped`), as does the
  request going away.

## Background work and playback

These read whole files or long stretches of them, from the disk or network
share that playbacks read too:

- seek previews (`/v1/trickplay`)
- fingerprints and key frames (`/v1/fingerprint`, `/v1/frames`)
- keyframe scans (`/v1/keyframes`)
- a library scan's probes (`/v1/probe`), which read the heads of many files

A 4K remux on a share needs all of the bandwidth to start in a second or two.
So while a playback starts, none of them starts (probes wait), and the FFmpeg
or FFprobe runs under way are paused, then resumed. Pausing suspends the
process (`NtSuspendProcess` on Windows, `SIGSTOP` on Linux; elsewhere runs only
wait to start). Nothing is lost: a paused run's time limit counts only the
time it ran.

A playback is starting:

- while a job is being created (probe, keyframe index, FFmpeg's start);
- while one of its FFmpeg processes (at Play, a seek, a restart) hasn't written
  its first 3 segments, 20 s at most;
- for `ms` after `POST /v1/hold`, which the Pley server sends for a direct play
  (10 s), since it serves those itself.

Transcodes come first too:

- While a job's FFmpeg or a conversion runs (it holds a transcode slot,
  `running_processes`), the same background runs wait and are paused, until
  5 s after the last one ends, so a seek's restart doesn't let them through.
- A job's FFmpeg paused far ahead of its player still counts; one stopped for
  an idle player doesn't.
- Probes and reading ahead wait for playback starts only: they read little,
  and new files or the next playback shouldn't wait for a film to end.
- The Pley server also waits for transcodes before it asks for seek previews
  or fingerprints.

Priorities: background runs and conversions start at below-normal process
priority (`nice` 10 on Linux), a job's FFmpeg at normal priority.

`/v1/status` counts the runs paused now (`background_paused`).

## Metrics

`GET /v1/status` returns:

- totals: jobs created and failed, processes started and failed, segments
  produced and served, bytes served, probes (with total probe time),
  thumbnails, subtitle extractions, keyframe scans
- running and hardware processes against their limits
- background FFmpeg runs paused for playback (`background_paused`)
- work folder disk use, and segments in memory (`memory_bytes`)
- per job: state, video and audio plans (input → output, encoder, decoder,
  tone mapper), FFmpeg progress (fps, speed, bitrate, position, dropped and
  duplicated frames), restarts, bytes served, warnings, the last error, and
  whether its segments are in memory (`in_memory`, `memory_bytes`)

`GET /metrics` exposes the same data for Prometheus:

| Metric | Type | Meaning |
|---|---|---|
| `pley_transcoder_jobs_created_total` | counter | jobs created |
| `pley_transcoder_jobs_failed_total` | counter | jobs whose FFmpeg failed |
| `pley_transcoder_processes_started_total` | counter | FFmpeg transcode processes started |
| `pley_transcoder_process_failures_total` | counter | processes that exited with an error |
| `pley_transcoder_segments_produced_total` | counter | HLS segments produced |
| `pley_transcoder_segments_served_total` | counter | HLS segments served |
| `pley_transcoder_bytes_served_total` | counter | bytes of segments served |
| `pley_transcoder_probes_total` | counter | probes |
| `pley_transcoder_probe_failures_total` | counter | failed probes |
| `pley_transcoder_probe_milliseconds_total` | counter | time spent probing |
| `pley_transcoder_thumbnails_total` | counter | thumbnails |
| `pley_transcoder_subtitle_extractions_total` | counter | subtitle extractions |
| `pley_transcoder_keyframe_scans_total` | counter | keyframe index scans |
| `pley_transcoder_uptime_seconds` | gauge | seconds since start |
| `pley_transcoder_running_processes` | gauge | FFmpeg transcode processes running |
| `pley_transcoder_hardware_processes` | gauge | of those, using a hardware encoder |
| `pley_transcoder_max_jobs` | gauge | `--max-jobs` |
| `pley_transcoder_jobs` | gauge | jobs known |
| `pley_transcoder_work_dir_bytes` | gauge | disk used by segments and caches |
| `pley_transcoder_background_paused` | gauge | background runs paused for playback |
| `pley_transcoder_job_speed{job}` | gauge | real-time multiplier of a running job |
| `pley_transcoder_job_fps{job}` | gauge | frames per second of a running job |
| `pley_transcoder_jobs_by_state{state}` | gauge | jobs per state |

The Pley web app shows these under *Settings → Dashboard*, next to the
processor and memory use the server samples itself every 2 s
(`GET /api/system/resources`, see [API.md](API.md#system-admin)). A managed
transcoder and its FFmpeg processes count there as Pley's (the server's
process tree); an external one doesn't.

## Benchmark

`scripts/bench-transcoder.py` (Python 3.9 or later, no modules to install)
runs the transcoder as built, over its API, on local files.

For each job it reports:

- the wait for the first segment and, for a seek, for the one asked for;
- the speed (× real time);
- processor time per media second, and the processors' use;
- the largest process's peak memory (on Windows its private commit, on Linux
  its resident set), and all processes' together (Windows);
- the bytes made.

Options:

- `--make-clips DIR --source <4K HDR file>` cuts the clips it needs from a
  film: 1080p H.264 with AAC, TrueHD and DTS copies, ten-minute loops, a
  minute of PGS subtitles from FFmpeg's FATE sample, an SRT.
- `--variants` plays cheaper computers: `gpu` (what the machine has), `sw` (no
  GPU), `sw4` and `sw2` (no GPU, four or two processors: a Windows job
  object's affinity or `sched_setaffinity` on Linux, one logical processor per
  core).
- `--mem-mb` sets a memory limit (Windows).
- `--bin name=path` (repeated) compares builds. They run in turns that change
  order each round, so a machine busy with other work weighs on each alike.
- `--session` sets the seconds of the real-time playback cases.

Cases: 4K HDR10 to 1080p SDR; 1080p to 720p; TrueHD to AAC and DTS to AC-3
with the video copied; a remux; a seek; text and picture subtitles drawn on
1080p and 4K HDR; and playbacks watched in real time (`session-*`) with
throttling on, which count what was made and whether the player waited.

```
python scripts/bench-transcoder.py --make-clips D:/bench --source 4k-hdr10.mkv
python scripts/bench-transcoder.py --media D:/bench --hdr 4k-hdr10.mkv \
    --bin before=old/pley-transcoder.exe --bin after=target/release/pley-transcoder.exe \
    --variants gpu,sw,sw4 --repeat 3 --out results.jsonl
```

Results on a 12-core Ryzen with an RTX 5070 (`sw4` standing in for a small
computer):

- 4K HDR10 to 1080p tone mapped with zimg at the output's size: 1× real time
  on four processors, 3.7 processor seconds per second of video (0.3× and 5.9
  when zimg tone mapped at full size).
- Picture subtitles on 4K laid on the scaled frame: 0.72× (0.23× on the full
  frame), and half the processor time with the GPU's pipeline.
- Three minutes watched in real time: 26 to 49 % less processor time with the
  pause 90 s ahead, the player never waiting.

## License

`pley-transcoder --license` prints:

```
pley-transcoder  Copyright (c) 2026 the Pley authors. All rights reserved.
Restricted: its licence terms are to be determined, and no licence to copy,
modify or distribute it is granted until then. It comes with ABSOLUTELY NO
WARRANTY. See the LICENSE file.
```

FFmpeg is run as a separate program and has its own license. The FFmpeg build
used during development (`--enable-gpl --enable-version3`) is GPLv3.
