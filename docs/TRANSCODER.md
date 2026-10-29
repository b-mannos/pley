# pley-transcoder

`pley-transcoder` is the part of Pley that runs FFmpeg. It:

- probes media files
- transcodes and remuxes video into HLS, and converts whole files
- tone maps HDR10, HLG and Dolby Vision to SDR, or keeps HDR in HEVC Main 10
- burns in and extracts subtitles
- makes thumbnails, seek previews and embedded cover art
- indexes keyframes

It is a separate program. The Pley server runs
it as a child process, or connects to one elsewhere, and talks to it only
through the HTTP API below. The request and response types live
in the `pley-transcoder-api` crate (`packages/transcoder-api`).

## Running

```
pley-transcoder [OPTIONS]
```

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `--listen <ADDR>` | `PLEY_TRANSCODER_LISTEN` | `127.0.0.1:7540` | Address to listen on. Port 0 picks a free port. |
| `--token <TOKEN>` | `PLEY_TRANSCODER_TOKEN` | none | Shared secret, at least 16 characters. Required unless `--listen` is a loopback address. Prefer the env var: command lines are visible to every account. |
| `--work-dir <DIR>` | `PLEY_TRANSCODER_WORK_DIR` | `pley-transcoder` in the user's cache folder | Segments and caches (see [Work folder](#work-folder)). |
| `--ffmpeg <PATH>` | `PLEY_FFMPEG` | `ffmpeg` from `PATH` | FFmpeg binary. |
| `--ffprobe <PATH>` | `PLEY_FFPROBE` | `ffprobe` from `PATH` | FFprobe binary. |
| `--max-jobs <N>` | `PLEY_TRANSCODER_MAX_JOBS` | 8 | Concurrent FFmpeg transcodes (transcode slots). |
| `--max-hw-jobs <N>` | `PLEY_TRANSCODER_MAX_HW_JOBS` | 8 | Concurrent hardware encoder sessions. Extra jobs fall back to software encoding. |
| `--cpu-decode <auto\|never>` | `PLEY_TRANSCODER_CPU_DECODE` | `auto` | Whether the CPU decodes the video of jobs the GPU can't keep ahead (see [Sharing the GPU](#sharing-the-gpu)). `never`: the GPU decodes what it can. |
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
  the folder that holds the server binary, with a random token in the
  environment. It reads the announced address and restarts the transcoder
  with backoff if it dies. The transcoder exits as soon as the server does.
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
  start-up: entries unused past their age limit, then the least recently used
  until the folder fits its budget. Reading an entry counts as using it.
- Temporaries left by a crash go after 3 hours untouched. A subtitle pass that
  runs longer touches its outputs every 10 minutes.

### Authentication

- With a token, every endpoint except `GET /health` requires
  `Authorization: Bearer <token>`.
- Without a token (loopback addresses only), only requests whose `Host` is
  `localhost`, `127.0.0.1` or `[::1]` (any port) are answered. This blocks
  DNS rebinding.
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
| GET | `/metrics` | Prometheus text format |
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
| `gpu_pipelines` | GPU-only decode → scale → encode pipelines, with the codecs each passed for |
| `vaapi_tonemappers` | tone mappers whose frames reach a VAAPI encoder (Linux; see [VAAPI encoders](#vaapi-encoders)) |
| `vaapi_vulkan_decode` | `vulkan_decode` on the Vulkan device made from the VAAPI one |
| `tonemap_speed?` | `{high, balanced, fast}`: frames a second libplacebo tone maps at each quality, 4K to 4K (see [libplacebo quality](#libplacebo-quality)) |
| `amf_upload` | FFmpeg uploads AMF's frames to the AMD GPU (Windows; see [AMF on Windows](#amf-on-windows)) |
| `h264_metadata` | FFmpeg has the `h264_metadata` bitstream filter, which writes the colour primaries `h264_amf` leaves out |
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

- Only keyframes are decoded. HDR and Dolby Vision are tone mapped with
  libplacebo when available.
- `GET /v1/trickplay/{id}/{n}.jpg` returns sheet `n` (`0` to `sheets - 1`).
- A set is cached per file version for a day, or until every sheet has been
  fetched: fetch them all in one go and keep them.

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

- `POST /v1/keyframes` `{path}`: `200` if indexed, `202` if queued (see
  [Remux](#remux)).
- `POST /v1/hold` `{ms}`: `204`. Background work pauses for `ms` (a minute at
  most) while a direct play starts (see
  [Background work and playback](#background-work-and-playback)).
- `POST /v1/prefetch`: see [Reading ahead](#reading-ahead).
- `POST /v1/fingerprint`: see [Audio fingerprints](#audio-fingerprints).
  `400` when FFmpeg lacks Chromaprint.
- `POST /v1/frames`: see [Key frames](#key-frames).

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
`{bitrate_kbps}`, the bitrate the transcode goes on at (capability
`live_bitrate`):

- Kept between 1 Mbps and the job's own bitrate.
- The segments past the last one its player asked for, and the next, are
  deleted and made again at the same size and layout, so the player plays on
  from the same playlist. A running process restarts there at once.
- `400` for a remux.

## Conversions

A conversion makes a `JobRequest` into one Matroska file in a single FFmpeg
run: a download for a device that can't play the original (sound it doesn't
decode, a picture too big for it). Capabilities `conversions`, and
`conversions_mp4` for MP4.

- `POST /v1/conversions` returns `ConversionStatus {id, state, progress,
  duration_ms, size?, error?, video?, audio?}`. Asked again for the same input
  and request, the same conversion (or its file) answers; a failed one starts
  again.
- `GET /v1/conversions/{id}`: `state` is `queued`, `running` (with `progress`,
  0 to 1), `done` (with `size`) or `failed` (with `error`).
- `GET /v1/conversions/{id}/file`: `video/x-matroska` or `video/mp4`, once
  done. One range (`bytes=a-`, `bytes=a-b`) as asked; `416` past the end.

- **Streams:** the same plan as a job's segments, at full speed from the
  start (`throttle` and `start_ms` are ignored).
- **Subtitles:** every subtitle track of the input, in its order, with its
  language and name. MP4's text is made SRT; teletext is left out (Matroska
  can't hold it).
- **Dolby Vision and HDR10+:** a remux whose segments would be edited (Dolby
  Vision converted, HDR10+ left out) has its video encoded instead.
- **Short output:** an FFmpeg run that ends without error but short of the
  media's end (a share that went away) fails.
- **MP4** (`container: "mp4"`), for players that open no Matroska
  (QuickTime): text subtitles as `mov_text`, in order; picture subtitles left
  out; HEVC tagged `hvc1`; index first (`+faststart`). It is a separate
  conversion from the Matroska one, named `<id>.mp4`.
- **Scheduling:** one conversion at a time, in a transcode slot (a hardware
  session when one is free, else the software encoder), at low process
  priority. It waits while every slot is in use.
- **Id:** from the input (path, size, modification time) and the request's
  video, audio and subtitle parts.
- **Abandoned:** a queued conversion nobody asks for or about
  (`GET /v1/conversions/{id}`) for an hour is not made when its turn comes.
  Asking again starts it again.
- **Files:** in `conversions/`. The janitor deletes those unused for a week,
  then the least recently used past 64 GB; one deleted is made again when
  asked for. After a restart the transcoder finds its files by name and
  deletes half-made ones.

## How HLS jobs work

Quality menus, Automatic quality and when players switch are explained from
the user's side in
[docs/QUALITY.md](https://github.com/hhaidar/pley/blob/master/docs/QUALITY.md).
This section is how the transcoder serves them.

### Segment layout

A job's segment layout is fixed when it is created, so the whole VOD playlist
is known up front and players can seek anywhere.

- Segments are `segment_seconds` long (1 to 10 s, 4 s by default). The Pley
  server asks for 2 s, so playback, seeks and quality switches start sooner.
- **Transcodes:** keyframes are forced on that grid.
- **Remuxes** (`video.mode = copy`): segments start on the source's keyframes
  (as many as make at least the segment length), from the keyframe index.
  Keyframe times are relative to the container's start time
  (`MediaInfo.start_ms`, often far from zero in MPEG-TS).
- The layout is deterministic, so a job recreated after a transcoder restart
  lines up.
- A job has at most 200 000 segments; long files get longer segments instead.
- Durations beyond about 9 days are not believed. The probe then uses the
  streams' own durations (Matroska `DURATION` tags included) or the file size
  over the streams' bitrates, and refuses a file with nothing plausible.

### Segments on demand

A segment that exists is served from disk or memory. Otherwise:

- **Wait** when a running FFmpeg process is about to make it: at most 12 s of
  segments ahead of the process, never behind it.
- **Restart instead** when the process is held to the read rate (see
  [Throttling](#throttling)), not slower, and the wait (counted from halfway
  through the segment being made) is longer than this job's last
  start-to-first-segment time plus stopping the old process. Not while
  another request waits for a segment the old process is about to make.
- **Else start a new process** at that segment, with `-ss`,
  `-output_ts_offset`, `-segment_times` and `-segment_start_number`, so
  timestamps stay on the absolute timeline. `avoid_negative_ts` is disabled.
- **Remux seeks** land 140 ms past the boundary keyframe (at most halfway to
  the next) where FFmpeg moves B-frame seeks back by 3/23 s (all containers
  but MP4 and MOV), 1 ms past elsewhere. They use `-noaccurate_seek`, so
  converted audio starts at the keyframe like the copied video.
- A new process stops right before the next segment that already exists.
- A process started less than 3 s ago is not replaced while another request
  waits for it. When that request goes away, the one held back looks again at
  once.

### Completion tracking

- FFmpeg's segment muxer writes a line to stdout as each segment finishes,
  which marks it ready and wakes waiting requests (no polling).
- A remux segment that needs editing is edited first (see
  [Dolby Vision in remuxes](#dolby-vision-in-remuxes)).
- FFmpeg's `-progress` output on stderr gives the live metrics: fps, speed,
  bitrate and position.

### Throttling

Applies when the request's `throttle` is on (`throttle_transcodes` in the
server, on by default). The read rate needs FFmpeg support (`readrate`).

- **Burst, then real time:** a process makes its first 120 s as fast as it
  can, then reads at real time (`-readrate 1`).
- **Catch-up:** a read that falls behind (a slow disk or share) catches up at
  2× where FFmpeg can (`-readrate_catchup`, FFmpeg 8).
- **Pause far ahead:** a process 90 s ahead of the last segment the player
  asked for is suspended (`NtSuspendProcess` on Windows, `SIGSTOP` on Linux)
  until it is less than 60 s ahead, or a request waits for a segment it is to
  make.
- **Speed:** measured over the process's last 4 s of progress (since it last
  resumed). A job's status reports it, and 1× for a process held to the read
  rate.

### Housekeeping

- A process idle for 60 s (paused player) is stopped.
- A job idle for 20 minutes expires.
- Segments more than 6 minutes behind the playhead are deleted, in small
  batches. In memory: more than a minute behind or 3 minutes ahead.
- On startup, segments left from an earlier run are removed, and in memory
  those of transcoders that are gone (a crash).
- Cache pruning: see [Work folder](#work-folder).
- FFmpeg never outlives the transcoder: on Windows it runs in a kill-on-close
  job object; on Linux every child gets a parent-death signal.
- On shutdown (a signal, or stdin closed with `--exit-with-parent`) all jobs
  are stopped, then open requests get 5 s to finish.

### Failure handling

- FFmpeg failing twice at the same segment returns the error. The job's
  `last_error` holds FFmpeg's last log lines.
- A hardware encoder that fails before the first segment (a driver update, a
  GPU gone, a session refused) hands the job to the software encoder, with a
  warning.
- After that happens to three jobs, `capabilities.json` is deleted and the
  next start tests the encoders again.

### Renditions

A transcode asked for with `variants` (adaptive bitrate) also comes in
smaller renditions:

- Each variant is the job's request with its own `max_width`, `max_height`
  and `max_bitrate_kbps`. One is kept only when it is no bigger and has fewer
  bits than the one above. Remuxes and conversions ignore `variants`.
  `JobCreated.variants` says how many were kept.
- They are jobs inside the job, in folders `v1/`…, with segments named
  `v1-{n}.ts`… laid out as the job's. Burned-in text subtitles are extracted
  once and copied to them.
- `master.m3u8` lists them after the job's own, largest first.
- Only the rendition the player asked for last has a running process; the
  others stop (keeping what they made).
- They expire and are cleaned up with the job. The job's status is the
  playing rendition's (`rendition`, `variants`), with what all of them served.

### Bitrates

A transcode's bitrate, unless asked for less, follows its size. Lines are
counted as for a 16:9 picture of the same width (a 1920×800 film gets 1080p's
8 Mbps).

| Lines (16:9) | 2000+ | 1400 | 1000 | 700 | 500 | 400 | less |
|---|---|---|---|---|---|---|---|
| Mbps | 20 | 12 | 8 | 4 | 2 | 1.2 | 0.7 |

### Segments in memory

A job's segments are kept in memory instead of `jobs/<id>/` when the request's
`options.in_memory` is on (`transcode_in_memory` in the server, on by
default), the job is throttled, and the transcoder has room.

- Memory is a folder in RAM: `/dev/shm` on Linux, when it is a tmpfs. The
  transcoder keeps `pley-transcoder-<pid>/<id>/` there (renditions in
  `v1/`…), private to its user and removed at shutdown, or at the next start
  after a crash.
- Each process runs in the folder it writes to, with a copy of the burned-in
  subtitles. Segments are served, edited and pruned there.
- Conversions and the caches stay on disk.
- **Windows and macOS** keep segments on disk (`memory.available: false`,
  "works on Linux only"): they have no folder in RAM by default.

Limits:

- **Per job:** segments are kept a minute behind the playhead and up to 3
  minutes ahead.
- **Budget:** at most a quarter of the computer's memory and half of the
  folder's size (`budget_bytes`). A budget under 256 MB counts as none, so
  Docker's default 64 MB `/dev/shm` is too small: give the container
  `--shm-size` (deploy/docker's compose file gives 2 GB).
- **Reservation:** each job (each rendition) reserves 3 minutes of its
  bitrate: a transcode's plus a quarter, a remux's at the source's, 40 Mbps
  when unknown (4K at 20 Mbps: about 700 MB).
- **Admission:** a process goes to memory when its reservation fits the budget
  beside the others' (or what they take, when more), the folder keeps 64 MB
  free, and the computer keeps a tenth of its memory free (512 MB at least,
  from `MemAvailable`). Otherwise it goes to disk.
- **Running short:** the job with the most in memory moves to disk, one every
  5 s. Its process restarts on disk at the segment it was making (a paused
  one at the next request). What it made stays in memory until pruned.

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
- Results are cached in `capabilities.json`, keyed by the FFmpeg binary's size
  and modification time and by the GPUs present (render nodes and their
  drivers, NVIDIA's driver), so a GPU added later is tested. `--redetect`
  ignores the cache.

Rate control: every encoder is capped at the job's bitrate (`-maxrate`, with a
two-second buffer) and may spend less on easy content. The start-up test uses
the same options.

| Encoder | Rate control |
|---|---|
| NVENC | VBR by quality: `-rc vbr -cq 23` (26 for HEVC) |
| AMF | peak-constrained VBR: `-rc vbr_peak` |
| x264, x265 | CRF 21 and 24 |
| VAAPI | `-rc_mode VBR` (FFmpeg picks CBR when bitrate equals maxrate) |
| VideoToolbox | `-maxrate` kept (`DataRateLimits`, a hard limit over one second) |

AMF HEVC needs no `-header_insertion_mode`: with `-forced_idr 1` FFmpeg sends
the VPS, SPS and PPS with every forced keyframe.

#### AMF on Windows

- FFmpeg uploads the frames to a Direct3D 11 device on the AMD GPU, whichever
  adapter it is, rather than leaving AMF to copy them from system memory,
  which is several times slower. The decoder uses the same device:

  ```
  -init_hw_device d3d11va=dx:,vendor_id=0x1002 -filter_hw_device dx
  -hwaccel d3d11va -hwaccel_device dx ... format=nv12,hwupload
  ```

  libplacebo makes a Vulkan device of its own and gives NV12, as for every
  hardware encoder. Only where the start-up test passed (`amf_upload`). A
  software encoder standing in for AMF, and OpenCL tone mapping, take frames
  from memory.
- `h264_amf` writes no colour primaries, so
  `-bsf:v h264_metadata=colour_primaries=…` adds them: BT.709 after tone
  mapping, else the source's own, none when the source has none. Only where
  FFmpeg has the filter (`h264_metadata`); `hevc_amf` writes them itself.
- 8-bit SDR video stays on the GPU throughout (decoded into Direct3D 11,
  scaled by `scale_d3d11`, encoded from there) for each codec whose pipeline
  passed its start-up test (`gpu_pipelines`, `amf`). Upstream FFmpeg 9.0's
  `scale_d3d11` fails that test on Vega graphics, which then use the upload
  above.

### Hardware decoding

- The decoder is paired with the encoder: `cuda` with NVENC, `d3d11va` with
  QSV or AMF on Windows, `vaapi` with VAAPI, `videotoolbox` with VideoToolbox.
- Frames come back to system memory. FFmpeg falls back to software decoding
  when the GPU doesn't decode the codec.
- A job whose hardware decoder failed to start ("Failed setup for format …")
  shows `cpu` as its decoder, with a warning.
- **Scaling on the GPU first.** Frames that come to memory for burned-in
  subtitles, zimg or OpenCL tone mapping, or x264/x265 (including the
  software stand-in) are first scaled by the GPU that decoded them, so only
  the output's pixels are copied.
  - CUDA: `-hwaccel cuda -hwaccel_output_format cuda` and
    `scale_cuda=w=…:h=…:format=nv12,hwdownload,format=nv12`; 10 bits
    (`p010le`) for tone mapping and kept HDR.
  - VAAPI: `scale_vaapi`, 8 bits only.
  - Only where the decoder's zero-copy pipeline passed its start-up test for
    the codec (`gpu_pipelines`). Not for interlaced video (deinterlaced at its
    own size first) or libplacebo (which scales as it tone maps).
  - Mesa's D3D12 VAAPI driver (WSL) crashes in `scale_vaapi`, so it fails the
    test and its frames come to memory whole.
- **Vulkan decoding** (into libplacebo) counts for a codec only when the
  start-up test's frames reach the filters in Vulkan memory (`pixfmt:vulkan`
  in FFmpeg's verbose log). GPUs whose driver lacks Vulkan video decoding
  (AMD's for Vega graphics on Windows, say) decode with the encoder's decoder
  instead.
- NVDEC runs without `-hwaccel_flags +unsafe_output`.

### Tone mapping

HDR10, HLG and Dolby Vision are tone mapped to SDR BT.709 by the best
available tone mapper:

| Order | Tone mapper | Runs on | Notes |
|---|---|---|---|
| 1 | `libplacebo` | GPU (Vulkan) | Applies Dolby Vision RPU reshaping, so **profile 5** (IPTPQc2) has correct colors. |
| 2 | `tonemap_opencl` | GPU (OpenCL) | |
| 3 | `zscale` + `tonemap` (zimg) | CPU | Scales to the output size first. |

- With Vulkan on the CPU only (Mesa's llvmpipe), libplacebo comes last; only
  profile 5 picks it.
- libplacebo tone maps by the stream's own metadata (`peak_detect=0`): HDR10's
  mastering display and content light levels, Dolby Vision's and HDR10+'s per
  scene. This keeps segments from different FFmpeg processes identical at the
  seams. Thumbnails and seek previews do the same.
- `tonemap_opencl`'s peak detection (63 frames) can't be turned off, so its
  brightness can jump between processes. zimg's `tonemap` has none.
- Dolby Vision profiles 7 and 8 have an HDR10- or HLG-compatible base layer
  and work with any tone mapper.
- Profile 5 without libplacebo gives a warning: its frames are tagged BT.2020
  PQ for zimg and OpenCL, and play with off colors. The same tagging fills in
  whatever an HDR stream leaves untagged.
- libplacebo caches compiled shaders in the work folder's `libplacebo/`
  (saving about 0.2 s at every start, seek and quality switch), unless the
  folder's path holds a character FFmpeg's filter syntax reads
  (`, ; [ ] ' :`, other than a drive's colon).

#### libplacebo quality

libplacebo runs at one of three qualities (`video.tonemap_quality` in the
job's plan):

| Quality | Options beyond its defaults |
|---|---|
| `high` | none |
| `balanced` | `upscaler=bilinear:contrast_recovery=0` |
| `fast` | balanced's, plus `downscaler=bilinear:dithering=none:gamut_mode=clip` |

- `fast` adds `skip_aa=1` where the output is at least half the source's
  width and height: the downscaler samples the source directly (2×2 averaged
  at exactly half). Its SDR output also adds
  `extra_opts=force_low_bit_depth_fbos=yes`: the one picture left between
  libplacebo's passes, the SDR output before it is split into planes, is
  kept in 8 bits.
- At startup, after the other tests, each quality is timed on synthetic 4K
  PQ frames made 4K SDR (`tonemap_speed`), from `fast` to `high`, for 20 s at
  most. A quality not reached in that time is never used. Where time allows,
  each runs twice and the faster run counts.
- A process takes the best quality that measured at least twice what the
  jobs tone mapping with libplacebo need together, itself included. Each
  needs its frame rate × its size relative to 4K to 4K (0.6 × the source's
  pixels + 0.4 × the output's). So a 4K film made 1080p beside one kept 4K
  needs less of the GPU than two kept 4K. When none did, the fastest.
  Renditions start at their job's quality.
- Without a measurement (no libplacebo on a GPU), `high`.

#### Sharing the GPU

Several jobs tone mapping with libplacebo share the GPU:

- **Decoding on the CPU.** A job decodes on the CPU when all of these
  hold, and its status then says `decoder: cpu`:
  - its GPU would decode into memory for libplacebo and encode on the same
    video engine (AMF, VAAPI, Quick Sync; not NVIDIA's);
  - no quality measured fast enough for the jobs tone mapping (the rule
    above), or the jobs fell behind even at `fast` (below): the GPU falls
    behind;
  - the CPU keeps 2 cores free after its decoders (4 cores each for 4K at
    24 fps, in proportion to pixels and frame rate) and what other programs
    keep busy, measured over the last seconds (Windows: the system's busy
    time less the transcoder's and its FFmpeg's; Linux: `/proc/stat` less
    the transcoder and its children). A job decoding on the CPU already
    doesn't count other programs again, so it doesn't move back and forth;
  - no software encode runs;
  - `--cpu-decode` isn't `never`.
- **What the jobs make.** The start-up measurement is checked against what
  the jobs make. A process that has run 30 s is judged by its speed over
  the last 20 s. It falls behind under 1.05× real time with less than 8 s
  ready past what its player asked for:
  - with another job tone mapping alike (the same quality, decoding on the
    same side) making at least twice as much of its own work (speed × what
    it needs, as above), it waits for something of its own (its file's hard
    disk, read by another program too) and teaches nothing. A job the CPU
    decodes for gets more done for that alone, so jobs decoding on the
    other side don't count;
  - with the CPU at least 90 % busy, the CPU has no time to spare: the jobs
    decode on the GPU;
  - else the GPU fell behind at its quality: a cheaper one, and at `fast`
    the CPU decodes where it has the cores.

  A lesson holds while the jobs tone mapping need at least 0.9 of what they
  did when it was learned, for 10 minutes after it was last confirmed. Then
  the start-up measurement is tried again. Each is logged once, at `info`.
- **Moving running jobs.** A job already running moves to the quality and
  decoding that the GPU's load calls for. It moves at once when another job
  starts using the GPU (checked again every 5 s), when a lesson calls for
  less work on the GPU, or back to decoding on it when the CPU has no time
  to spare. It moves back to more work on the GPU only after its process
  has run 60 s and is at least 30 s of video ahead of the player. Each move
  restarts FFmpeg at the next segment. A paused process moves once it
  resumes.
- The job's status gives its latest process's `tonemap_quality`.
- Measured on a Ryzen 5 5600G (Radeon Vega 7) with two 4K HDR films and a
  player each: kept 4K, 1.20× to 1.28× real time each, decoding on the
  CPU; made 1080p, 1.57× to 1.74×, decoding on the GPU. Three kept 4K make
  55 to 60 frames a second together, short of real time for each: its
  memory's bandwidth is what they share.

#### VAAPI encoders

Before a VAAPI encoder (Intel and AMD graphics on Linux), the tone mapper's
device is made from VAAPI's, and the frames are uploaded back to it:

```
-init_hw_device vaapi=va:<render node> -init_hw_device vulkan=vk@va
-filter_hw_device vk ... hwupload=derive_device=vaapi
```

(`opencl=ocl@va` for OpenCL.)

- A Vulkan device made from VAAPI's is the same GPU where VAAPI says which it
  is (VA-API 1.15's PCI id, or AMD's name), else Vulkan's first device.
- An OpenCL device can be made from VAAPI's only by Intel's OpenCL
  (`cl_intel_va_api_media_sharing`), not AMD's.
- At startup each GPU tone mapper is tried this way into `h264_vaapi`
  (`vaapi_tonemappers`), and Vulkan decoding per codec
  (`vaapi_vulkan_decode`).
- Jobs with a VAAPI encoder use only the tone mappers that passed, and zimg.
  Where none is left, or Dolby Vision profile 5 would lose libplacebo's
  colors, the job encodes with x264, with a warning.
- HDR kept in HEVC that libplacebo would convert is tone mapped instead when
  libplacebo's frames don't reach VAAPI's encoder.

### HDR kept

With `video.codec = hevc` and `video.dynamic_range` `hdr10` or `hlg`, HDR
video is encoded in HEVC Main 10 instead of being tone mapped.

- Each HEVC encoder is also test-encoded in 10 bits (`-profile:v main10`,
  P010; x265 with `yuv420p10le` and `-dolbyvision 0`) and marked `main10`.
- **Same range** (HDR10, HDR10+, Dolby Vision 7 and 8.1 as their HDR10 base
  layer for `hdr10`; HLG and Dolby Vision 8.4 for `hlg`): decoded, scaled to
  P010 (`scale_cuda` on the GPU where the CUDA pipeline handles the codec,
  else the CPU) and encoded.
- **Other range** (HLG or Dolby Vision 8.4 to HDR10, PQ to HLG, Dolby Vision
  profile 5 to either): libplacebo with a 10-bit BT.2020 target
  (`format=p010`, `color_trc=smpte2084` or `arib-std-b67`), on the Vulkan
  pipeline when the GPU decodes into Vulkan memory.
- **Metadata:**
  - HDR10 mastering display and content light levels pass to the encoder,
    which writes them as SEI with every frame (NVENC; x265 with
    `repeat-headers=1`), so every segment starts with them.
  - PQ or HLG, BT.2020 primaries and matrix are set on the encoder
    (`-color_trc`…).
  - Dolby Vision RPUs are never written: Dolby Vision comes out as its base
    layer, or converted when it has none.
  - HDR10+ dynamic metadata is not kept.
- **Falls back to SDR**, with a warning, when H.264 is asked for, subtitles
  are drawn onto the video, there is no `main10` encoder, or a range change
  needs libplacebo and it is missing.
- When hardware sessions run out, an HDR job's software fallback is a `main10`
  x265.
- The master playlist names the output `hvc1.2.4.LLL.B0` with
  `VIDEO-RANGE=PQ` or `HLG`. The plan's `dynamic_range_out` says `hdr10`,
  `hlg` or `sdr` for transcodes.

### Frame grabs

`/v1/thumbnail` with `hdr` tone maps Dolby Vision profile 5 through
libplacebo and other HDR through zimg. If tone mapping fails (a Vulkan device
that went away, say), the frame is grabbed as it is.

### Subtitle burn-in

- **Picture subtitles** (PGS, VobSub, DVB, `.sup` files) are overlaid on the
  scaled, tone-mapped frame, their canvas fitted to its width. They are
  blended on the CPU, also in Vulkan pipelines (after the one download).
- **Text subtitles** (SRT, ASS) are rendered by libass from an `.ass` file
  extracted into the job folder when the job is created.
  - Extracting an embedded track reads the whole file: it gives up after 20
    minutes (the server waits 25 for the job). A job folder whose job isn't
    made after all is removed.
  - Drawn with the fonts attached to the file (without them libass falls back
    to a system font). Fonts are read from a Matroska file's Attachments
    element while the track is extracted, and kept in
    `subtitles/<file key>.fonts/` (256 MB at most per file) for later jobs of
    the same file version, passed to the filter as `fontsdir`.
  - Subtitle files next to the video are drawn with the video's fonts too.

### Audio

**Copy:** `audio.mode = copy` copies AAC, MP3, AC-3 and E-AC-3. Anything else
asked to be copied is converted, with a warning.

**Conversion** uses FFmpeg's own encoders (in every build):

| `codec` | Encoder | Channels | Default bitrate (kbit/s) |
|---|---|---|---|
| `aac` (default) | `-c:a aac` | stereo unless `channels` says more, up to 7.1 | 192 stereo, 64 per channel above |
| `eac3` (Dolby Digital Plus) | `-c:a eac3` | at most 5.1 (`-ac 6`) | 192 stereo, 640 above |
| `ac3` (Dolby Digital) | `-c:a ac3` | at most 5.1 (`-ac 6`) | 192 stereo, 640 above |

- `channels` never goes above the source's.
- `bitrate_kbps` overrides the default, within 64 to 640.
- Audio not at 44.1 or 48 kHz is resampled to 48 kHz.
- The job's audio plan reports the codec as `codec_out`.
- Which codec the Pley server asks for per player: *Version 2* in
  [docs/API.md](https://github.com/hhaidar/pley/blob/master/docs/API.md#version-2).
- **Lossless sources** are decoded only as far as needed: DTS-HD Master Audio
  to 5.1 or fewer channels from its lossy core (`-core_only`); TrueHD to
  stereo or 5.1 from the matching substream (`-downmix stereo` or `5.1`).
- **Downmix to stereo:** 5.0, 5.1 and 7.1 use Pley's own matrix
  (`DOWNMIX_51`, `DOWNMIX_71` in `plan.rs`) instead of `-ac 2`: FFmpeg's
  default matrix (fronts whole, surrounds at -3 dB, LFE dropped) but the
  center at 0 dB for dialogue, then a limiter at -2 dBFS (`alimiter`, its
  look-ahead compensated). The layout is made 5.1 (or 7.1) first, so 5.0 and
  broadcasts switching to stereo still map. TrueHD decoded to stereo by its
  own decoder, and other or unknown layouts, use `-ac 2`.
- **Copied sound beside transcoded video:** a process that starts past 0
  drops audio packets before the seek point (`-copypriorss:a 0`), so the
  sound doesn't overlap the previous segment. A remux keeps its sound from
  the keyframe its video starts on.

### Remux

H.264 and HEVC video is copied into HLS, with segments cut exactly on the
source's keyframes. The keyframe index comes from:

- **Matroska and WebM:** the Cues element, read instantly, so remuxing works
  on the first play.
- **MP4, MOV and M4V:** the video track's sample tables (`stss`, `stts`,
  `ctts`) and edit list, or the fragments' sample flags in fragmented files
  (CMAF, OBS recordings). Times match FFmpeg's packet timestamps, edit lists
  included. Several edits, sync points in sample groups, inconsistent tables,
  or fragments that take more than 2 s to walk fall back to a packet scan.
- **Other containers:** a packet scan in the background, one file at a time,
  after `POST /v1/keyframes` (the server sends it when an item page opens).
  Until the index exists, jobs transcode.

Formats are recognized by content, not by file name.

### Turned pictures

Phone video is often stored landscape with a display matrix that turns it.

- The probe reads the angle (`MediaStream.rotation`: 90, -90, 180).
- A transcode is planned at the shown size (1920×1080 turned a quarter is
  1080×1920; 608×1080 within 1080p).
- FFmpeg turns only frames in system memory (`-autorotate`), so turned video
  uses the CPU's filters, hardware decoded into memory (no zero-copy, Vulkan
  or GPU scaling).
- MPEG-TS can't say a picture is turned, so a remux asked for is transcoded
  instead, with a warning.
- Media info probed before rotation was read has no `rotation`; such files are
  planned unturned until probed again.

### Frames timed as they are decoded

Some MP4s (FFmpeg muxing a raw H.264 or HEVC stream) give each frame its
decode time as its display time. Players that trust these times show frames
out of order or drop them, and copying the video keeps the problem.

- The probe of an MP4 or MOV file reads the times of its first 128 packets.
- When a stream with `has_b_frames` shows every packet at its decode time, its
  first 48 packets are decoded. If more than one frame comes out earlier than
  the frame before it, the stream is marked `decode_order_times`.
- The server then transcodes its video.

### Dolby Vision in remuxes

A copied HEVC stream keeps its Dolby Vision RPUs (NAL type 62) in the MPEG-TS
segments, which can't signal them, so `master.m3u8` does, as
`VideoRequest.dolby_vision` asks:

| `dolby_vision` | `master.m3u8` |
|---|---|
| `copy` (default) | the base layer's codec only (`hvc1.2.4.L153.B0`) |
| `signal` | Profile 5: `dvh1.05.LL` alone. Profile 8: the HEVC codec with `SUPPLEMENTAL-CODECS="dvh1.08.LL/db1p"` (8.1, `VIDEO-RANGE=PQ`), `/db4h` (8.4, `HLG`) or `/db2g` (8.2, `SDR`). Profile 7: `dvh1.07.LL`. |
| `to81` | profile 7 made 8.1 and named so (below); other profiles as `signal` |

`to81`: FFmpeg copies the stream, and the transcoder edits each finished
segment before it is ready.

- The enhancement layer's NAL units (type 63) are left out.
- Each RPU is rewritten as `dovi_tool`'s mode 2 does: no residual data (NLQ),
  no enhancement-layer resampling, one partition, the P8.1 color coefficients
  (BT.2020 YCbCr to RGB, RGB to LMS, `signal_color_space` 0), and no-op
  mapping curves for a full enhancement layer. Extension blocks (levels 1 to
  11, 254, 255; content mapping 2.9 and 4.0) and the rest are kept; the CRC
  is recomputed.
- The code is the transcoder's own (`src/dovi.rs`); nothing of FFmpeg's or
  `dovi_tool`'s is linked or run.
- An RPU that can't be read (or has a wrong CRC) is left out with a warning on
  the job, and that frame plays as its base layer.
- Single-track profile 7 (as MakeMKV writes it) is supported. A disc's
  dual-PID streams carry the RPU in the enhancement-layer PID, which isn't
  copied.

`strip_hdr10plus` removes the HDR10+ messages (ITU-T T.35, country 0xB5,
provider 0x003C, application 4) from each segment's prefix SEI NAL units,
keeping the others (mastering display, content light level, captions); an SEI
unit left empty goes. It is for Dolby Vision decoders that show black or
freeze with both (androidx/media#1895).

Segment editing re-packetizes the video PES: the first TS packet keeps its
adaptation field (PCR, random access), continuity counters run on, and other
PIDs' packets are left as they were. A 2 s 4K segment takes milliseconds. The
job's video plan reports `dolby_vision` (`copy`, `signal`, `to81`) and
`hdr10plus_stripped`.

### HDR10+ and profile 7's enhancement layer

- The probe of a Dolby Vision or PQ stream decodes its first frames
  (`-read_intervals %+#12 -show_entries frame_side_data`) and records
  `MediaStream.hdr10plus`; a PQ stream with HDR10+ becomes
  `dynamic_range: hdr10plus`.
- For profile 7, the same frames say what the enhancement layer holds
  (`MediaStream.dv_el`): `mel` (minimal) when its residual is zero in every
  component (no NLQ offset, an input maximum of one, no dead zone), else
  `fel` (full).
- Both are absent when the frames can't be read, and for other streams. The
  job's video plan repeats them (`dv_el`, `hdr10plus`).

### Subtitle extraction

Text tracks (SubRip, ASS/SSA, plain text and WebVTT) become WebVTT, cached per
file version.

**Matroska, through the Cues.** The Cues list every subtitle block with its
place in its cluster (mkvmerge and FFmpeg write them so), so the track is read
block by block: one disk seek per subtitle instead of reading the whole file.

- Sixteen reads at once. A file's tracks are read side by side (half of
  `--probe-concurrency` at once in all, 4 by default).
- A track's extraction runs on its own. Requests for the track wait for it (a
  `partial` one a few seconds at most, and not at all once FFmpeg reads the
  file for it), and it goes on to the cache when they stop waiting.
- A track the Cues couldn't give goes straight to FFmpeg next time (the last
  thousand such tracks are remembered).
- Limits: at most 64 MiB kept (headers, Cues, 256 bytes per listed block, the
  blocks, what they inflate to and the WebVTT); no block over 1 MiB or
  inflating past 10 MB; at most 10,000 ASS styles; a minute in all. The Cues
  are looked for in the seek heads and before the first cluster only. Each
  block must lie in its cluster, after the one before.
- The output is byte for byte what FFmpeg's `-c:s webvtt` makes of the same
  track (bold, italics, underline, line breaks and quirks included). An
  ignored test checks it against FFmpeg on real files:

  ```
  PLEY_TEST_MKV=<file> cargo test -p pley-transcoder cues_match_ffmpeg -- --ignored --nocapture
  ```

#### FFmpeg fallback

FFmpeg extracts the tracks instead when:

- the file isn't Matroska, or its Cues aren't found as above;
- its tracks don't line up with ffprobe's streams;
- its Cues don't list the subtitles with their places;
- mkvmerge's statistics tags disagree with the Cues on a track's block count;
- a cue points at no block, at another track's, or at one past the limits;
- the track is encrypted, compressed with bzip2 or LZO (zlib and header
  stripping are read) or in two ways;
- the track has its own timestamp scale or codec delay, laced blocks, or cues
  before the start or out of order;
- the track is `S_TEXT/WEBVTT` (mkvmerge's; WebM's `D_WEBVTT` is read);
- the file's timestamps aren't whole milliseconds and it has a start time.

Then:

- Every text track comes out in one pass over the file, with a time limit of
  20 minutes or the time to read the file at 10 MB/s, whichever is longer
  (two hours for 70 GB). A `partial` request gets the cues found so far.
- Two such passes run at once; quick FFmpeg runs (thumbnails, cover art,
  subtitle files) never wait for them.
- When a pass fails, one more reads the asked track alone, unless it ran out
  of time.
- A track whose pass failed answers with that error for 30 minutes (the last
  thousand such tracks are remembered).

#### Subtitle files

- Charset detected from the first megabyte: UTF-8, UTF-16 with BOM, or
  Windows-1252 as a fallback.
- A file over 64 MB is refused (`400`) before it is read.

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
  audio downmixed to mono 11025 Hz.
- Each value covers 1365 samples (0.124 s, `item_seconds`).
- Two requests run at a time, at below-normal priority, paused while a
  playback starts or a transcode runs (see
  [Background work and playback](#background-work-and-playback)).
- `capabilities.chromaprint` says whether FFmpeg includes Chromaprint.

### Key frames

`POST /v1/frames` takes `{path, start_ms, duration_ms}` and returns `Frames
{frames: [{ms, black, ylow, yhigh, ymax, sat}]}`. The server looks in them for
credits: text on black.

- Decodes only the key frames of the first video stream (`-skip_frame nokey`),
  shrunk to 320 pixels wide in 8 bits (HDR too).
- `blackframe` gives `black` (percent of pixels at most 32); `signalstats`
  gives `ylow` and `yhigh` (luma of the darkest and brightest tenth), `ymax`
  and `sat` (average saturation). Luma is 8-bit studio range (black is 16).
- Shares the fingerprints' limit and priority.

## Reading ahead

`POST /v1/prefetch` reads the parts of a file a playback is about to read, so
they come from the share's or this host's cache when it starts. The server
asks for it when an item's page opens, or a card's Play button has the pointer
or the focus, at most every two minutes per file.

| Request field | Meaning |
|---|---|
| `path` | the file |
| `start_ms?` | where the playback would start (0 or none: the beginning) |

It reads, in this order, each byte once:

| `part` | What |
|---|---|
| `head` | the first 4 MB |
| `index` | the Matroska Cues (found through the seek heads and the elements before the first cluster only) or the MP4 `moov`, wherever it is, 64 MB at most |
| `start` | for `start_ms > 0`: from the keyframe at or before the start of the 4 s segment holding `start_ms` to the first keyframe 4 s past `start_ms`, 2 to 40 MB |

- Keyframe positions come from the index (the Cues' `CueClusterPosition`, or
  `stco`/`co64` and `stsc`). Files with neither get their head only.
- Reads go in 2 MB pieces, four at a time, each with its own handle.
- A keyframe index for remuxing is kept from the index read.
- One file at a time, one request per file (another answers `skipped`), ten
  seconds at most, waiting included.
- None starts within 5 s of a job creation or while a playback starts
  (`skipped`). A playback that begins meanwhile, or the request going away,
  stops it (`stopped`).

The answer is `Prefetched {ranges: [{part, start, end}], bytes, elapsed_ms,
skipped?, stopped}` (`end` exclusive).

## Background work and playback

Seek previews (`/v1/trickplay`), fingerprints and key frames
(`/v1/fingerprint`, `/v1/frames`), keyframe scans (`/v1/keyframes`) and a
library scan's probes (`/v1/probe`) read from the same disks and shares as
playbacks. While a playback starts:

- none of them starts (probes wait);
- FFmpeg and FFprobe runs under way are suspended (`NtSuspendProcess` on
  Windows, `SIGSTOP` on Linux; elsewhere they only wait to start), then
  resumed. A paused run's time limit counts only the time it ran.

A playback is starting:

- while a job is being created (probe, keyframe index, FFmpeg's start);
- while one of its FFmpeg processes (at Play, a seek, a restart) hasn't written
  its first 3 segments, 20 s at most;
- for `ms` after `POST /v1/hold`, which the Pley server sends (10 s) for a
  direct play.

While any transcode slot is in use (`running_processes`: a job's FFmpeg,
even paused far ahead, or a conversion), the same runs also wait and pause,
until 5 s after the last one ends. Probes and reading ahead wait for playback
starts only. The Pley server also waits for transcodes before asking for seek
previews or fingerprints.

Background runs and conversions start at below-normal process priority
(`nice` 10 on Linux), a job's FFmpeg at normal priority. `/v1/status` counts
the runs paused now (`background_paused`).

## Metrics

`GET /v1/status` returns the counters below, running and hardware processes
against their limits, `background_paused`, work folder disk use,
`memory_bytes` (segments in memory), and every job's `JobStatus`: state
(`starting`, `running`, `idle`, `finished`, `failed`), video and audio plans
(input → output, encoder, decoder, tone mapper), FFmpeg progress (fps, speed,
bitrate, position, dropped and duplicated frames), restarts, bytes served,
`warnings`, `last_error`, `rendition`, `variants`, `in_memory` and
`memory_bytes`.

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
processor and memory use the server samples every 2 s
(`GET /api/system/resources` in
[docs/API.md](https://github.com/hhaidar/pley/blob/master/docs/API.md#system-admin)).
A managed transcoder and its FFmpeg processes count there as Pley's; an
external one doesn't.

## Benchmark

`scripts/bench-transcoder.py` (Python 3.9 or later, no modules to install)
runs the transcoder as built, over its API, on local files. Per job it
reports the wait for the first (or sought) segment, speed, processor time and
use, peak memory (Windows: private commit, also summed; Linux: resident set)
and bytes made.

- `--make-clips DIR --source <4K HDR file>` cuts the clips it needs from a
  film: 1080p H.264 with AAC, TrueHD and DTS copies, ten-minute loops, a
  minute of PGS subtitles from FFmpeg's FATE sample, an SRT.
- `--variants`: `gpu` (what the machine has), `sw` (no GPU), `sw4` and `sw2`
  (no GPU, four or two processors, one logical processor per core).
- `--mem-mb` sets a memory limit (Windows).
- `--bin name=path` (repeated) compares builds, in alternating order.
- `--session` sets the seconds of the real-time playback cases.

Cases: 4K HDR10 to 1080p SDR, 1080p to 720p, TrueHD to AAC and DTS to AC-3
(video copied), a remux, a seek, subtitles burned into 1080p and 4K HDR, and
throttled real-time playbacks (`session-*`) that check whether the player
waited.

```
python scripts/bench-transcoder.py --make-clips bench --source 4k-hdr10.mkv
python scripts/bench-transcoder.py --media bench --hdr 4k-hdr10.mkv \
    --bin before=old/pley-transcoder.exe --bin after=target/release/pley-transcoder.exe \
    --variants gpu,sw,sw4 --repeat 3 --out results.jsonl
```

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
