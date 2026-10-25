# pley-transcoder

`pley-transcoder` is the component of pley that does **everything involving
FFmpeg**:

- probing media files
- HLS transcoding and remuxing
- HDR10, HLG and Dolby Vision tone mapping
- subtitle burn-in and extraction
- thumbnails and embedded cover art
- keyframe indexing

It is a separate program. The pley media server
runs it as a child process, or connects to one running elsewhere, and talks to
it only through the HTTP API described here. The request and response types
live in the `pley-transcoder-api` crate.

## Running

```
pley-transcoder [OPTIONS]
  --listen <ADDR>          default 127.0.0.1:7540 (port 0 = pick a free port)
  --token <TOKEN>          shared secret, at least 16 characters; env PLEY_TRANSCODER_TOKEN.
                           Required unless --listen is a loopback address
  --work-dir <DIR>         segments, caches (default: the user's cache folder,
                           e.g. ~/.cache/pley-transcoder); kept private to the user
  --ffmpeg / --ffprobe     binaries (default: from PATH)
  --max-jobs <N>           concurrent ffmpeg transcodes (default 8)
  --max-hw-jobs <N>        concurrent hardware encoder sessions (default 8);
                           extra jobs fall back to software encoding
  --probe-concurrency <N>  concurrent probes/extractions (default 8)
  --announce               print {"event":"listening","addr":"…"} once ready
  --exit-with-parent       exit when stdin closes (used by the managed mode)
  --redetect               ignore the cached capability detection
  --license                print the licence notice
```

**Managed mode (default).** The server starts
`pley-transcoder --listen 127.0.0.1:0 --announce --exit-with-parent` from the
folder that holds the server binary. It passes a random token through the
environment, reads the announced address, and restarts the transcoder with
backoff if it dies. The transcoder exits as soon as the server does.

**External mode.** Start the transcoder yourself, for example on a machine with
a GPU, then point the server at it with `--transcoder-url http://host:7540
--transcoder-token <secret>`. Media paths must be identical on both machines,
for example through the same network mount.

All endpoints except `GET /health` require `Authorization: Bearer <token>` when
a token is configured. Errors are `{"code": "...", "message": "..."}` with the
following status codes:

| Status | Code | Meaning |
|---|---|---|
| 400 | `bad_request` | invalid request |
| 401 | `unauthorized` | missing or wrong token |
| 404 | `job_not_found` | unknown job (expired, deleted, or the transcoder restarted) |
| 404 | `not_found` | unknown file, or segment out of range |
| 500 | `ffmpeg_failed` | ffmpeg or ffprobe could not process the file |
| 500 | `internal` | anything else (e.g. ffprobe could not be started) |
| 503 | `busy` | every transcode slot is in use |
| 504 | `timeout` | the operation took too long |

## Endpoints

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/health` | – | `ok` |
| GET | `/v1/info` | – | `TranscoderInfo`: version, license, source, capabilities, limits |
| GET | `/v1/status` | – | `TranscoderStatus`: counters, running processes, disk use, every job with live metrics |
| GET | `/metrics` | – | Prometheus text format |
| POST | `/v1/probe` | `ProbeRequest {path}` | `MediaInfo` (cached per file version) |
| POST | `/v1/thumbnail` | `ThumbnailRequest {path, time_ms?, width?, duration_ms?, hdr}` | `image/jpeg` |
| POST | `/v1/cover` | `CoverRequest {path}` | embedded cover art (`image/jpeg` or `image/png`) |
| POST | `/v1/subtitles` | `SubtitleRequest {path, stream_index?, partial?}` | `text/vtt`: a text track of the file, or a subtitle file (see *Subtitle extraction*). With `partial` (embedded tracks): within a few seconds, the whole track when it's done by then (as a rule when it can be read through the file's Cues), else the cues read so far with `X-Pley-Complete: 0` while the extraction goes on in the background |
| POST | `/v1/keyframes` | `KeyframesRequest {path}` | `200` if indexed, `202` if queued |
| POST | `/v1/prefetch` | `PrefetchRequest {path, start_ms?}` | `Prefetched {ranges: [{part, start, end}], bytes, elapsed_ms, skipped?, stopped}`: reads the parts of the file a playback starting at `start_ms` reads first, so they come from a cache when it does (see *Reading ahead*) |
| POST | `/v1/trickplay` | `TrickplayRequest {path, interval_ms?, width?}` | `TrickplayInfo {id, interval_ms, width, height, columns, rows, count, sheets}`: seek-bar previews, cached per file version until all sheets have been fetched (or for a day). Keyframes only are decoded (seconds for a film); HDR and Dolby Vision are tone-mapped with libplacebo when available. |
| GET | `/v1/trickplay/{id}/{n}.jpg` | – | sprite sheet `n` (`columns × rows` thumbnails); once every sheet of a set has been fetched, the set is deleted (fetch them all in one go and keep them) |
| POST | `/v1/fingerprint` | `FingerprintRequest {path, stream_index?, start_ms, duration_ms}` | `Fingerprint {start_ms, item_seconds, data: u32[]}`: raw Chromaprint values of the audio (`400` when FFmpeg lacks Chromaprint) |
| POST | `/v1/frames` | `FramesRequest {path, start_ms, duration_ms}` | `Frames {frames: [{ms, black, ylow, yhigh, ymax, sat}]}`: how dark the key frames of part of the video are and how much contrast they have, for credits detection |
| POST | `/v1/jobs` | `JobRequest` | `201 JobCreated` |
| GET | `/v1/jobs` | – | `JobStatus[]` |
| GET | `/v1/jobs/{id}` | – | `JobStatus` |
| DELETE | `/v1/jobs/{id}` | – | `204` (kills ffmpeg, deletes segments) |
| POST | `/v1/jobs/{id}/touch` | – | `204` (keeps a paused session alive) |
| GET | `/v1/jobs/{id}/index.m3u8` | – | HLS VOD playlist |
| GET | `/v1/jobs/{id}/{n}.ts` | – | MPEG-TS segment `n` (waits for it, or starts ffmpeg) |

## How HLS jobs work

1. **Layout first.** When a job is created, its segment layout is fixed and the
   whole VOD playlist is known up front, so players can seek anywhere.
   - When transcoding, segments are every 4 s and keyframes are forced on that
     grid.
   - When remuxing (`video.mode = copy`), segments start on the source's
     keyframes, taken from the keyframe index. Keyframe timestamps are made
     relative to the container start time (`MediaInfo.start_ms`, often far from
     zero in MPEG-TS), because that is what `-ss` and the output timeline use.

   The layout is deterministic. If the transcoder restarts, the server
   recreates the job and the same segment numbers still line up.

   A job has at most 200 000 segments, so a damaged or crafted file can't
   claim a length that costs gigabytes of memory. Durations beyond about 9
   days are not believed: the probe then uses the streams' own durations
   (Matroska `DURATION` tags included) or the file size over the streams'
   bitrates, and a file with nothing plausible is refused. Media info sent
   with a job is probed again when its duration is implausible. Files long
   enough to need it get segments longer than requested instead of more.
2. **Segments on demand.** A segment request is answered from disk if that
   segment already exists. If it doesn't:
   - When a running ffmpeg process is about to produce it (at most three
     segments ahead of the process, never behind it), the request waits for
     that process.
   - Otherwise a new process starts exactly at that segment, using `-ss`,
     `-output_ts_offset`, `-segment_times` and `-segment_start_number`, so
     timestamps stay on the absolute timeline. `avoid_negative_ts` is disabled
     so B-frame delay can't shift one process's segments relative to another.
   - A remux process seeks a little past its boundary keyframe. With B-frames,
     FFmpeg moves a seek back by 3/23 s (130 ms) in every container that
     doesn't seek by presentation time (all but MP4 and MOV): a seek right at
     the keyframe would land on the one before it, and every segment of the
     process would hold the GOP before its own. The seek goes 140 ms past the
     boundary there (at most halfway to the next keyframe), 1 ms elsewhere.
     Remuxes seek with `-noaccurate_seek`, so converted audio starts at the
     keyframe like the copied video, not at the seek point, which would leave
     a hole where an older process's segment plays into the new one's.
   - A new process stops right before the next segment that already exists,
     so segments being served are never rewritten.
   - A process started less than 3 s ago is not replaced while another request
     is waiting for it, so two requests far apart cannot keep restarting each
     other.
3. **Completion tracking.** ffmpeg's segment muxer writes a line to stdout as
   each segment finishes, which marks it ready and wakes waiting requests at
   once, with no polling. ffmpeg's `-progress` output on stderr supplies the
   live metrics: fps, speed, bitrate and position.
4. **Throttling.** Once a 60 s buffer exists, ffmpeg reads at most 2× realtime
   (`-readrate`), which saves CPU, GPU and disk. This applies when supported
   and enabled.
5. **Housekeeping.**
   - A process idle for 60 s (paused player) is stopped.
   - A job idle for 20 minutes expires.
   - Segments more than 90 behind the playhead are deleted (in small batches,
     off the async runtime).
   - On startup, segments left over from an earlier run are removed.
   - The caches in the work directory are pruned an hour apart (the first
     time two minutes after start-up). Entries unused for longer than their
     age limit go, then the least recently used ones until the folder fits its
     budget; temporaries left by a crash go after 3 hours untouched (a
     subtitle pass that runs longer touches its outputs every 10 minutes).
     Reading an entry counts as using it.

     | Folder | Holds | Age limit | Budget |
     |---|---|---|---|
     | `keyframes/` | keyframe indexes | 90 days | 256 MB |
     | `subtitles/` | converted and extracted subtitles | 60 days | 512 MB |
     | `trickplay/` | seek-preview sheets | 1 day | 1 GB |
     | `libplacebo/` | libplacebo's compiled shaders | 30 days | 128 MB |

     A set of seek-preview sheets is also deleted as soon as every sheet in
     it has been fetched: the server stores the sheets it fetches.
   - ffmpeg never outlives the transcoder: on Windows the transcoder runs in a
     kill-on-close job object, on Linux every child gets a parent-death signal.
     On shutdown (signal, or stdin closed with `--exit-with-parent`) all jobs
     are stopped first, then open requests get 5 s to finish.
6. **Failure handling.** If ffmpeg fails twice at the same segment, the error is
   returned instead of retrying forever. The job's `last_error` holds ffmpeg's
   last log lines.

## Transcoding features

- **Encoders.** At startup each candidate encoder is test-encoded rather than
  just listed:
  - NVENC, QSV and AMF on Windows and Linux
  - VAAPI on Linux
  - VideoToolbox on macOS
  - libx264 and libx265 as software fallbacks

  `hwaccel = auto` picks the first hardware encoder that works. Forced
  keyframes are IDR frames (`-forced-idr`), so every segment can be decoded on
  its own. The results are cached in `capabilities.json`, keyed by the ffmpeg
  binary's size and modification time.
- **Hardware decoding.** The decoder is paired with the encoder: `cuda` with
  NVENC, `d3d11va` with QSV/AMF on Windows, `vaapi`, or `videotoolbox`. Frames
  come back to system memory, and ffmpeg falls back to software decoding by
  itself if the codec isn't supported.
- **Tone mapping** of HDR10, HLG and Dolby Vision to SDR BT.709, best available
  first:
  - `libplacebo` on the GPU through Vulkan. It also applies Dolby Vision RPU
    reshaping, so **profile 5** (IPTPQc2) renders with correct colors.
  - `tonemap_opencl` on the GPU.
  - `zscale` plus `tonemap` on the CPU.

  libplacebo keeps the shaders it compiles in the work directory's
  `libplacebo/` folder (made at start-up, with an absolute path, since FFmpeg
  runs in a job's folder), so later processes skip about 0.2 s of compiling:
  every start, seek and quality switch of a tone-mapped job. A work directory
  whose path holds a character FFmpeg's filter syntax reads (`, ; [ ] ' :`
  other than a drive's colon) goes without it.

  Profiles 7 and 8 have an HDR10- or HLG-compatible base layer and work with any
  of these. Profile 5 without libplacebo produces a warning: its frames carry
  no colour properties (they only come from the RPU), so they are tagged
  BT.2020 PQ for zimg and OpenCL. The video plays, with off colours. The
  same tagging fills in whatever an HDR stream leaves untagged.
- **Frame grabs** (`/v1/thumbnail` with `hdr`) tone-map Dolby Vision profile 5
  through libplacebo and other HDR through zimg. If tone mapping fails (a
  Vulkan device that went away, say), the frame is grabbed as it is rather
  than not at all.
- **Subtitle burn-in.**
  - Bitmap subtitles (PGS, VobSub, DVB, `.sup` files) are overlaid at source
    resolution, after tone mapping and before scaling.
  - Text subtitles (SRT, ASS) are rendered by libass from an `.ass` file
    extracted into the job folder while the job is created. An embedded
    track's extraction reads the whole file, so it gives up after 20
    minutes, inside the 25 the server waits for the job; a job folder whose
    job isn't made after all (an error, or the request gone) is removed.
- **Audio.** Copied when the client supports the codec. Otherwise it's
  transcoded to AAC, downmixed to stereo by default, and resampled to 48 kHz
  when needed.
- **Remux.** H.264 video is copied into HLS, with segments cut exactly on the
  source's keyframes.
  - Matroska and WebM: the keyframe index is read instantly from the file's
    Cues element, so remuxing works on the first play.
  - MP4, MOV and M4V: the index is read just as quickly from the video
    track's sample tables (`stss`, `stts`, `ctts`) and edit list, or from the
    movie fragments' sample flags in fragmented files (CMAF, OBS
    recordings). Times match FFmpeg's packet timestamps, edit lists included:
    checked against ffprobe on files written by FFmpeg and by Windows Media
    Foundation (`moov` at the end, B-frames without an edit list).
    Files with structures this doesn't model (several edits, sync points in
    sample groups, inconsistent tables) are left to the packet scan below,
    as are fragmented files whose fragments take more than 2 s to walk.
  - Other containers: the index is built in the background (one file at a
    time, by reading packet headers) after `POST /v1/keyframes`, which the
    server sends when an item page is opened. Until it exists, jobs transcode.
  - Formats are recognised by content, not by file name.
- **Subtitle extraction.** Text tracks (SubRip, ASS/SSA, plain text and
  WebVTT) become WebVTT, cached per file version.
  - Matroska: mkvmerge and FFmpeg list every subtitle block in the Cues, with
    its place in its cluster, so the track asked for is read block by block:
    one disk seek per subtitle instead of reading the whole file. A film's
    track takes milliseconds on a local disk and seconds on a NAS (a
    thousand subtitles of a 77 GB remux on a disk-backed share: 5 to 12 s),
    where reading the file takes minutes. Sixteen reads run at once. A
    cluster's head (its timestamp) is only read where the block can't be
    told without it; one cluster in sixteen is checked, and a file whose
    blocks aren't where their cues say has every head read. A file's tracks
    are read side by side (half of `--probe-concurrency` at once in all, 4
    by default).
  - A track's extraction runs on its own: every request for the track
    waits for the one under way (a `partial` one a few seconds at most, and
    not at all once FFmpeg reads the file for it), and it goes on, to the
    cache, when they stop waiting. A track the Cues couldn't give goes
    straight to FFmpeg the next time (the last thousand such tracks are
    remembered).
  - A read through the Cues runs in the transcoder itself, so it has
    limits that FFmpeg, a process of its own, doesn't need. What it keeps
    takes at most 64 MiB: the file's seek heads, info, tracks, Cues and
    tags as it reads them, 256 bytes for the records of each block the Cues
    list for the track (counted as the Cues are read, so a file listing
    millions of tiny blocks is turned down before any is read), the blocks
    and what they inflate to, and the WebVTT made of them. No block may be
    bigger than 1 MiB nor inflate past 10 MB, an ASS header may define at
    most 10,000 styles, and the read takes at most a minute, finding the
    Cues and converting included. The Cues are looked for in the seek
    heads and before the first cluster, never cluster by cluster. Each
    block must lie in its cluster, after the one before.
  - The output is what FFmpeg's `-c:s webvtt` makes of the same track, byte
    for byte: the SubRip, ASS, plain text and WebVTT decoders and the WebVTT
    encoder (bold, italics, underline and line breaks; its quirks included)
    are followed step by step. This is checked against FFmpeg on real files
    by an ignored test (`PLEY_TEST_MKV=<file> cargo test -p pley-transcoder
    cues_match_ffmpeg -- --ignored --nocapture`).
  - FFmpeg extracts the tracks instead when anything is out of the
    ordinary: the file isn't Matroska; its Cues aren't found as above; its
    tracks don't line up with ffprobe's streams; its Cues don't list the
    subtitles, or not with their places; mkvmerge's own statistics tags
    disagree with the Cues on how many blocks a track has; a cue points at
    no block, at another track's, or at one that breaks the limits above;
    the track is encrypted, compressed with bzip2 or LZO (zlib and header
    stripping are read) or in two ways, has a timing of its own (a
    timestamp scale or a codec delay), laced blocks or cues before the
    start or out of order, or is WebVTT as mkvmerge writes it
    (`S_TEXT/WEBVTT`; WebM's `D_WEBVTT` is read); the file's timestamps
    aren't whole milliseconds and it has a start time, as ffprobe gives
    every file with audio or video (pley's media info keeps that start in
    whole milliseconds, while FFmpeg takes it off to the microsecond). Then
    every text track of the file comes out in a single pass over it, which
    may take 20 minutes, or as long as reading the file at 10 MB/s takes
    (two hours for 70 GB). FFmpeg writes each cue as soon as it is read: a
    `partial` request is answered with the cues found so far. Two such
    passes run at once, apart from the quick FFmpeg runs (thumbnails,
    cover art, subtitle files), which never wait for them. When a pass
    fails, one more reads the track asked for alone (another track may be
    what FFmpeg couldn't convert), unless it ran out of time.
  - External files are converted with charset detection (UTF-8, UTF-16 BOM,
    or Windows-1252 as a fallback).

## Audio fingerprints

`POST /v1/fingerprint` runs FFmpeg's `chromaprint` muxer (`-f chromaprint
-fp_format raw`) on part of the audio, downmixed to mono 11025 Hz, which is
what Chromaprint analyses. Each value covers 1365 samples (0.124 s) and is
returned as a plain number. The pley server compares the fingerprints of
episodes to find shared intros and credits. Requests run two at a time, at
below-normal process priority, so they never slow down playback. Whether the
FFmpeg build includes Chromaprint is reported as `capabilities.chromaprint`.

`POST /v1/frames` decodes only the key frames of part of the first video
stream (`-skip_frame nokey`), shrinks them to 320 pixels wide in 8 bits (HDR
too) and measures each with `blackframe` (share of pixels at most 32) and
`signalstats` (luma of the darkest and brightest tenth, the maximum,
average saturation). The server finds credits in them: text on black. It
shares the fingerprints' limit and priority.

## Reading ahead

`POST /v1/prefetch` reads a file the way a playback is about to, before it
does: the server asks for it when an item's page opens, or a card's Play
button has the pointer or the focus, at most every two minutes per file. On
a network share a cold 4 s of video takes 0.3-3 s to read, and a few
milliseconds once it is in the share's or this host's cache. It reads, in
this order, each byte once (`part` in the answer):

- `head`: the first 4 MB (headers, the first clusters or chunks);
- `index`: the Matroska Cues or the MP4 movie box (`moov`), wherever it is,
  64 MB at most. Matroska Cues are found through the seek heads and the
  elements before the first cluster only: never by walking the clusters;
- `start`, for `start_ms > 0`: from the keyframe at or before the start of
  the 4 s segment holding `start_ms` (where a transcode decodes from; a
  remux or a direct play starts there or later) to the first keyframe 4 s
  past `start_ms`, 2 to 40 MB. The keyframes' places come from the index:
  the Cues' `CueClusterPosition`, or the video track's `stco`/`co64` chunk
  offsets and `stsc` samples per chunk. Files without either get their head
  only.

Ranges are read in 2 MB pieces, four at a time, each reader with its own
handle. A keyframe index for remuxing is kept from the index read. It is
background work: one file at a time, one request per file at once (another
answers `skipped`), ten seconds at most, waiting included. None starts
within 5 s of a job creation (`skipped`), and one that begins meanwhile
stops the reading (`stopped`), as does the request going away.

## Metrics

`GET /v1/status` returns:

- totals: jobs created and failed, processes started and failed, segments
  produced and served, bytes served, probes (with total probe time),
  thumbnails, subtitle extractions, keyframe scans
- running and hardware processes against their limits
- work-dir disk usage
- per job: state, video and audio plans (input → output, encoder, decoder,
  tone mapper), ffmpeg progress (fps, speed, bitrate, position, dropped and
  duplicated frames), restarts, bytes served, warnings and the last error

`GET /metrics` exposes the same data for Prometheus (`pley_transcoder_*`). The
pley web UI shows it under *Settings → Dashboard*.

## License

```
pley-transcoder  Copyright (c) 2026 the Pley authors. All rights reserved.
Restricted: its licence terms are to be determined, and no licence to copy,
modify or distribute it is granted until then. It comes with ABSOLUTELY NO
WARRANTY. See the LICENSE file.
```

FFmpeg is invoked as a separate program and has its own license. The FFmpeg
build used during development (`--enable-gpl --enable-version3`) is GPLv3.
