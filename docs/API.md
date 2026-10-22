# pley HTTP API (v1)

All endpoints live under `/api`. Requests and responses are JSON (`snake_case` keys)
unless stated otherwise. Timestamps are **unix seconds**; durations/positions are
**milliseconds**. Ids are integers unless stated otherwise.

## Authentication

* `POST /api/auth/login` (or `/api/setup`) sets an `HttpOnly` cookie named
  `pley_token` and also returns the token in the body.
* Every other endpoint (except the public ones listed below) requires one of:
  * the `pley_token` cookie (what the web UI uses — `<img>`, `<video>`, hls.js
    requests carry it automatically because the UI is same-origin),
  * `Authorization: Bearer <token>`,
  * `X-Pley-Token: <token>` header,
  * `?token=<token>` query parameter (handy for external players like VLC).
* Missing/invalid token → `401`. Non-admin calling an admin endpoint → `403`.

### Errors

Non-2xx responses carry:

```json
{ "code": "not_found", "message": "item 42 not found" }
```

`code` is one of `bad_request`, `unauthorized`, `forbidden`, `not_found`,
`conflict`, `unavailable`, `internal`.

---

## Types

```ts
type Id = number;
type Timestamp = number; // unix seconds

interface User {
  id: Id;
  username: string;
  is_admin: boolean;
  created_at: Timestamp;
}

type LibraryKind = "movies" | "shows" | "music";

interface Library {
  id: Id;
  name: string;
  kind: LibraryKind;
  paths: string[];
  item_count: number;           // number of top-level items (movies / shows / artists)
  created_at: Timestamp;
  scanned_at: Timestamp | null;
  scanning: boolean;            // a scan is queued or running right now
}

type ItemKind = "movie" | "show" | "season" | "episode" | "artist" | "album" | "track";

interface UserState {
  position_ms: number;          // resume position, 0 if none
  watched: boolean;             // containers: true when every leaf is watched
  play_count: number;
  last_played_at: Timestamp | null;
}

interface ItemSummary {
  id: Id;
  library_id: Id;
  kind: ItemKind;
  title: string;
  sort_title: string;
  year: number | null;
  index: number | null;             // season number / episode number / track number
  parent_id: Id | null;             // season for episodes, show for seasons, album for tracks, artist for albums
  parent_title: string | null;
  parent_index: number | null;      // season number for episodes, disc number for tracks
  grandparent_id: Id | null;        // show for episodes, artist for tracks
  grandparent_title: string | null;
  duration_ms: number | null;
  rating: number | null;            // 0..10
  added_at: Timestamp;
  poster_url: string | null;        // e.g. "/api/items/12/images/poster?v=1729600000" (append &w=<px>)
  backdrop_url: string | null;
  thumb_url: string | null;         // episode stills / video frame grabs (16:9)
  user_state: UserState;
  leaf_count: number | null;        // show/season: #episodes, artist/album: #tracks
  viewed_leaf_count: number | null; // show/season: #watched episodes
  child_count: number | null;       // show: #seasons, artist: #albums
}

interface ItemDetail extends ItemSummary {
  original_title: string | null;
  summary: string | null;
  tagline: string | null;
  genres: string[];
  studio: string | null;
  content_rating: string | null;    // e.g. "PG-13"
  release_date: string | null;      // "YYYY-MM-DD"
  tmdb_id: number | null;
  imdb_id: string | null;
  files: MediaFile[];               // empty for containers (show/season/artist/album)
  markers: Marker[];                // intro and credits of the files (Skip Intro / Skip Credits)
}

interface Marker {
  file_id: Id;                      // markers belong to a file: versions can be different cuts
  kind: "intro" | "credits";
  start_ms: number;
  end_ms: number;                   // credits that run to the end of the file end at its duration
  source: "chapter" | "fingerprint";
}

interface MediaFile {
  id: Id;
  path: string;
  size: number;                     // bytes
  container: string | null;         // "mkv" | "mp4" | "webm" | "avi" | "mp3" | "flac" | ...
  duration_ms: number | null;
  bitrate: number | null;           // bits per second
  width: number | null;
  height: number | null;
  video_codec: string | null;       // ffprobe codec name: "h264", "hevc", "av1", ...
  audio_codec: string | null;       // codec of the default audio stream
  streams: MediaStream[];
  selected_audio: number | null;    // tracks to start with for this user (remembered
  selected_subtitle: number | null; // choice for the show or movie); null = default / off
}

interface MediaStream {
  index: number;                    // ffprobe stream index; external subtitle files use 1000+
  kind: "video" | "audio" | "subtitle";
  codec: string;
  language: string | null;          // ISO 639-2 as found in the file, e.g. "eng"
  title: string | null;
  default: boolean;
  forced: boolean;
  bitrate?: number;
  // video
  width?: number;
  height?: number;
  frame_rate?: number;
  bit_depth?: number;
  hdr?: boolean;
  dynamic_range?: "sdr" | "hdr10" | "hdr10plus" | "hlg" | "dolby_vision";
  dv_profile?: number;              // Dolby Vision profile (5, 7, 8, …)
  dv_bl_compat?: number;            // DV base-layer compatibility id (1 = HDR10, 4 = HLG, 0 = none)
  profile?: string;
  // audio
  channels?: number;
  channel_layout?: string;
  sample_rate?: number;
  // subtitle
  external?: boolean;               // sidecar file next to the media
  text_based?: boolean;             // srt/ass/webvtt/mov_text (true) vs pgs/vobsub (false, needs burn-in)
}

interface Hub {
  key: string;                      // "continue_watching" | "recently_added:<library_id>" | ...
  title: string;
  library_id: Id | null;
  items: ItemSummary[];
}

interface SubtitleTrack {
  index: number;
  codec: string;
  language: string | null;
  title: string | null;
  default: boolean;
  forced: boolean;
  external: boolean;
  text_based: boolean;
  url: string | null;               // WebVTT url for text-based subs, null for image subs
}

interface PlaybackInfo {
  session_id: string;
  method: "direct" | "remux" | "transcode";
  url: string;                      // direct: file url; otherwise HLS playlist url
  hls: boolean;
  file_id: Id;
  duration_ms: number;
  audio_index: number | null;
  subtitle_index: number | null;    // the subtitle that is burned in (only when burn_subtitles)
  burn_subtitles: boolean;
  subtitles: SubtitleTrack[];       // all subtitle tracks of the file
  reason: string | null;            // human readable "why not direct play"
}
```

---

## Public endpoints

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/health` | – | `{ "status": "ok" }` |
| GET | `/api/server` | – | `{ name: string, version: string, setup_required: boolean }` |
| POST | `/api/setup` | `{ username, password }` | `{ user: User, token: string }` — creates the first (admin) account and logs in. `409` once a user exists. |
| POST | `/api/auth/login` | `{ username, password }` | `{ user: User, token: string }` (`401` on bad credentials) |

## Auth

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/auth/logout` | – | `204`, cookie cleared |
| GET | `/api/auth/me` | – | `User` |
| PUT | `/api/auth/password` | `{ current_password, new_password }` | `204` |

## Libraries

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/libraries` | – | `Library[]` |
| POST | `/api/libraries` *(admin)* | `{ name, kind, paths: string[] }` | `Library` (a scan is queued automatically) |
| GET | `/api/libraries/{id}` | – | `Library` |
| PATCH | `/api/libraries/{id}` *(admin)* | `{ name?, paths? }` | `Library` |
| DELETE | `/api/libraries/{id}` *(admin)* | – | `204` |
| POST | `/api/libraries/{id}/scan` *(admin)* | `{ force?: boolean }` (force = re-probe files and refresh metadata) | `202` |
| POST | `/api/libraries/scan` *(admin)* | – | `202` (scan all) |
| GET | `/api/libraries/{id}/items` | query: `sort=title\|year\|added\|rating\|release\|duration\|last_played` (default `title`), `order=asc\|desc`, `watched=true\|false`, `genre=<name>`, `year=<yyyy>`, `offset`, `limit` | `{ total: number, items: ItemSummary[] }` — top-level items only |
| GET | `/api/libraries/{id}/filters` | – | `{ genres: string[], years: number[] }` |
| GET | `/api/libraries/{id}/hubs` | – | `{ hubs: Hub[] }` (continue watching, recently added, recently released, top rated – empty hubs omitted) |

## Items

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/items/{id}` | – | `ItemDetail` |
| GET | `/api/items/{id}/children` | – | `{ items: ItemSummary[] }` (seasons of a show, episodes of a season, albums of an artist, tracks of an album), ordered by index |
| GET | `/api/items/{id}/on-deck` | – | `{ item: ItemSummary \| null }` – for show/season: the episode to play when pressing *Play* (in progress, else next unwatched, else first) |
| GET | `/api/items/{id}/next` | – | `{ item: ItemSummary \| null }` – episode/track following this one (for autoplay) |
| POST | `/api/items/{id}/progress` | `{ position_ms, duration_ms?, state: "playing"\|"paused"\|"stopped", session_id? }` | `204`. ≥ 90 % marks the item watched and clears the resume point. |
| POST | `/api/items/{id}/watched` | – | `204` (containers: all leaves) |
| PUT | `/api/items/{id}/streams` | `{ file_id?, audio_index, subtitle_index }` (`subtitle_index: null` = off) | `204`. Remembers the choice for the whole show (episodes) or the movie, as language/title/forced, so every episode starts with its own matching tracks. |
| POST | `/api/items/{id}/unwatched` | – | `204` |
| POST | `/api/items/{id}/refresh` *(admin)* | – | `202` re-fetch metadata |
| POST | `/api/items/{id}/analyze` *(admin)* | – | `202` find the intro and credits again: a movie's chapters, or every episode of an episode's season, a season or a show. `400` for other kinds. |
| GET | `/api/items/{id}/images/{poster\|backdrop\|thumb}` | query: `w=<px>` (optional resize, bucketed), `v=` (cache buster, ignored) | image bytes (`image/jpeg`), long-lived cache headers. Falls back to parent artwork (season → show). `404` if none. |

## Home & search

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/api/hubs/home` | – | `{ hubs: Hub[] }` – `continue_watching` (in-progress + next up), then `recently_added:<library_id>` per library |
| GET | `/api/search` | `q`, `limit` (default 40) | `{ items: ItemSummary[] }` ranked by relevance |

## Playback

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/playback` | see below | `PlaybackInfo` |
| DELETE | `/api/playback/{session_id}` | – | `204` (stops any transcode) |
| GET | `/api/playback/{session_id}/index.m3u8` | – | HLS VOD playlist (`application/vnd.apple.mpegurl`) |
| GET | `/api/playback/{session_id}/{n}.ts` | – | MPEG-TS segment |
| GET | `/api/files/{id}/stream` | – | raw file, supports `Range` (direct play) |
| GET | `/api/files/{id}/subtitles/{index}.vtt` | – | WebVTT (text subtitles; extracted/converted on demand and cached) |

`POST /api/playback` body:

```ts
{
  item_id: Id;
  file_id?: Id;                   // default: first file of the item
  audio_index?: number | null;    // ffprobe stream index, default: the file's default audio stream
  subtitle_index?: number | null; // only needed for image-based subs (forces burn-in transcode)
  max_bitrate?: number | null;    // kbps, null/absent = original quality
  max_height?: number | null;     // e.g. 1080, 720, 480
  start_ms?: number;              // resume position; transcodes start producing there
  mode?: "auto" | "direct" | "remux" | "transcode"; // default "auto"
  client_id?: string;             // identifies the player (browser tab, device)
  caps: {                         // what the client can decode natively
    containers: string[];         // e.g. ["mp4","m4v","mov","webm","mp3","m4a","flac","ogg","wav"]
    video_codecs: string[];       // e.g. ["h264","hevc","vp9","av1"]
    audio_codecs: string[];       // e.g. ["aac","mp3","opus","flac","vorbis"]
    hdr?: boolean;                // shows HDR10 / HLG without tone mapping
    dolby_vision?: boolean;       // decodes Dolby Vision (needed for profile 5)
  };
}
```

Decision (`mode: "auto"`): **direct** when container + codecs are all supported,
the dynamic range can be shown, the requested audio track is the default one,
nothing must be burned in and the bitrate fits; otherwise **remux** (HLS, video
stream copied) or **transcode** (HLS, H.264/AAC).

- HDR10 and HLG need `caps.hdr`. Dolby Vision profile 5 needs
  `caps.dolby_vision`; profiles 7 and 8 play directly with either, because their
  base layer is HDR10 (8.1) or HLG (8.4). Otherwise the video is tone-mapped to
  SDR.
- `mode: "direct"` always returns the original file (useful when a browser plays
  more than it reports; image subtitles and alternate audio tracks are then
  unavailable). `"remux"` and `"transcode"` force HLS; `"transcode"` always
  re-encodes the video.
- Starting a session with a `client_id` ends that user's previous sessions on
  the same client (quality or audio switch, next episode) before the transcode
  limit is checked.
- If the transcoder loses a job (restart, expiry), the server recreates it with
  the same segment layout. When that is impossible, segment requests answer
  `404 not_found` ("playback session expired") and the player should start a
  new session at its current position.
The HLS playlist covers the whole file (VOD) so seeking works natively; the
server restarts ffmpeg at the right segment on seeks. Audio-only items (tracks)
use the same endpoint and get either a direct URL or an audio-only HLS stream.

The player should send `POST /api/items/{id}/progress` every ~10 s while
playing (with `session_id`), on pause, and with `state: "stopped"` when closing,
then `DELETE /api/playback/{session_id}`.

## Intro and credits detection

Markers come from two sources, best first:

1. **Chapters** titled like an intro or credits (*Intro*, *Opening*, *OP*,
   *Opening Credits*, *Ending*, *ED*, *Credits*, *End Credits*, …), for movies
   and episodes.
2. **Audio fingerprints** of episodes. The transcoder computes Chromaprint
   fingerprints of the first half (at most 10 minutes) and the last 35 % (at
   most 8 minutes) of every episode. An intro is audio of 12 s to 3 minutes
   near the start that the nearest episodes of the season (or of the show, for
   a season with one episode) share; credits are shared audio near the end.
   Each episode is compared with up to four others and the boundaries agreed
   on by most comparisons win. Fingerprints are cached per file version in the
   cache folder, so a new episode only needs its own.

A background worker analyses one season at a time after library scans (and
30 s after start-up), pauses while two or more transcodes run, and records the
algorithm version so improvements re-run automatically. It is skipped when
`detect_markers` is off or the transcoder's FFmpeg lacks Chromaprint (chapters
are still used).

## Activity & events

| Method | Path | Response |
|---|---|---|
| GET | `/api/activity` | `{ scans: ScanStatus[], sessions: SessionInfo[], analysis: AnalysisStatus \| null }` (non-admins only see their own sessions and no analysis) |
| GET | `/api/events` | Server-Sent Events, `data:` is JSON `ServerEvent` |

```ts
interface AnalysisStatus {          // intro/credits detection of one season
  item_id: Id;                      // the season
  title: string;                    // "Show — Season 1"
  done: number;                     // audio fingerprints computed
  total: number;
}

interface ScanStatus {
  library_id: Id;
  library_name: string;
  phase: "queued" | "walking" | "probing" | "metadata" | "done";
  processed: number;
  total: number;
  started_at: Timestamp;
}

interface SessionInfo {
  session_id: string;
  user_id: Id;
  username: string;
  item: ItemSummary;
  method: "direct" | "remux" | "transcode";
  position_ms: number;
  state: "playing" | "paused" | "stopped";
  started_at: Timestamp;
  updated_at: Timestamp;
  transcode: null | TranscodeInfo;
}

interface TranscodeInfo {
  job_id: string;
  state: "starting" | "running" | "idle" | "finished" | "failed";
  video: "copy" | "transcode" | "none";
  audio: "copy" | "transcode" | "none";
  video_in: string | null;      // e.g. "HEVC 3840×2160 Dolby Vision P8.1"
  video_out: string | null;     // e.g. "H.264 1920×1080"
  audio_in: string | null;      // e.g. "EAC3 5.1"
  audio_out: string | null;     // e.g. "AAC 2.0"
  encoder: string | null;       // e.g. "h264_nvenc", "libx264"
  decoder: string | null;       // "cuda", "vulkan", "cpu", …
  pipeline: string | null;      // "gpu" (zero-copy), "hybrid", "cpu" or "copy"
  tonemap: string | null;       // "libplacebo" | "opencl" | "zscale" when HDR/DV is tone-mapped to SDR
  subtitle_burn: boolean;
  speed: number | null;         // ffmpeg speed multiplier (2.5 = 2.5× realtime)
  fps: number | null;
  transcoded_ms: number;        // how far ffmpeg got (absolute media position)
  segments_ready: number;
  segments_total: number;
  last_error: string | null;
}

type ServerEvent =
  | { type: "scan_started"; library_id: Id }
  | { type: "scan_progress"; library_id: Id; phase: ScanStatus["phase"]; processed: number; total: number }
  | { type: "scan_finished"; library_id: Id; added: number; updated: number; removed: number }
  | { type: "library_changed"; library_id: Id }      // items/metadata changed → refetch
  | { type: "libraries_changed" }                    // library created/updated/deleted
  | { type: "sessions_changed" };
```

## Users *(admin)*

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/users` | – | `User[]` |
| POST | `/api/users` | `{ username, password, is_admin }` | `User` |
| PATCH | `/api/users/{id}` | `{ password?, is_admin? }` | `User` |
| DELETE | `/api/users/{id}` | – | `204` (cannot delete yourself) |

## System *(admin)*

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/system` | – | `SystemInfo` |
| GET | `/api/transcoder` | – | `TranscoderStatus` – live metrics of the transcoder component (all jobs, counters) |
| GET | `/api/settings` | – | `Settings` |
| PUT | `/api/settings` | partial `Settings` | `Settings` |
| GET | `/api/fs` | `path` (omit for roots/drives) | `{ path: string \| null, parent: string \| null, entries: { name: string, path: string }[] }` – directories only |

```ts
interface Settings {
  server_name: string;
  tmdb_api_key: string;             // "" = metadata only from file names / local artwork
  metadata_language: string;        // e.g. "en-US"
  hwaccel: "auto" | "none" | "nvenc" | "qsv" | "amf" | "vaapi" | "videotoolbox";
  hardware_decoding: boolean;       // decode on the GPU when possible
  tonemapping: "auto" | "libplacebo" | "opencl" | "zscale" | "none"; // HDR10/HLG/Dolby Vision → SDR
  max_transcodes: number;
  transcode_preset: "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium";
  throttle_transcodes: boolean;     // limit ffmpeg to ~2× realtime once a buffer is built
  scan_interval_minutes: number;    // 0 disables periodic scans
  watch_libraries: boolean;         // pick up file changes automatically
  detect_markers: boolean;          // find intros and credits in the background (default true)
}

interface SystemInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
  uptime_secs: number;
  data_dir: string;
  transcoder: {
    available: boolean;
    managed: boolean;               // spawned and supervised by pley (vs. external URL)
    url: string | null;
    version: string | null;
    license: string | null;         // "LicenseRef-Restricted"
    ffmpeg_version: string | null;
    encoders: string[];             // working video encoders, e.g. ["h264_nvenc", "libx264"]
    active_encoder: string | null;  // encoder used for new transcodes with current settings
    hwaccels: string[];
    tonemapping: string[];          // available tone mappers, e.g. ["libplacebo", "opencl", "zscale"]
    dolby_vision: boolean;          // Dolby Vision profile 5 can be tone-mapped correctly
    max_jobs: number;
    active_jobs: number;
    error: string | null;
  };
  counts: {
    libraries: number; movies: number; shows: number; episodes: number;
    artists: number; albums: number; tracks: number; files: number; users: number;
  };
}

interface TranscoderStatus {
  available: boolean;
  version: string | null;
  uptime_secs: number;
  max_jobs: number;
  running_processes: number;        // ffmpeg processes currently running
  hw_processes: number;
  max_hw_jobs: number;
  work_dir_bytes: number;           // disk used by segments
  counters: {
    jobs_created: number; jobs_failed: number; processes_started: number; process_failures: number;
    segments_produced: number; segments_served: number; bytes_served: number;
    probes: number; probe_failures: number; probe_ms_total: number;
    thumbnails: number; subtitle_extractions: number; keyframe_scans: number;
  };
  jobs: {
    id: string;
    label: string | null;
    owner: string | null;
    state: "starting" | "running" | "idle" | "finished" | "failed";
    created_at_ms: number;
    last_access_ms: number;
    info: TranscodeInfo;
    restarts: number;
    served_bytes: number;
    warnings: string[];
  }[];
}
```
