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
  /** Default tracks for titles without a remembered choice. ISO 639 code; `null`: the file's default track. */
  audio_language: string | null;
  /** `null`: the audio language. */
  subtitle_language: string | null;
  subtitle_mode: SubtitleMode;
  /** `false`: a managed profile, or an account without a password. */
  has_password: boolean;
  /** Signs in with a code from an authenticator app after the password. */
  two_step: boolean;
}

/**
 * When subtitles come on by themselves: never, only forced ones (foreign
 * dialogue), full ones when the audio is in another language, or always.
 */
export type SubtitleMode = "off" | "forced" | "foreign" | "always";

/** The account settings `PATCH /api/me` takes. */
export interface ProfilePatch {
  display_name?: string | null;
  avatar_color?: string | null;
  audio_language?: string | null;
  subtitle_language?: string | null;
  subtitle_mode?: SubtitleMode;
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
  /** The server build's short commit hash; "unknown" without the repository. */
  commit?: string;
  setup_required: boolean;
  /** Setting up takes the code in the server's log. */
  setup_code_required?: boolean;
}

/** `GET /api/me/two-step`: two-step sign-in for the signed-in user. */
export interface TwoStepStatus {
  enabled: boolean;
  /** When it was turned on. */
  since: Timestamp | null;
  recovery_codes_left: number;
}

/** `POST /api/me/two-step/setup`: a new secret for an authenticator app. */
export interface TwoStepSetup {
  /** Base32, for typing in. */
  secret: string;
  /** `otpauth://totp/…`: what the QR code says, and a link that opens an authenticator app on this device. */
  uri: string;
  /** The QR code: its width in modules, and an SVG path of its dark modules. */
  qr: { size: number; path: string };
  /** The secret must be confirmed with a code before then. */
  expires_at: Timestamp;
}

/** Shown once: each signs in once instead of a code from the app. */
export interface RecoveryCodes {
  recovery_codes: string[];
}

export interface LoginResponse {
  user: User;
  /** Only for clients that aren't browsers: browsers keep the session in a cookie. */
  token?: string;
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

export type ItemKind = "movie" | "show" | "season" | "episode" | "artist" | "album" | "track" | "extra";

export interface UserState {
  position_ms: number;
  watched: boolean;
  play_count: number;
  last_played_at?: Timestamp | null;
  /** The user's own rating, 0-10 (two points per star). */
  rating?: number | null;
}

/** Fields without a value are left out of the JSON (smaller lists). */
export interface ItemSummary {
  id: Id;
  library_id: Id;
  kind: ItemKind;
  title: string;
  sort_title: string;
  year?: number | null;
  index?: number | null;
  parent_id?: Id | null;
  parent_title?: string | null;
  parent_index?: number | null;
  grandparent_id?: Id | null;
  grandparent_title?: string | null;
  duration_ms?: number | null;
  rating?: number | null;
  added_at: Timestamp;
  poster_url?: string | null;
  backdrop_url?: string | null;
  thumb_url?: string | null;
  user_state: UserState;
  leaf_count?: number | null;
  viewed_leaf_count?: number | null;
  child_count?: number | null;
  summary?: string | null;
  release_date?: string | null;
  /** Bonus material: "trailer", "featurette", "behind_the_scenes", … */
  extra_type?: string | null;
  /** What the title's files are like: in listings sorted by bitrate. */
  media?: MediaSummary | null;
}

/** A title's best file (a show's files on average). */
export interface MediaSummary {
  /** Bits per second. */
  bitrate: number | null;
  /** Bytes (a show's: all its episodes'). */
  size: number | null;
  width: number | null;
  height: number | null;
  video_codec: string | null;
  dynamic_range: DynamicRange | null;
}

export interface ItemDetail extends ItemSummary {
  original_title: string | null;
  tagline: string | null;
  genres: string[];
  studio: string | null;
  content_rating: string | null;
  tmdb_id: number | null;
  imdb_id: string | null;
  files: MediaFile[];
  /** Fields edited by hand; metadata refreshes keep them. */
  locked_fields?: string[];
  /** Intro and credits of the files (absent on servers without detection). */
  markers?: Marker[];
  /** Trailers and other bonus material (movies and shows). */
  extras?: ItemSummary[];
  /** Actors in billing order; for episodes, the guest stars. */
  cast?: Credit[];
  /** Creators, directors, writers and producers. */
  crew?: Credit[];
  /** On the user's My List (movies and shows). */
  in_watchlist?: boolean;
}

export type CreditRole = "actor" | "creator" | "director" | "writer" | "producer";

/** A person credited in an item. */
export interface Credit {
  /** The person (`/person/{id}`). */
  id: Id;
  name: string;
  role: CreditRole;
  character?: string;
  photo_url?: string;
}

export interface PersonSummary {
  id: Id;
  name: string;
  photo_url?: string;
}

export interface PersonRole {
  role: CreditRole;
  character?: string;
  /** Credits from single episodes (guest parts): how many. */
  episodes?: number;
}

/** A movie or show a person is credited in (episode credits count towards the show). */
export interface PersonCredit {
  item: ItemSummary;
  roles: PersonRole[];
}

export interface PersonDetail extends PersonSummary {
  /** Newest first. */
  credits: PersonCredit[];
}

export interface HistoryUser {
  id: Id;
  name: string;
  avatar_url?: string | null;
  avatar_color?: string | null;
}

/** A play that reached the end (`GET /api/history`). */
export interface HistoryEntry {
  id: Id;
  viewed_at: Timestamp;
  /** "Chrome on Windows", a Plex client's name. */
  device?: string;
  user: HistoryUser;
  item: ItemSummary;
}

export interface HistoryPage {
  total: number;
  entries: HistoryEntry[];
}

/** Space used by what pley stores (`GET /api/system/storage`). */
export interface StorageUsage {
  blobs: number;
  blob_bytes: number;
  media_folders: number;
  media_bytes: number;
  cache_bytes: number;
}

export interface CleanReport {
  blobs_removed: number;
  media_folders_removed: number;
  bytes_freed: number;
}

export interface DatabaseHealth {
  ok: boolean | null;
  problem?: string;
  checked_at: Timestamp | null;
}

export interface Backup {
  name: string;
  path: string;
  bytes: number;
  created_at: Timestamp;
}

export interface Backups {
  health: DatabaseHealth;
  backups: Backup[];
}

export interface SearchResults {
  items: ItemSummary[];
  people?: PersonSummary[];
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
  /** The edition the file is tagged with (`{edition-Final Cut}`). */
  edition: string | null;
  /** Tracks to start with: the user's remembered choice for this show or movie. */
  selected_audio: number | null;
  selected_subtitle: number | null;
  chapters: Chapter[];
}

export interface Chapter {
  start_ms: number;
  end_ms: number;
  title: string | null;
  /** Picture of the chapter, once the file has seek previews. */
  thumb_url?: string;
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
  /** DV base-layer compatibility id (1 = HDR10, 2 = SDR, 4 = HLG, 6 = Blu-ray HDR10 (profile 7), 0 = none). */
  dv_bl_compat?: number;
  /** Dolby Vision level. */
  dv_level?: number;
  /** Profile 7's enhancement layer: full or minimal (absent: not known). */
  dv_el?: "fel" | "mel";
  /** HDR10+ in the first frames (Dolby Vision and PQ streams; absent: not looked for). */
  hdr10plus?: boolean;
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
  /** The audio is converted, also when the video is copied (a direct stream). */
  audio_converted?: boolean;
  subtitles: SubtitleTrack[];
  /** Why the file isn't played as it is, one short sentence each. */
  reasons?: string[];
  /** The same, in one line. */
  reason: string | null;
  /**
   * The quality a conversion of the original aims for (the closest one the
   * player offers, by id: "30m"…): absent when a quality was picked or the
   * video isn't converted.
   */
  auto_quality?: string | null;
  /**
   * What the screen gets of HDR video, by name: "Dolby Vision P8.1", "HDR10
   * (Dolby Vision base layer)", "SDR (tone-mapped)"… Absent for SDR video
   * and from older servers.
   */
  video_range?: string | null;
}

/** `GET /api/playback/{session}`: how a session plays, with its transcode's live numbers. */
export interface SessionStatus {
  session_id: string;
  method: PlaybackMethod;
  transcode: TranscodeInfo | null;
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
  /** The software encoder's preset now. */
  preset?: string | null;
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
  /** What the transcode aims for, video and audio together (bits per second). */
  bitrate_out?: number | null;
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
  /** Files waiting for seek previews (admins only). */
  previews_pending?: number;
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
  | { type: "sessions_changed" }
  /** My List changed on another of the user's devices. */
  | { type: "watchlist_changed" }
  /**
   * The user's watch state of an item changed, here or on another device:
   * `item_ids` is the item, then its season and show (album and artist);
   * `children`: everything under the first one changed too.
   */
  | { type: "user_state_changed"; item_ids: Id[]; children: boolean }
  /** A playback session was ended on the server (`DELETE /api/sessions/{id}`): the device playing it stops. */
  | { type: "session_stopped"; session_id: string; message: string | null; by_admin: boolean };

export type HwAccel = "auto" | "none" | "nvenc" | "qsv" | "amf" | "vaapi" | "videotoolbox";
/** `auto`: the transcoder picks one from its load and how far ahead of the player it is, and changes it as they change. */
export type TranscodePreset = "auto" | "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium";
export type ToneMapping = "auto" | "libplacebo" | "opencl" | "zscale" | "none";
/** Where TV show metadata comes from; "auto" is TMDB, else TheTVDB (when they have a key), else TVmaze. */
export type TvMetadata = "auto" | "tmdb" | "tvdb" | "tvmaze";

export interface Settings {
  server_name: string;
  tmdb_api_key: string;
  metadata_language: string;
  tv_metadata: TvMetadata;
  tvdb_api_key: string;
  /** Subscriber PIN, for TheTVDB keys that need one. */
  tvdb_pin: string;
  /** Artists and albums from MusicBrainz, covers from the Cover Art Archive. */
  music_metadata: boolean;
  hwaccel: HwAccel;
  hardware_decoding: boolean;
  tonemapping: ToneMapping;
  max_transcodes: number;
  transcode_preset: TranscodePreset;
  throttle_transcodes: boolean;
  scan_interval_minutes: number;
  watch_libraries: boolean;
  detect_markers: boolean;
  seek_previews: boolean;
  previews_in_background: boolean;
  /** Apps by other publishers may sign in (pley's own apps always can); turning it off signs them out. */
  third_party_apps: boolean;
  /**
   * Addresses with a port pley is reached at through a reverse proxy that
   * leaves the port out of `Host` (`https://media.example.com:9999`).
   * Absent from servers before it.
   */
  proxy_addresses?: string[];
}

/** A service the settings can test (`POST /api/settings/test`). */
export type TestedService = "tmdb" | "tvdb" | "tvmaze" | "musicbrainz" | "transcoder";

export interface ServiceTest {
  service: TestedService;
  /** A key typed in the form (not saved yet); the saved one when left out. */
  api_key?: string;
  pin?: string;
}

export interface ServiceTestResult {
  ok: boolean;
  message: string;
  /** How long the answer took. */
  ms: number;
}

export interface TranscoderInfo {
  available: boolean;
  managed: boolean;
  url: string | null;
  version: string | null;
  commit?: string | null;
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

/** Where pley listens: addresses such as `127.0.0.1:7539` (Settings → Network). */
export interface ListenChoice {
  /** The web app and its API. */
  web: string[];
  /** The Plex-compatible API: off when empty. */
  plex: string[];
  /** Whether the pley apps on the networks find the server. */
  discovery: boolean;
}

export type ListenerKind = "web" | "plex" | "discovery";

export interface NetworkListener {
  kind: ListenerKind;
  address: string;
  /** Why pley doesn't listen there (yet). */
  error: string | null;
}

export interface NetworkAdapter {
  /** "Wi-Fi", "Ethernet", "eth0". */
  name: string;
  /** The hardware, on Windows. */
  description: string | null;
  /** Connected. */
  up: boolean;
  /** Its IPv4 addresses. */
  addresses: string[];
  /** The network it is on, as Windows knows it. */
  network: { name: string; kind: "public" | "private" | "domain" } | null;
}

export interface NetworkSettings extends ListenChoice {
  /** What the command line or the environment sets: it can't change here. */
  fixed: { web: boolean; plex: boolean; discovery: boolean };
  listeners: NetworkListener[];
  adapters: NetworkAdapter[];
  /** Where a change is saved. */
  config_file: string;
  /** Windows Firewall decides which kinds of network reach pley. */
  windows: boolean;
}

export interface SystemInfo {
  name: string;
  version: string;
  commit?: string;
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

/** The machine and pley (the server, its transcoder and their FFmpegs) at one moment. */
export interface ResourceSample {
  /** Unix milliseconds. */
  at: number;
  /** Share of all the processors' time, 0–100. */
  cpu: number;
  pley_cpu: number;
  /** Bytes in use. */
  memory: number;
  pley_memory: number;
}

export interface Resources {
  available: boolean;
  interval_ms: number;
  cpus: number;
  memory_total: number;
  /** Oldest first, the last five minutes. */
  samples: ResourceSample[];
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

export type LibrarySort =
  | "title"
  | "year"
  | "added"
  | "rating"
  | "release"
  | "duration"
  | "last_played"
  | "user_rating"
  | "bitrate";
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
