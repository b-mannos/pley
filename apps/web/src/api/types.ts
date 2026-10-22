// Mirrors docs/API.md. Keep in sync with the server.

export type Id = number;
/** Unix seconds. */
export type Timestamp = number;

export interface User {
  id: Id;
  username: string;
  /** Display name, or the username when there is none. */
  name: string;
  display_name: string | null;
  is_admin: boolean;
  /** A profile without a password, reached by switching from a home member. */
  managed: boolean;
  /** Listed in the household's profile switcher. */
  home: boolean;
  has_pin: boolean;
  avatar_url: string | null;
  /** "#38bdf8" */
  avatar_color: string | null;
  /** `false`: only the libraries in `library_ids`. */
  all_libraries: boolean;
  library_ids: Id[];
  /** Age limit for movies and shows; `null` = none. */
  max_age: number | null;
  disabled: boolean;
  created_at: Timestamp;
}

/** A signed-in session (browser, app). */
export interface Device {
  id: string;
  name: string;
  user_agent: string | null;
  ip: string | null;
  created_at: Timestamp;
  last_seen_at: Timestamp;
  /** The session making this request. */
  current: boolean;
}

export interface Invite {
  code: string;
  created_at: Timestamp;
  expires_at: Timestamp | null;
  uses_left: number;
  all_libraries: boolean;
  library_ids: Id[];
  max_age: number | null;
  note: string | null;
}

export interface InviteCheck {
  server_name: string;
  expires_at: Timestamp | null;
}

export interface ServerStatus {
  name: string;
  version: string;
  setup_required: boolean;
}

export interface LoginResponse {
  user: User;
  token: string;
}

export type LibraryKind = "movies" | "shows" | "music";

export interface Library {
  id: Id;
  name: string;
  kind: LibraryKind;
  paths: string[];
  item_count: number;
  created_at: Timestamp;
  scanned_at: Timestamp | null;
  scanning: boolean;
}

export type ItemKind = "movie" | "show" | "season" | "episode" | "artist" | "album" | "track";

export interface UserState {
  position_ms: number;
  watched: boolean;
  play_count: number;
  last_played_at: Timestamp | null;
}

export interface ItemSummary {
  id: Id;
  library_id: Id;
  kind: ItemKind;
  title: string;
  sort_title: string;
  year: number | null;
  index: number | null;
  parent_id: Id | null;
  parent_title: string | null;
  parent_index: number | null;
  grandparent_id: Id | null;
  grandparent_title: string | null;
  duration_ms: number | null;
  rating: number | null;
  added_at: Timestamp;
  poster_url: string | null;
  backdrop_url: string | null;
  thumb_url: string | null;
  user_state: UserState;
  leaf_count: number | null;
  viewed_leaf_count: number | null;
  child_count: number | null;
}

export interface ItemDetail extends ItemSummary {
  original_title: string | null;
  summary: string | null;
  tagline: string | null;
  genres: string[];
  studio: string | null;
  content_rating: string | null;
  release_date: string | null;
  tmdb_id: number | null;
  imdb_id: string | null;
  files: MediaFile[];
  /** Fields edited by hand; metadata refreshes keep them. */
  locked_fields?: string[];
  /** Intro and credits of the files (absent on servers without detection). */
  markers?: Marker[];
}

export type MarkerKind = "intro" | "credits";

export interface Marker {
  file_id: Id;
  kind: MarkerKind;
  start_ms: number;
  end_ms: number;
  source: "chapter" | "fingerprint";
}

export interface MediaFile {
  id: Id;
  path: string;
  size: number;
  container: string | null;
  duration_ms: number | null;
  bitrate: number | null;
  width: number | null;
  height: number | null;
  video_codec: string | null;
  audio_codec: string | null;
  streams: MediaStream[];
  /** Tracks to start with: the user's remembered choice for this show or movie. */
  selected_audio: number | null;
  selected_subtitle: number | null;
  chapters: Chapter[];
}

export interface Chapter {
  start_ms: number;
  end_ms: number;
  title: string | null;
}

/** Seek-bar preview sprite sheets of a file (`GET /api/files/{id}/trickplay`). */
export interface Trickplay {
  id: string;
  interval_ms: number;
  width: number;
  height: number;
  columns: number;
  rows: number;
  count: number;
  sheets: number;
}

export type StreamKind = "video" | "audio" | "subtitle";

export type DynamicRange = "sdr" | "hdr10" | "hdr10plus" | "hlg" | "dolby_vision";

export interface MediaStream {
  index: number;
  kind: StreamKind;
  codec: string;
  language: string | null;
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
  dynamic_range?: DynamicRange;
  /** Dolby Vision profile (5, 7, 8, …). */
  dv_profile?: number;
  /** DV base-layer compatibility id (1 = HDR10, 4 = HLG, 0 = none). */
  dv_bl_compat?: number;
  profile?: string;
  // audio
  channels?: number;
  channel_layout?: string;
  sample_rate?: number;
  // subtitle
  external?: boolean;
  text_based?: boolean;
}

export interface Hub {
  key: string;
  title: string;
  library_id: Id | null;
  items: ItemSummary[];
}

export interface SubtitleTrack {
  index: number;
  codec: string;
  language: string | null;
  title: string | null;
  default: boolean;
  forced: boolean;
  external: boolean;
  text_based: boolean;
  url: string | null;
}

export type PlaybackMethod = "direct" | "remux" | "transcode";

export interface PlaybackInfo {
  session_id: string;
  method: PlaybackMethod;
  url: string;
  hls: boolean;
  file_id: Id;
  duration_ms: number;
  audio_index: number | null;
  subtitle_index: number | null;
  burn_subtitles: boolean;
  subtitles: SubtitleTrack[];
  reason: string | null;
}

export interface ClientCaps {
  containers: string[];
  video_codecs: string[];
  audio_codecs: string[];
  /** Shows HDR10 / HLG without tone mapping. */
  hdr?: boolean;
  /** Decodes Dolby Vision (needed for profile 5). */
  dolby_vision?: boolean;
}

/** `auto` lets the server pick the cheapest method the client supports. */
export type PlaybackMode = "auto" | "direct" | "remux" | "transcode";

export interface PlaybackRequest {
  item_id: Id;
  file_id?: Id;
  audio_index?: number | null;
  subtitle_index?: number | null;
  max_bitrate?: number | null;
  max_height?: number | null;
  start_ms?: number;
  mode?: PlaybackMode;
  /** Identifies the player; its previous sessions end when it starts a new one. */
  client_id?: string;
  caps: ClientCaps;
}

export type PlaybackState = "playing" | "paused" | "stopped";

export interface ProgressRequest {
  position_ms: number;
  duration_ms?: number;
  state: PlaybackState;
  session_id?: string;
}

export type ScanPhase = "queued" | "walking" | "probing" | "metadata" | "done";

export interface ScanStatus {
  library_id: Id;
  library_name: string;
  phase: ScanPhase;
  processed: number;
  total: number;
  started_at: Timestamp;
}

export type TranscodeJobState = "starting" | "running" | "idle" | "finished" | "failed";

export type TranscodePipeline = "gpu" | "hybrid" | "cpu" | "copy";

export interface TranscodeInfo {
  job_id: string;
  state: TranscodeJobState;
  video: "copy" | "transcode" | "none";
  audio: "copy" | "transcode" | "none";
  video_in: string | null;
  video_out: string | null;
  audio_in: string | null;
  audio_out: string | null;
  encoder: string | null;
  decoder: string | null;
  /** Where frames are processed: zero-copy on one GPU, mixed, CPU only, or stream copy. */
  pipeline?: TranscodePipeline | null;
  tonemap: string | null;
  subtitle_burn: boolean;
  speed: number | null;
  fps: number | null;
  transcoded_ms: number;
  segments_ready: number;
  segments_total: number;
  last_error: string | null;
}

export interface SessionInfo {
  session_id: string;
  user_id: Id;
  username: string;
  item: ItemSummary;
  method: PlaybackMethod;
  position_ms: number;
  state: PlaybackState;
  started_at: Timestamp;
  updated_at: Timestamp;
  transcode: TranscodeInfo | null;
}

export interface Activity {
  scans: ScanStatus[];
  sessions: SessionInfo[];
  /** Intro/credits detection in progress (admins only). */
  analysis?: AnalysisStatus | null;
}

export interface AnalysisStatus {
  item_id: Id;
  title: string;
  done: number;
  total: number;
}

export type ServerEvent =
  | { type: "scan_started"; library_id: Id }
  | { type: "scan_progress"; library_id: Id; phase: ScanPhase; processed: number; total: number }
  | { type: "scan_finished"; library_id: Id; added: number; updated: number; removed: number }
  | { type: "library_changed"; library_id: Id }
  | { type: "libraries_changed" }
  | { type: "sessions_changed" };

export type HwAccel = "auto" | "none" | "nvenc" | "qsv" | "amf" | "vaapi" | "videotoolbox";
export type TranscodePreset = "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium";
export type ToneMapping = "auto" | "libplacebo" | "opencl" | "zscale" | "none";

export interface Settings {
  server_name: string;
  tmdb_api_key: string;
  metadata_language: string;
  hwaccel: HwAccel;
  hardware_decoding: boolean;
  tonemapping: ToneMapping;
  max_transcodes: number;
  transcode_preset: TranscodePreset;
  throttle_transcodes: boolean;
  scan_interval_minutes: number;
  watch_libraries: boolean;
  detect_markers: boolean;
}

export interface TranscoderInfo {
  available: boolean;
  managed: boolean;
  url: string | null;
  version: string | null;
  license: string | null;
  ffmpeg_version: string | null;
  encoders: string[];
  active_encoder: string | null;
  hwaccels: string[];
  tonemapping: string[];
  dolby_vision: boolean;
  max_jobs: number;
  active_jobs: number;
  error: string | null;
}

export interface TranscoderCounters {
  jobs_created: number;
  jobs_failed: number;
  processes_started: number;
  process_failures: number;
  segments_produced: number;
  segments_served: number;
  bytes_served: number;
  probes: number;
  probe_failures: number;
  probe_ms_total: number;
  thumbnails: number;
  subtitle_extractions: number;
  keyframe_scans: number;
}

export interface TranscoderJob {
  id: string;
  label: string | null;
  owner: string | null;
  state: TranscodeJobState;
  created_at_ms: number;
  last_access_ms: number;
  info: TranscodeInfo;
  restarts: number;
  served_bytes: number;
  warnings: string[];
}

export interface TranscoderStatus {
  available: boolean;
  version: string | null;
  uptime_secs: number;
  max_jobs: number;
  running_processes: number;
  hw_processes: number;
  max_hw_jobs: number;
  work_dir_bytes: number;
  counters: TranscoderCounters;
  jobs: TranscoderJob[];
}

export interface SystemInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
  uptime_secs: number;
  data_dir: string;
  transcoder: TranscoderInfo;
  counts: {
    libraries: number;
    movies: number;
    shows: number;
    episodes: number;
    artists: number;
    albums: number;
    tracks: number;
    files: number;
    users: number;
  };
}

export interface FsEntry {
  name: string;
  path: string;
}

export interface FsListing {
  path: string | null;
  parent: string | null;
  entries: FsEntry[];
}

export type LibrarySort = "title" | "year" | "added" | "rating" | "release" | "duration" | "last_played";
export type SortOrder = "asc" | "desc";

export interface LibraryItemsQuery {
  sort?: LibrarySort;
  order?: SortOrder;
  watched?: boolean;
  genre?: string;
  year?: number;
  offset?: number;
  limit?: number;
}

export interface LibraryItemsResponse {
  total: number;
  items: ItemSummary[];
}

export interface LibraryFilters {
  genres: string[];
  years: number[];
}
