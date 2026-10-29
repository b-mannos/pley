# pley-transcoder

`pley-transcoder` is the component of Pley that does **everything involving
FFmpeg**:

- probing media files
- HLS transcoding and remuxing
- HDR10, HLG and Dolby Vision tone mapping, or HDR kept in HEVC Main 10
- subtitle burn-in and extraction
- thumbnails and embedded cover art
- keyframe indexing

It is a separate program. The Pley media server
runs it as a child process, or connects to one running elsewhere, and talks to
it only through the HTTP API described here. The request and response types
live in the `pley-transcoder-api` crate.

## Running

```
pley-transcoder [OPTIONS]
  --listen <ADDR>          default 127.0.0.1:7540 (port 0 = pick a free port)
  --token <TOKEN>          shared secret, at least 16 characters; better given as
                           PLEY_TRANSCODER_TOKEN in the environment (a command line is
                           in the list of processes, for every account to read).
                           Required unless --listen is a loopback address
  --work-dir <DIR>         segments, caches (default: the user's cache folder,
                           e.g. ~/.cache/pley-transcoder). The folders the transcoder
                           keeps in it (jobs, keyframes, subtitles, trickplay,
                           libplacebo, conversions) are its own, private to the user;
                           so is the folder itself when the transcoder makes it, or it
                           is the default one. One given that is there already (/tmp)
                           keeps its permissions
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
a GPU, with its token in `PLEY_TRANSCODER_TOKEN`, then point the server at it
with `transcoder_url = "http://host:7540"` and the same `transcoder_token` (or
`PLEY_TRANSCODER_TOKEN` in its environment). Media paths must be identical on
both machines, for example through the same network mount.

All endpoints except `GET /health` require `Authorization: Bearer <token>` when
a token is configured. Without one (only on a loopback address), the
transcoder answers only requests addressed to this computer: a `Host` of
`localhost`, `127.0.0.1` or `[::1]`, with any port. A web page whose own
name was pointed at 127.0.0.1 (DNS rebinding) sends that name and is
refused. Other accounts on the same computer can still reach it: where that
matters, give it a token. Errors are `{"code": "...", "message": "..."}`
with the following status codes:

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

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/health` | – | `ok` |
| GET | `/v1/info` | – | `TranscoderInfo`: version, commit (the build's short hash, when known), license, source, capabilities, limits, and `memory {available, reason?, path?, budget_bytes}`: whether segments can be kept in memory (see *Segments in memory*) |
| GET | `/v1/status` | – | `TranscoderStatus`: counters, running processes, disk use, `memory_bytes` (segments in memory), every job with live metrics (`in_memory`, `memory_bytes`) |
| GET | `/metrics` | – | Prometheus text format |
| POST | `/v1/probe` | `ProbeRequest {path}` | `MediaInfo` (cached per file version) |
| POST | `/v1/thumbnail` | `ThumbnailRequest {path, time_ms?, width?, duration_ms?, hdr}` | `image/jpeg` |
| POST | `/v1/cover` | `CoverRequest {path}` | embedded cover art (`image/jpeg` or `image/png`) |
| POST | `/v1/subtitles` | `SubtitleRequest {path, stream_index?, partial?}` | `text/vtt`: a text track of the file, or a subtitle file (see *Subtitle extraction*). With `partial` (embedded tracks): within a few seconds, the whole track when it's done by then (as a rule when it can be read through the file's Cues), else the cues read so far with `X-Pley-Complete: 0` while the extraction goes on in the background |
| POST | `/v1/subtitles/mks` | `SubtitleRequest {path, stream_index?}` | `video/x-matroska`: the track (or the subtitle file, without `stream_index`) copied as it is into a Matroska file of its own, for players that show picture subtitles (PGS, VobSub with its palette, DVB) next to an HLS stream. Times are the original's from the start of the file. Anything but subtitles fails (`-vn -an -dn`). Reads the whole file, as one of the passes over whole files; not cached (the server caches it). Transcoders that serve it say `subtitle_matroska: true` in their capabilities |
| POST | `/v1/keyframes` | `KeyframesRequest {path}` | `200` if indexed, `202` if queued |
| POST | `/v1/hold` | `HoldRequest {ms}` | `204`: background work waits (and its FFmpeg runs pause) for `ms`, a minute at most: a playback the transcoder doesn't run, a direct play, is starting (see *Background work and playback*) |
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
| POST | `/v1/jobs/{id}/bitrate` | `BitrateRequest {max_bitrate_kbps}` | `BitrateChanged {bitrate_kbps}`: a transcode goes on at another video bitrate (1 Mbps at least, the job's own at most). The segments past the one its player asked for last and the next are deleted and made again, the same size and layout, so the player plays on from the same playlist; a process running restarts there at once. `400` for a remux. Transcoders that do it say `live_bitrate: true` |
| GET | `/v1/jobs/{id}/index.m3u8` | – | HLS VOD playlist |
| GET | `/v1/jobs/{id}/master.m3u8` | – | multivariant playlist: the job's variant (`index.m3u8`), then its smaller renditions' (`v1.m3u8`…, see *Renditions*), each with `CODECS`, `SUPPLEMENTAL-CODECS`, `RESOLUTION`, `FRAME-RATE`, `VIDEO-RANGE` and the bandwidth (see *Dolby Vision in remuxes*). Transcoders that serve it say `hls_master_playlist: true` in their capabilities |
| GET | `/v1/jobs/{id}/{n}.ts` | – | MPEG-TS segment `n` (waits for it, or starts ffmpeg) |
| GET | `/v1/jobs/{id}/v{k}.m3u8`, `/v1/jobs/{id}/v{k}-{n}.ts` | – | rendition `k`'s playlist and segments (`JobRequest.variants`; transcoders that make them say `hls_variants: true`) |
| POST | `/v1/conversions` | `JobRequest` | `ConversionStatus {id, state, progress, duration_ms, size?, error?, video?, audio?}`: the request made into one Matroska file, or MP4 with `container: "mp4"` (see *Conversions*). Asked again for the same input and request, the same conversion (or its file) answers; a failed one starts again. Transcoders that serve it say `conversions: true` in their capabilities, and `conversions_mp4: true` when they make MP4 |
| GET | `/v1/conversions/{id}` | – | `ConversionStatus`: `queued`, `running` (with `progress`, 0 to 1), `done` (with `size`) or `failed` (with `error`) |
| DELETE | `/v1/conversions/{id}` | – | `204` (stops ffmpeg, deletes the file) |
| GET | `/v1/conversions/{id}/file` | – | `video/x-matroska` (or `video/mp4`), once done; one range (`bytes=a-`, `bytes=a-b`) as asked, `416` past the end |

## Conversions

A conversion makes a `JobRequest` into one Matroska file, the whole media
in one FFmpeg run: what a download is when the device that keeps it
doesn't play the original as it is (sound it doesn't decode, a picture
too big for it). Its streams are what a job's segments would be (the same
plan: encoder, tone mapping, AAC), made at full speed (`throttle` is
ignored) from the start (`start_ms` is too), with every subtitle track
of the input as it is, in its order (MP4's text made SRT; teletext, which
Matroska can't hold, left out), with its language and name: a player
finds a track by its place among the subtitle tracks, as in the original.
A conversion whose FFmpeg ends without error but short of the media's end
(a share that went away) fails rather than keeping half a film. One whose
file the janitor deleted is made again when asked for. A remux whose segments would
be edited (Dolby Vision converted, HDR10+ left out) has its video made
anew instead, as a file can't be edited the way segments are.

With `container: "mp4"` the file is MP4, for players that open no
Matroska (QuickTime): its text subtitles made MP4's own (`mov_text`), in
their order, its picture subtitles left out, HEVC tagged `hvc1` (QuickTime
plays no `hev1`), and its index ahead of the media (`+faststart`), so it
plays as soon as it opens. It is another conversion than the Matroska one
of the same request (its id says so), named `<id>.mp4`.

One conversion runs at a time, in a transcode slot like a job's process
(a hardware session when one is free, else the software encoder), at low
process priority so that playbacks keep the processor; while every slot
is in use it waits. Its id comes from the input (path, size,
modification time) and the request's video, audio and subtitle parts, so
the same file asked for twice is made once. One nobody asks for or about
(`GET /v1/conversions/{id}`) for an hour while it waits isn't made when
its turn comes: the device that wanted it went, and asking again starts
it again. Files live in `conversions/` under the work folder; the janitor
deletes those unused for a week, and the least recently used past 64 GB.
A transcoder that restarts knows the files it made by their names;
half-made ones are deleted.

## How HLS jobs work

1. **Layout first.** When a job is created, its segment layout is fixed and the
   whole VOD playlist is known up front, so players can seek anywhere.
   - Segments are as long as the request's `segment_seconds` (1 to 10 s, 4 s
     by default). The Pley server asks for 2 s: nothing plays before FFmpeg
     has written a whole segment, so shorter ones start playback, seeks and
     quality switches sooner, for a keyframe every 2 s.
   - When transcoding, keyframes are forced on that grid.
   - When remuxing (`video.mode = copy`), segments start on the source's
     keyframes (as many as make at least the segment length), taken from the
     keyframe index. Keyframe timestamps are made
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
2. **Segments on demand.** A segment request is answered from disk (or
   memory, see *Segments in memory*) if that segment already exists. If it
   doesn't:
   - When a running ffmpeg process is about to produce it (at most 12 s of
     segments ahead of the process, never behind it), the request waits for
     that process. Not when the read rate holds the process back (see
     *Throttling*) and that wait is longer than a restart: past its burst and
     going at the read rate (not slower: then the encoder is the limit, and a
     new process would be no quicker), it makes a segment of 2 s in about a
     second, so a seek a few segments past it would wait seconds. The
     request then starts a new process at its segment, with a burst of its
     own, when the wait (from halfway through the segment being made) is
     longer than this job's last start to a first segment plus the stop of
     the old process, and no other request waits for a segment the old
     process is about to make.
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
     other. When that request goes away (served, or aborted by its player),
     the one held back looks again at once.
3. **Completion tracking.** ffmpeg's segment muxer writes a line to stdout as
   each segment finishes (a remux whose video needs editing has the segment
   edited first: see *Dolby Vision in remuxes*), which marks it ready and
   wakes waiting requests at once, with no polling (after noting the process has moved past it, so a
   woken request never sees it still owing that segment). ffmpeg's `-progress` output on stderr supplies the
   live metrics: fps, speed, bitrate and position.
4. **Throttling.** A process makes its first 120 s as fast as it can, then
   reads at real time (`-readrate 1`), so it stays as far ahead of the
   player as that burst took it, at a steady 1×: this saves CPU, GPU and
   disk. A read that falls behind (a slow disk or share) catches up at 2×
   where FFmpeg can (`-readrate_catchup`, FFmpeg 8). And once a process is 90 s
   ahead of the last segment the player asked for (players hold their own
   buffer ahead of the picture, so minutes ahead of it), it is paused, its
   process suspended (`NtSuspendProcess` on Windows, `SIGSTOP` on Linux),
   until it is less than 60 s ahead, or a request waits for a segment it is
   to make. Without the pause a process could go on to the end of the
   film, minutes or an hour ahead: work lost when the player
   stops or seeks, taken from other playbacks meanwhile (on a small
   computer, what lets two play at once), and segments on the disk. Both
   apply when the request's `throttle` is on (`throttle_transcodes`, on by
   default) and the read rate when FFmpeg supports it. How fast a process
   goes is measured over its last 4 s of progress (since it last went on,
   after a pause), since FFmpeg's own `speed` averages in the burst and the
   pauses; a job's status says that speed, and 1× for a process held to the
   read rate.
5. **Housekeeping.**
   - A process idle for 60 s (paused player) is stopped.
   - A job idle for 20 minutes expires.
   - Segments more than 6 minutes behind the playhead are deleted (in small
     batches, off the async runtime); in memory, those more than a minute
     behind it or 3 minutes ahead (see *Segments in memory*).
   - On startup, segments left over from an earlier run are removed, in
     memory those of transcoders gone (a crash).
   - The caches in the work directory are pruned an hour apart (the first
     time two minutes after start-up). Entries unused for longer than their
     age limit go, then the least recently used ones until the folder fits its
     budget; temporaries left by a crash go after 3 hours untouched (a
     subtitle pass that runs longer touches its outputs every 10 minutes).
     Reading an entry counts as using it.

     | Folder | Holds | Age limit | Budget |
     |---|---|---|---|
     | `keyframes/` | keyframe indexes | 90 days | 256 MB |
     | `subtitles/` | converted and extracted subtitles, the fonts attached to files | 60 days | 512 MB |
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
   last log lines. A process whose hardware encoder fails before its first
   segment (a driver update, a GPU gone, a session refused) hands the job to
   the software encoder, as when hardware sessions run out, with a warning
   on the job; once that happened to three jobs, `capabilities.json` is
   deleted, so the next start tests the encoders again.
7. **Renditions.** A transcode asked for with `variants` (adaptive
   bitrate: a player that moves between sizes by itself) also comes
   smaller: each variant is the job's request with its own `max_width`,
   `max_height` and `max_bitrate_kbps`, kept when it is no bigger and has
   fewer bits than the one above and its segments are laid out as the
   job's (transcodes' are: the layout depends on the duration alone).
   They are jobs of their own inside the job, in folders `v1/`…, with
   their segments named `v1-{n}.ts`… in their playlists; burned-in text
   subtitles are extracted once and copied to them. The job's
   `master.m3u8` lists them after its own, largest first. A process runs
   for the rendition the player asked last: when its requests move to
   another, the others' processes stop (what they made stays), and one
   asked for again starts where it is wanted. They expire, go and are
   tidied with the job, and its status is the playing rendition's
   (`rendition`, `variants`), with what all of them served.

   A transcode's bitrate, unless asked for less, is its size's: 20 Mbps
   from 2000 lines, 12 from 1400, 8 from 1000, 4 from 700, 2 from 500, 1.2
   from 400 and 0.7 below, counting the lines a 16:9 picture as wide
   would have (a 1920×800 film is 1080p's 8 Mbps).
8. **Segments in memory.** A job's segments live in its folder on disk,
   `jobs/<id>/` in the work folder, or in memory when the request's
   `options.in_memory` is on (`transcode_in_memory`, on by default), the
   job is throttled (its lead on the player is bounded then) and the
   transcoder has room. FFmpeg writes segments as files, so memory is a
   folder in RAM: `/dev/shm` on Linux, when it is a tmpfs. The transcoder
   keeps its own folder there, `pley-transcoder-<pid>/<id>/` (renditions
   in `v1/`…), private to its user, removed at shutdown; one left by a
   transcoder gone (a crash, its process no longer there) is removed at
   the next start, as a tmpfs holds it in memory until then. Each process
   runs in the folder it writes to, with a copy of the burned-in
   subtitles there, and each segment is served, edited (Dolby Vision, see
   *Dolby Vision in remuxes*) and pruned where its process made it.

   Windows and macOS have no folder in RAM by default, and their segments
   stay on disk (`/v1/info` says `memory.available: false`, "works on
   Linux only"). FFmpeg creates the files, so they can't be marked to
   stay in the file cache (`FILE_ATTRIBUTE_TEMPORARY`); a RAM disk needs a
   driver (Windows) or stays behind when the transcoder doesn't (macOS's
   `hdiutil`); FFmpeg sending its segments to the transcoder over HTTP
   would need the segment muxer's file handling and Dolby Vision's
   editing redone in memory. Windows' file cache keeps segments just
   written in memory for serving anyway.

   Memory is bounded twice:
   - A job keeps segments in memory a minute behind the playhead (6 on
     disk), and those more than 3 minutes ahead of it (a seek back leaves
     them) go too. A throttled process pauses 90 s ahead of the player.
   - The transcoder takes at most a quarter of the computer's memory and
     half of the folder's size (`/v1/info`'s `budget_bytes`; a budget
     under 256 MB, as Docker's default 64 MB `/dev/shm`, is no memory:
     give the container `--shm-size`, as deploy/docker's compose file
     does, 2 GB). Each job (each rendition) reserves what it
     is reckoned to take, 3 minutes of its bitrate (a transcode's with a
     quarter to spare, a remux's the source's, 40 Mbps when unknown):
     4K at 20 Mbps takes about 700 MB. A process goes to memory when that
     reservation fits the budget beside the others' (or what they take,
     when more), the folder keeps 64 MB free besides, and the computer a
     tenth of its memory (512 MB at least, from `MemAvailable`); else to
     disk.
   - When memory runs short later (segments past the budget, the folder
     or the computer short of room), the job with most in memory moves
     to disk, one every 5 s: its reservation goes, and its process starts
     again on disk at the segment it was making (a paused one, far ahead,
     at the next request). What it made stays in memory, served from
     there until pruned, so the player notices nothing.

   Conversions (`conversions/`) and the caches (keyframes, subtitles,
   seek previews, shaders) stay on disk.

## Transcoding features

- **Encoders.** At startup each candidate encoder is test-encoded rather than
  just listed:
  - NVENC, QSV and AMF on Windows and Linux
  - VAAPI on Linux, on the first Intel or AMD render node
    (`PLEY_VAAPI_DEVICE` names another)
  - VideoToolbox on macOS
  - libx264 and libx265 as software fallbacks

  `hwaccel = auto` picks the first hardware encoder that works. Forced
  keyframes are IDR frames (`-forced-idr`), so every segment can be decoded on
  its own. The results are cached in `capabilities.json`, keyed by the ffmpeg
  binary's size and modification time and by the GPUs the system shows (its
  render nodes and their drivers, NVIDIA's driver), so a GPU given later is
  tested.
- **Hardware decoding.** The decoder is paired with the encoder: `cuda` with
  NVENC, `d3d11va` with QSV/AMF on Windows, `vaapi`, or `videotoolbox`. Frames
  come back to system memory, and ffmpeg falls back to software decoding by
  itself if the codec isn't supported. Vulkan decoding (into libplacebo)
  counts for a codec only when the start-up test's frames reach the filters
  in Vulkan memory (`pixfmt:vulkan` in FFmpeg's verbose log): libplacebo
  takes frames from memory too, so a driver without Vulkan video decoding
  (AMD's for Vega graphics on Windows, say) would otherwise pass while the
  CPU decodes. Those GPUs decode with the encoder's decoder instead. A job
  whose FFmpeg says its hardware decoder failed to start ("Failed setup for
  format …") shows `cpu` as its decoder, with a warning.
- **Tone mapping** of HDR10, HLG and Dolby Vision to SDR BT.709, best available
  first:
  - `libplacebo` on the GPU through Vulkan. It also applies Dolby Vision RPU
    reshaping, so **profile 5** (IPTPQc2) renders with correct colors.
  - `tonemap_opencl` on the GPU.
  - `zscale` plus `tonemap` on the CPU. The first `zscale` scales to the
    output's size as it makes the picture linear, so the tone mapping (in
    32-bit floats, three planes) works on the output's pixels: 4K to 1080p,
    a quarter of them.

  When Vulkan only runs on the CPU (Mesa's llvmpipe, with no GPU),
  libplacebo comes last: it is several times slower than zscale there, and
  only profile 5 picks it, for its colors.

  libplacebo tone maps by the stream's own metadata (`peak_detect=0`):
  HDR10's mastering display and content light levels, Dolby Vision's and
  HDR10+'s per scene. Its peak detection, on by default, follows the
  brightest pixels of the frames it has seen, and each FFmpeg process starts
  with none: a process started by a seek, a new process window or a size
  switch made its first frames brighter or darker than the process before
  it made the same frames, a jump at the seam between their segments. On a
  test clip whose highlight brightens and dims (200 to 4000 nits every
  3 s), a face's brightness on the first frame of a segment was 121 (of 255)
  from a process started there and 128 from one started a segment or the
  file earlier (PSNR 35 dB between them); without detection the three
  processes give the same frames bit for bit, the face at 111 throughout
  (a little darker: the curve is set for the stream's stated 4000 nits rather
  than the 900 the frames reached). Thumbnails and seek previews tone map the
  same way. `tonemap_opencl` has peak detection that can't be turned off
  (63 frames, in its kernel), so it has the same seams; zimg's `tonemap`
  has none.

  Before a VAAPI encoder (Intel and AMD graphics on Linux) the tone
  mapper's device is made from VAAPI's: `-init_hw_device
  vaapi=va:<render node> -init_hw_device vulkan=vk@va` (or `opencl=ocl@va`)
  `-filter_hw_device vk`, and the last filter is `hwupload=derive_device=vaapi`.
  VAAPI's encoders take frames in VAAPI's memory only, and a plain
  `hwupload` uploads to the filters' device, Vulkan's or OpenCL's: the
  encoder couldn't start ("Impossible to convert between the formats
  supported by the filter 'Parsed_hwupload' and the filter 'auto_scale'"),
  and every HDR job fell back to x264. `derive_device=vaapi` finds the
  VAAPI device the filters' was made from. A Vulkan device made from
  VAAPI's is the same GPU where VAAPI says which it is (VA-API 1.15's PCI
  id, or AMD's name), and Vulkan's first device otherwise; an OpenCL one
  can only be made by Intel's OpenCL (`cl_intel_va_api_media_sharing`),
  not AMD's. At start-up each GPU tone mapper is tried that way into
  `h264_vaapi` (`capabilities.vaapi_tonemappers`), and Vulkan decoding
  too, per codec (`capabilities.vaapi_vulkan_decode`). Jobs with a VAAPI
  encoder use only the tone mappers that passed, and zimg; where none of
  them is left, or Dolby Vision profile 5 would lose libplacebo's
  colours, the job encodes with x264 instead, with a warning. HDR kept in
  HEVC that libplacebo would convert is tone mapped instead when
  libplacebo's frames don't reach VAAPI's encoder. Tried with FFmpeg 9.0.2
  and Mesa 26.2 in Debian (Docker on WSL, VAAPI through Mesa's D3D12
  driver on an RTX 5070, Vulkan on llvmpipe): the old arguments failed for
  libplacebo, Vulkan decoding and OpenCL alike; the new ones made HLS
  segments with libplacebo (the VA display's PCI id hidden, so that Vulkan
  falls back to llvmpipe) and with zimg, and an OpenCL device that can't be
  made from VAAPI's fails its start-up test. Not tried on Intel or AMD
  graphics.

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
- **HDR kept** (`video.codec = hevc` with `video.dynamic_range` `hdr10` or
  `hlg`): HDR video is encoded in HEVC Main 10 instead of being tone-mapped.
  Encoder detection test-encodes every working HEVC encoder in 10 bits too
  (`-profile:v main10`, P010; x265 with `yuv420p10le` and `-dolbyvision 0`)
  and marks it `main10`; `capabilities.hdr_transcode` says one passed.
  - Video already in the asked range (HDR10, HDR10+, Dolby Vision 7 and 8.1
    as their HDR10 base layer, HLG and 8.4 for `hlg`) isn't tone-mapped:
    decoded, scaled to P010 (`scale_cuda` without leaving the GPU when the
    CUDA zero-copy pipeline handles the codec, else on the CPU) and encoded.
  - Other HDR (HLG or Dolby Vision 8.4 to HDR10, PQ to HLG, Dolby Vision
    profile 5 to either) goes through libplacebo with a BT.2020 target in
    10 bits (`format=p010`, `color_trc=smpte2084` or `arib-std-b67`), on the
    Vulkan pipeline when the GPU decodes the codec into Vulkan memory.
  - The decoded frames' HDR10 metadata (mastering display, content light
    levels) goes through scaling and libplacebo to the encoder, which writes
    it as SEI messages with every frame (NVENC, and x265 with
    `repeat-headers=1`): every segment starts with it. PQ or HLG, BT.2020
    primaries and matrix are also set on the encoder (`-color_trc`…), so the
    stream says them whatever the frames were tagged with. Dolby Vision RPUs
    are never written (NVENC doesn't, x265 is told not to): Dolby Vision
    comes out as its base layer, or converted when it has none. HDR10+
    dynamic metadata is not kept.
  - Where HDR can't be kept the job tone-maps to SDR as above, with a
    warning: H.264 asked for, subtitles drawn onto the video, no `main10`
    encoder, or a conversion without libplacebo. When hardware sessions run
    out, the software fallback for an HDR job is a `main10` x265.
  - The master playlist names the output `hvc1.2.4.LLL.B0` with
    `VIDEO-RANGE=PQ` or `HLG`; the plan's `dynamic_range_out` says `hdr10`,
    `hlg` or `sdr` for transcodes.
- **Frame grabs** (`/v1/thumbnail` with `hdr`) tone-map Dolby Vision profile 5
  through libplacebo and other HDR through zimg. If tone mapping fails (a
  Vulkan device that went away, say), the frame is grabbed as it is rather
  than not at all.
- **Subtitle burn-in.**
  - Bitmap subtitles (PGS, VobSub, DVB, `.sup` files) are overlaid on the
    scaled, tone-mapped frame, their canvas fitted to its width: for 4K to
    1080p, a quarter of the pixels to blend.
  - They are blended on the CPU, after the one download in Vulkan pipelines
    too: about three quarters of a Ryzen 9 5900X core for 4K, a quarter of
    that for 1080p. FFmpeg's GPU ways don't do: libplacebo makes an output
    frame of each frame of each of its inputs, and FFmpeg sends a subtitle
    frame with every video frame (sub2video's heartbeat), doubling them;
    `overlay_vulkan` takes its two inputs in one format only, and gave black
    frames for `yuva420p` on NVIDIA. The integrated GPUs that would gain
    most also have CPU to spare more than GPU.
  - Text subtitles (SRT, ASS) are rendered by libass from an `.ass` file
    extracted into the job folder while the job is created. An embedded
    track's extraction reads the whole file, so it gives up after 20
    minutes, inside the 25 the server waits for the job; a job folder whose
    job isn't made after all (an error, or the request gone) is removed.
  - They are drawn with the fonts attached to the file (styled ASS, anime
    above all, names fonts that only the file has; without them libass
    falls back to a system font). The fonts are read from a Matroska file's
    Attachments element alone, which its seek head points to, while the
    track is extracted, so they cost the start nothing, and kept in
    `subtitles/<file key>.fonts/` (256 MB of them at most per file) for
    the next job of the same file version; the filter gets that folder as
    `fontsdir`. Subtitle files next to the video are drawn with the
    video's fonts too. Rendering from the source file with the filter's
    `si=` instead would read the whole file in every FFmpeg process, at
    each seek.
- **Audio.** Copied (`audio.mode = copy`) when the source is AAC, MP3, AC-3
  or E-AC-3, which MPEG-TS carries; anything else asked to be copied is
  converted, with a warning. Converted audio is encoded to `audio.codec`:
  - `aac` (the default): `-c:a aac`, stereo unless `channels` says more (up
    to 7.1), 192 kbit/s for stereo and 64 per channel above;
  - `eac3` (Dolby Digital Plus) or `ac3` (Dolby Digital): `-c:a eac3` or
    `-c:a ac3`, at most 5.1 (`-ac 6`), 640 kbit/s above stereo, 192 for
    stereo. FFmpeg's own encoders, in every build.

  `channels` never goes above the source's (a stereo track stays stereo),
  `bitrate_kbps` overrides the defaults within 64 to 640, and audio that
  isn't at 44.1 or 48 kHz is resampled to 48 kHz (the AC-3 encoders take
  nothing above 48 kHz). The Pley server asks for E-AC-3 5.1 for players
  whose output has 6 channels or more and that decode E-AC-3 or pass it
  through, AC-3 5.1 when they take only AC-3, and AAC otherwise (docs/API.md,
  *Version 2*). The job's audio plan reports the codec as `codec_out`.

  Lossless audio is decoded only as far as the conversion keeps: a DTS-HD
  Master Audio track converted to 5.1 or fewer channels is decoded from
  its lossy core (`-core_only`), and TrueHD converted to stereo or 5.1
  from the substream mastered with those channels (`-downmix stereo` or
  `5.1`; Blu-rays carry both inside the 7.1, and without one FFmpeg
  decodes the whole).
- **Remux.** H.264 and HEVC video is copied into HLS, with segments cut
  exactly on the source's keyframes.
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
- **Frames timed as they are decoded.** Some MP4s (FFmpeg muxing a raw H.264
  or HEVC stream) give every frame the time it is decoded at as the time it
  is shown, although B-frames are stored ahead of where they are shown.
  Players that trust these times show frames out of turn or drop them
  (Chrome drops about one in five), and copying the video keeps them. The
  probe of an MP4 or MOV file reads the times of its first 128 packets along
  with its streams (Matroska times frames as they are shown, MPEG-TS both
  ways). When a stream that reorders frames (`has_b_frames`) shows every
  packet read at its decode time, its first 48 packets are decoded, and the
  stream is marked `decode_order_times` if more than one frame comes out
  earlier than the frame before it. The server then converts its video:
  decoding gives every frame its right time.
- **Dolby Vision in remuxes.** A copied HEVC stream keeps its Dolby Vision
  metadata (the RPU NAL units, type 62) in the MPEG-TS segments, which have
  no way to say so (FFmpeg's MPEG-TS muxer writes no Dolby Vision
  descriptor): `master.m3u8` does, as `VideoRequest.dolby_vision` asks.
  - `copy` (the default, what older servers ask): nothing named, the base
    layer's codec only (`hvc1.2.4.L153.B0`);
  - `signal`: named for a player that decodes it. Profile 5: `dvh1.05.LL`
    alone; profile 8: the HEVC codec with
    `SUPPLEMENTAL-CODECS="dvh1.08.LL/db1p"` (8.1, `VIDEO-RANGE=PQ`),
    `/db4h` (8.4, `HLG`) or `/db2g` (8.2, `SDR`); profile 7: `dvh1.07.LL`;
  - `to81` (profile 7 only, else as `signal`): made profile 8.1 and named
    so. FFmpeg can't: `dovi_rpu` refuses to write profile 7 RPUs and never
    changes a profile, `dovi_split` (FFmpeg 9) drops the enhancement layer
    but keeps profile 7 RPUs. So FFmpeg copies the stream as it is and the
    transcoder edits each finished segment before it is ready: the
    enhancement layer's NAL units (type 63) are left out, and each RPU is
    rewritten as `dovi_tool`'s mode 2 does: no residual data (NLQ), no
    enhancement-layer resampling, one partition, the P8.1 colour
    coefficients in the display management data (BT.2020 YCbCr to RGB, RGB
    to LMS, `signal_color_space` 0), and, for a full enhancement layer (not
    a minimal one, whose residual is zero), no-op mapping curves, since its
    curves only make sense with the layer. The extension blocks (levels 1 to
    11, 254, 255; content mapping 2.9 and 4.0) and everything else stay as
    they are, and the CRC is computed again. The RPU syntax is FFmpeg's
    (`libavcodec/dovi_rpudec.c`) and `dovi_tool`'s; this is the transcoder's
    own code (`src/dovi.rs`), nothing of theirs is linked or run. An RPU
    that can't be read (or whose CRC is wrong) is left out with a warning on
    the job, and that frame plays as its base layer. Profile 7 as MakeMKV
    writes it is meant (one track); a disc's dual-PID streams carry the RPU
    in the enhancement-layer PID, which isn't copied.
  - `strip_hdr10plus`: the HDR10+ messages (ITU-T T.35, country 0xB5,
    provider 0x003C, application 4) are taken out of the prefix SEI NAL
    units of each segment, the other messages (mastering display, content
    light level, captions) kept; an SEI unit with nothing else goes. For
    Dolby Vision decoders that show black or freeze with both
    (androidx/media#1895).

  Editing a segment reads it whole, takes its video PES packets apart into
  NAL units and packetizes the PES again: the first TS packet keeps its
  adaptation field (PCR, random access), continuity counters run on, and
  other PIDs' packets are left as they were (those met in the middle of a
  PES come right after it). A 2 s segment of 4K takes milliseconds. The
  job's video plan reports `dolby_vision` (`copy`, `signal`, `to81`) and
  `hdr10plus_stripped`.
- **HDR10+, and profile 7's enhancement layer.** Only a stream's frames say
  whether it carries HDR10+ (ffprobe's stream side data doesn't in Matroska
  or MP4), so the probe of a Dolby Vision or PQ stream decodes its first
  frames (a fraction of a second; `-read_intervals %+#12 -show_entries
  frame_side_data`) and records `MediaStream.hdr10plus`; a PQ stream with
  HDR10+ becomes `dynamic_range: hdr10plus`. For profile 7 the same frames'
  Dolby Vision metadata says what the enhancement layer holds
  (`MediaStream.dv_el`): minimal (`mel`) when its residual is zero in every
  component (no NLQ offset, an input maximum of one, no dead zone), full
  (`fel`) otherwise, as `dovi_tool` tells them apart. Both are left out
  (unknown) when the frames can't be read, and for other streams. The job's
  video plan repeats them (`dv_el`, `hdr10plus`) for the input's name.
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
    every file with audio or video (Pley's media info keeps that start in
    whole milliseconds, while FFmpeg takes it off to the microsecond). Then
    every text track of the file comes out in a single pass over it, which
    may take 20 minutes, or as long as reading the file at 10 MB/s takes
    (two hours for 70 GB). FFmpeg writes each cue as soon as it is read: a
    `partial` request is answered with the cues found so far. Two such
    passes run at once, apart from the quick FFmpeg runs (thumbnails,
    cover art, subtitle files), which never wait for them. When a pass
    fails, one more reads the track asked for alone (another track may be
    what FFmpeg couldn't convert), unless it ran out of time. A track whose
    pass failed answers with that error for 30 minutes (the last thousand
    such tracks are remembered): a player asking every few seconds hears
    why, rather than each request reading the whole file again.
  - External files are converted with charset detection (UTF-8, UTF-16 BOM,
    or Windows-1252 as a fallback), told from their first megabyte. A file
    over 64 MB is no text subtitle file (a `.sup`, say): it is refused
    (`400`) before it is read.

## Audio fingerprints

`POST /v1/fingerprint` runs FFmpeg's `chromaprint` muxer (`-f chromaprint
-fp_format raw`) on part of the audio, downmixed to mono 11025 Hz, which is
what Chromaprint analyses. Each value covers 1365 samples (0.124 s) and is
returned as a plain number. The Pley server compares the fingerprints of
episodes to find shared intros and credits. Requests run two at a time, at
below-normal process priority, and pause while a playback starts or a
transcode runs (see *Background work and playback*), so they never slow down
playback. Whether the
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
within 5 s of a job creation or while a playback starts (`skipped`, see
*Background work and playback*), and one that begins meanwhile stops the
reading (`stopped`), as does the request going away.

## Background work and playback

Seek previews (`/v1/trickplay`), fingerprints and key frames
(`/v1/fingerprint`, `/v1/frames`) and keyframe scans (`/v1/keyframes`)
read whole files or long stretches of them, and a library scan's probes
(`/v1/probe`) the heads of many, from the disk or network share playbacks
read too: a 4K remux on a share needs all of it to start in a second or
two. So while a playback starts, none of them starts (probes wait), and the
FFmpeg (or FFprobe) runs under way are paused, their process suspended
(`NtSuspendProcess` on Windows, `SIGSTOP` on Linux; elsewhere they only
wait to start), then resumed. Nothing is lost: a paused run's time limit
counts the time it ran. A playback starts:

- while a job is being created (probe, keyframe index, FFmpeg's start);
- while one of its FFmpeg processes, at Play, a seek, a restart, hasn't
  written its first 3 segments, 20 s at most;
- for `ms` after `POST /v1/hold`, which the Pley server sends for a direct
  play (10 s): it serves those itself.

Transcodes come first too. While a job's FFmpeg or a conversion runs (it
holds a transcode slot, `running_processes`; a job's FFmpeg paused far
ahead of its player still counts, one stopped for an idle player doesn't),
the same runs wait and are paused, until 5 s after the last one ends, so a
seek's restart doesn't let them through. Probes and reading ahead wait for
starts only: they read little, and new files or the next playback
shouldn't wait for a film to end. The Pley server waits for transcodes as
well before it asks for seek previews or fingerprints.

Background runs and conversions start at below-normal process priority
(`nice` 10 on Linux), a job's FFmpeg at normal priority.

`/v1/status` counts the runs paused now (`background_paused`).

## Metrics

`GET /v1/status` returns:

- totals: jobs created and failed, processes started and failed, segments
  produced and served, bytes served, probes (with total probe time),
  thumbnails, subtitle extractions, keyframe scans
- running and hardware processes against their limits
- background FFmpeg runs paused for playback
- work-dir disk usage
- per job: state, video and audio plans (input → output, encoder, decoder,
  tone mapper), ffmpeg progress (fps, speed, bitrate, position, dropped and
  duplicated frames), restarts, bytes served, warnings and the last error

`GET /metrics` exposes the same data for Prometheus (`pley_transcoder_*`). The
Pley web UI shows it under *Settings → Dashboard*, next to the processors'
and memory's use that the server samples itself every 2 s
(`GET /api/system/resources`, docs/API.md): a managed transcoder and its
FFmpeg processes count there as Pley's (the server's process tree); an
external one doesn't.

## Benchmark

`scripts/bench-transcoder.py` (Python 3.9 or later, no modules to install)
runs the transcoder as it is built, over its API, on local files, and
reports for each job: the wait for the first segment and, for a seek, for
the one asked for; the speed (× real time); the processor time per media
second and the processors' use; the largest process's peak memory (on
Windows its private commit, on Linux its resident set) and all of
theirs (Windows); the bytes made. `--make-clips DIR --source <4K HDR file>`
cuts the clips it needs from a film (1080p H.264 with AAC, TrueHD and DTS
copies, ten-minute loops, a minute of PGS subtitles from FFmpeg's FATE
sample, an SRT). Cases: 4K HDR10 to 1080p SDR, 1080p to 720p, TrueHD to
AAC and DTS to AC-3 with the video copied, a remux, a seek, text and
picture subtitles drawn on 1080p and 4K HDR, and playbacks watched in real
time (`session-*`, `--session` seconds) with throttling on, which count
what was made and whether the player waited.

Cheaper computers are played with `--variants`: `gpu` (what the machine
has), `sw` (no GPU), `sw4` and `sw2` (no GPU, four or two processors: a
Windows job object's affinity, `sched_setaffinity` on Linux, one logical
processor per core); on Windows `--mem-mb` sets a memory limit. Several
builds (`--bin name=path`, repeated) run in turns that change order each
round, so a machine busy with other work weighs on each alike.

What it showed (a 12-core Ryzen with an RTX 5070, `sw4` for a small
computer): 4K HDR10 to 1080p tone mapped with zimg went from 0.3× to 1×
real time on four processors (5.9 to 3.7 processor seconds per second of
video) once zimg tone mapped at the output's size; picture subtitles on
4K, from 0.23× to 0.72× (and half the processor time with the GPU's
pipeline) once laid on the scaled frame; and three minutes watched in real
time cost 26 to 49% less processor time with the pause 90 s ahead, the
player never waiting.

```
python scripts/bench-transcoder.py --make-clips D:/bench --source 4k-hdr10.mkv
python scripts/bench-transcoder.py --media D:/bench --hdr 4k-hdr10.mkv \
    --bin before=old/pley-transcoder.exe --bin after=target/release/pley-transcoder.exe \
    --variants gpu,sw,sw4 --repeat 3 --out results.jsonl
```

## License

```
pley-transcoder  Copyright (c) 2026 the Pley authors. All rights reserved.
Restricted: its licence terms are to be determined, and no licence to copy,
modify or distribute it is granted until then. It comes with ABSOLUTELY NO
WARRANTY. See the LICENSE file.
```

FFmpeg is invoked as a separate program and has its own license. The FFmpeg
build used during development (`--enable-gpl --enable-version3`) is GPLv3.
