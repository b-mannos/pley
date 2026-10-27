# pley HTTP API (v1)

All endpoints live under `/api`. Requests and responses are JSON (`snake_case` keys)
unless stated otherwise. Timestamps are **unix seconds**; durations/positions are
**milliseconds**. Ids are integers unless stated otherwise.

## Authentication

* `POST /api/auth/login` (or `/api/setup`, `/api/auth/switch`, accepting an
  invitation) sets an `HttpOnly` cookie named `pley_token`.
  Clients that aren't browsers (requests without an `Origin` header) also get
  the token in the body; browsers keep it in the cookie only, so no script on
  the page ever sees it.
* Every other endpoint (except the public ones listed below) requires one of:
  * the `pley_token` cookie (what the web UI uses — `<img>`, `<video>`, hls.js
    requests carry it automatically because the UI is same-origin),
  * `Authorization: Bearer <token>`,
  * `X-Pley-Token: <token>` header,
  * `?token=<token>` query parameter (handy for external players like VLC).
* Missing/invalid token → `401`. Non-admin calling an admin endpoint → `403`.
* Apps signed in on pley's own page (see [Apps](#apps-oauth-21)) send their
  access token (`pley_at_…`) in `Authorization: Bearer` only: never in the
  cookie, `X-Pley-Token` or `?token=`. The compatibility API refuses it.
  Its refusals carry `WWW-Authenticate` (RFC 6750 §3):
  * `400` `bad_request` with `Bearer error="invalid_request"`: an app's token
    anywhere but the Authorization header;
  * `401` `unauthorized` with `Bearer error="invalid_token"`: an expired,
    signed-out or unknown token, a refresh token sent as a bearer token, or a
    disabled account;
  * `403` `insufficient_scope` with
    `Bearer error="insufficient_scope", scope="play"`: a route the token's
    scopes don't open. `scope` names the scope that would; it is left out
    when none does ("apps can't do that: use pley's web app").

Every answer carries `X-Content-Type-Options: nosniff`, `X-Frame-Options:
DENY`, `Referrer-Policy: same-origin` and a `Content-Security-Policy` that
lets pages load only pley's own scripts and styles. Password checks queue
(a few at a time); when one has waited 10 seconds the answer is `503`.

pley's Samsung and LG TV apps are web apps their TVs load from the TV
itself, and their web engines hold them to CORS: their requests come with
`Origin: file://` (or `null`, or an `app://` address). A website's page
in a sandboxed frame sends `null` too, but these answers only ever carry
what a token in `Authorization` opens, never a cookie's. For those origins only, the API and
`/.well-known/oauth-authorization-server` answer CORS: a preflight lets
`Authorization`, `Content-Type`, `Accept` and `Range` through for 10
minutes, and answers carry `Access-Control-Allow-Origin: *` and expose
`WWW-Authenticate`, `X-Pley-Complete`, `ETag`, `Content-Range` and
`Retry-After`. A wildcard never lets a page read an answer made with
cookies, and the consent page's endpoints (`/api/oauth/requests…`,
`/api/oauth/device/verify`) never answer CORS.

### Errors

Non-2xx responses carry:

```json
{ "code": "not_found", "message": "item 42 not found" }
```

`code` is one of `bad_request`, `unauthorized`, `forbidden`, `not_found`,
`conflict`, `too_many_requests` (sign-in, PIN, password and code attempts:
after 5 failures in 15 minutes, counting attempts still being checked;
sign-ins count per account from each address, and 50 from all of them),
`two_step` (`401`: the account has two-step sign-in, and the code is missing
or wrong), `insufficient_scope` (`403`: an app's token on a route its scopes
don't open), `unavailable`, `internal`. The token and revocation endpoints
of apps answer in OAuth's own form instead (see
[Tokens](#tokens)).

A playback session stopped from elsewhere answers its player's next
requests with `410` `{ code: "session_stopped", error: "session_stopped",
message }`, where `message` is what the one who stopped it wrote, or `null`
(see [Stopping a stream](#stopping-a-stream)).

---

## Types

```ts
type Id = number;        // a library's, item's, file's or user's is never given to another, also once deleted
type Timestamp = number; // unix seconds

interface User {
  id: Id;
  username: string;
  name: string;                    // display name, or the username
  display_name: string | null;
  is_admin: boolean;
  managed: boolean;                // profile without a password, reached by switching
  home: boolean;                   // listed in the household's profile switcher
  has_pin: boolean;
  avatar_url: string | null;
  avatar_color: string | null;     // "#38bdf8"
  all_libraries: boolean;          // false: only `library_ids`
  library_ids: Id[];
  max_age: number | null;          // age limit for movies and shows; null = none
  disabled: boolean;
  created_at: Timestamp;
  // Default tracks for titles without a remembered choice:
  audio_language: string | null;    // ISO 639 code ("eng", "fr"); null = the file's default track
  subtitle_language: string | null; // null = the audio language
  subtitle_mode: "off" | "forced" | "foreign" | "always";
  has_password: boolean;           // false: a managed profile, or an account without a password
  two_step: boolean;               // signs in with a code from an authenticator app after the password
}

type LibraryKind = "movies" | "shows" | "music";

interface Library {
  id: Id;
  name: string;
  kind: LibraryKind;
  paths: string[];              // empty for apps' tokens
  item_count: number;           // top-level items (movies / shows / artists) this user can see
  created_at: Timestamp;
  scanned_at: Timestamp | null;
  scanning: boolean;            // a scan is queued or running right now
}

type ItemKind = "movie" | "show" | "season" | "episode" | "artist" | "album" | "track";

interface UserState {
  position_ms: number;          // resume position, 0 if none
  watched: boolean;             // containers: true when every leaf is watched
  play_count: number;
  last_played_at?: Timestamp;
  rating?: number;                  // the user's own rating, 0-10 (two points per star)
}

// Optional fields are left out when they have no value, which keeps big
// listings small; treat a missing field like null.
interface ItemSummary {
  id: Id;
  library_id: Id;
  kind: ItemKind;
  title: string;
  sort_title: string;
  year?: number;
  index?: number;                   // season number / episode number / track number
  parent_id?: Id;                   // season for episodes, show for seasons, album for tracks, artist for albums
  parent_title?: string;
  parent_index?: number;            // season number for episodes, disc number for tracks
  grandparent_id?: Id;              // show for episodes, artist for tracks
  grandparent_title?: string;
  duration_ms?: number;
  rating?: number;                  // 0..10
  added_at: Timestamp;
  poster_url?: string;              // e.g. "/api/items/12/images/poster?v=1729600000" (append &w=<px>)
  backdrop_url?: string;
  thumb_url?: string;               // episode stills / video frame grabs (16:9)
  user_state: UserState;
  leaf_count?: number;              // show/season: #episodes, artist/album: #tracks
  viewed_leaf_count?: number;       // show/season: #watched episodes
  child_count?: number;             // show: #seasons, artist: #albums
  summary?: string;                 // in lists only for episodes, every kind's with `summaries=1` (below); always in ItemDetail
  release_date?: string;            // "YYYY-MM-DD"
  extra_type?: string;              // kind "extra": trailer | featurette | behind_the_scenes | deleted_scene | interview | scene | short | other
  media?: MediaSummary;             // only in library listings sorted by bitrate
}

interface MediaSummary {            // what a title's files are like: a movie's best file, a show's episodes together
  bitrate: number | null;           // bits per second (a show: its episodes' average)
  size: number | null;              // bytes (a show: all its episodes)
  width: number | null;
  height: number | null;            // a show: its largest episode
  video_codec: string | null;       // null for shows
  dynamic_range: "sdr" | "hdr10" | "hdr10plus" | "hlg" | "dolby_vision" | null; // null for shows
}

interface ItemDetail extends ItemSummary {
  original_title: string | null;
  tagline: string | null;
  genres: string[];
  studio: string | null;
  content_rating: string | null;    // e.g. "PG-13"
  tmdb_id: number | null;
  imdb_id: string | null;
  files: MediaFile[];               // empty for containers (show/season/artist/album)
  markers: Marker[];                // intro and credits of the files (Skip Intro / Skip Credits)
  extras: ItemSummary[];            // movies and shows: trailers and other bonus material (kind "extra"), trailers first
  cast: Credit[];                   // actors in billing order; episodes: guest stars; seasons: the show's
  crew: Credit[];                   // creators, directors, writers, producers
  in_watchlist: boolean;            // in the user's My List (movies and shows)
}

interface Credit {
  id: Id;                           // the person: GET /api/people/{id}
  name: string;
  role: "actor" | "creator" | "director" | "writer" | "producer";
  character?: string;               // actors: the part played
  photo_url?: string;               // "/api/people/7/photo?v=…" (append &w=<px>)
}

interface PersonSummary {
  id: Id;
  name: string;
  photo_url?: string;
}

interface PersonDetail extends PersonSummary {
  credits: {                        // newest first; episode credits count towards their show
    item: ItemSummary;              // a movie or show
    roles: { role: Credit["role"]; character?: string; episodes?: number }[];
  }[];
}

interface Marker {
  file_id: Id;                      // markers belong to a file: versions can be different cuts
  kind: "intro" | "credits";
  start_ms: number;
  end_ms: number;                   // credits that run to the end of the file end at its duration
  source: "chapter" | "fingerprint" | "frames";
}

interface MediaFile {
  id: Id;
  path: string;                     // apps' tokens: the file name only
  size: number;                     // bytes
  container: string | null;         // "mkv" | "mp4" | "webm" | "avi" | "mp3" | "flac" | ...
  duration_ms: number | null;
  bitrate: number | null;           // bits per second
  width: number | null;
  height: number | null;
  video_codec: string | null;       // ffprobe codec name: "h264", "hevc", "av1", ...
  audio_codec: string | null;       // codec of the default audio stream
  streams: MediaStream[];
  edition: string | null;           // from `{edition-Final Cut}` in the file's or its folder's name
  selected_audio: number | null;    // tracks to start with for this user (remembered
  selected_subtitle: number | null; // choice or their defaults); null = default / off. The audio
                                    // for an unknown player: a play request picks the track of
                                    // that language its player plays best (`audio_index` below)
  chapters: Chapter[];              // from the container, by start time (empty if none)
}

interface Chapter {
  start_ms: number;
  end_ms: number;
  title: string | null;
  thumb_url?: string;               // picture of the chapter, once the file has seek previews
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
  dynamic_range?: "sdr" | "hdr10" | "hdr10plus" | "hlg" | "dolby_vision"; // "hdr10plus": PQ with HDR10+ in its frames
  dv_profile?: number;              // Dolby Vision profile (5, 7, 8, …)
  dv_bl_compat?: number;            // DV base-layer compatibility id (1 = HDR10, 2 = SDR, 4 = HLG, 6 = Blu-ray HDR10
                                    // (profile 7), 0 = none)
  dv_el?: "fel" | "mel";            // profile 7's enhancement layer: full (picture data) or minimal (absent: not known)
  decode_order_times?: boolean;     // frames timed as they're decoded, not as they're shown: the video is converted
  dv_level?: number;                // Dolby Vision level
  hdr10plus?: boolean;              // Dolby Vision and PQ streams: HDR10+ in the first frames (absent: not looked for).
                                    // On Dolby Vision, true is Dolby Vision + HDR10+
  // How clients name the format, the same everywhere: "Dolby Vision P8.1", "Dolby Vision P8.4", "Dolby Vision P5",
  // "Dolby Vision P7 FEL" (or MEL; "P7" when not known: never "P7.6"), "Dolby Vision P8.1 + HDR10+", "HDR10+",
  // "HDR10", "HLG"; badges shorten "Dolby Vision" to "DV".
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
  key: string;                      // "continue_watching" | "my_list" | "recently_added:<library_id>" | ...
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
  url: string;                      // direct: file url; otherwise HLS playlist url (master.m3u8 for
                                    // version 2 players with hls.master_playlist, else index.m3u8)
  hls: boolean;
  file_id: Id;
  duration_ms: number;
  audio_index: number | null;
  subtitle_index: number | null;    // the subtitle that is burned in (only when burn_subtitles)
  burn_subtitles: boolean;
  audio_converted: boolean;         // the audio is converted (also in a remux, where the video is copied)
  subtitles: SubtitleTrack[];       // all subtitle tracks of the file
  reasons: string[];                // why it isn't played as it is, one short sentence each
  reason: string | null;            // the same in one line
  auto_quality: string | null;      // the original converted: the closest quality players offer ("30m" = 30 Mbps 4K,
                                    // "20m", "15m" = 15 Mbps 4K, "12m", "8m", "4m", "2m"); null when a quality was asked for
  video_range: string | null;       // what the screen gets of HDR video, for the player's details (null for SDR):
                                    // "Dolby Vision P8.1", "Dolby Vision P7 FEL", "HDR10+", "HLG",
                                    // "HDR10 (Dolby Vision base layer)", "HDR10+ (Dolby Vision base layer)",
                                    // "Dolby Vision P8.1 (converted from P7 FEL)", "Dolby Vision P8.1 (HDR10+ left out)",
                                    // "HDR10 (this screen doesn't show HDR10+)", "HDR10 (converted from HLG)",
                                    // "SDR (tone-mapped)". Version 1 players don't say which HDR formats their
                                    // screen shows: their HDR10+ is named as it is
}
```

Lists leave out the synopses of movies, shows, seasons and the rest: a big
library's would be several times larger. `summaries=1` in the query of
`GET /api/hubs/home`, `/api/libraries/{id}/items`, `/api/items/{id}/children`,
`/api/collections/{id}`, `/api/playlists/{id}` and `/api/search` brings every
item's `summary`, for a client that shows nothing more of a title than its
lists carry (Kodi's information page).

---

## Public endpoints

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/health` | – | `{ "status": "ok" }` |
| GET | `/api/server` | – | `{ name: string, version: string, commit: string, setup_required: boolean, setup_code_required: boolean }`; `commit` is the build's short hash, `"unknown"` when built without the repository. Setup takes the code from the server's log |
| GET | `/api/oauth/device/qr?code=` | – | where to type a TV's code, as a QR code (PNG): see [TVs](#tvs) |
| POST | `/api/setup` | `{ username, password, setup_code }` | `{ user: User, token? }` — creates the first (admin) account and signs in. `409` once a user exists, `403` without the right `setup_code` (5 tries per 15 minutes from each address, 1000 from all of them, then `429`) |
| POST | `/api/auth/login` | `{ username, password, code? }` | `{ user: User, token? }` (`401` on bad credentials). With two-step sign-in, `code` is a code from the authenticator app or a recovery code: without it, or with a wrong one, `401` with `code: "two_step"` |

## Auth

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/auth/logout` | – | `204`, cookie cleared. With an app's token, wherever it was sent, that app is signed out (its whole grant ends) |
| GET | `/api/auth/me` | – | `User`. With an app's token, `User & { scope: string }`: `scope` is what the app was allowed (`"library play"`), and `is_admin`, `all_libraries` and `max_age` are the app's rights (`is_admin` always false) |
| PUT | `/api/auth/password` | `{ current_password, new_password }` | `204`; signs the user out everywhere but this device, and every app using the account or allowed by it |
| GET | `/api/auth/profiles` | – | `User[]`: home profiles this user can switch to (empty for non-home users) |
| POST | `/api/auth/switch` | `{ user_id, pin?, password?, code? }` | `{ user, token? }`: this device becomes that profile (old session ends). A profile with a PIN needs it; after 100 wrong PINs in a row it stops working until it is set again. Without one, a profile that can see more than yours (an administrator, a higher age limit, libraries yours can't open) needs its password, and its `code` with two-step sign-in, unless you are an administrator; a managed one can't be switched to until it has a PIN. `429` after 5 wrong tries in 15 min. |

Login attempts are throttled the same way (5 failures per username from each address per 15 min, 50 from all addresses).
Disabled accounts get `403` at login and their sessions stop working.

### Two-step sign-in

Optional for each account with a password: after the password, a 6-digit
code from an authenticator app (TOTP: SHA-1, 30-second steps), or one of ten
recovery codes. Each code works once; wrong ones count like wrong passwords,
and after 100 in a row only a recovery code signs in. `pley reset-password`
turns it off.

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/me/two-step` | – | `{ enabled, since: Timestamp \| null, recovery_codes_left }` |
| POST | `/api/me/two-step/setup` | `{ password }` | `{ secret, uri, qr: { size, path }, expires_at }`: a new secret (base32), its `otpauth://` URI and a QR code of it (`path` draws its dark modules on a `size`-wide grid). `409` when it is on already |
| POST | `/api/me/two-step` | `{ code }` | `{ recovery_codes: string[] }`: a code from the new secret turns it on (within 30 minutes, 5 tries); the codes are shown once |
| DELETE | `/api/me/two-step` | `{ password, code }` | `204`, off |
| POST | `/api/me/two-step/recovery-codes` | `{ password }` | `{ recovery_codes }`: new ones; the old ones stop working |

## Your account

| Method | Path | Body | Response |
|---|---|---|---|
| PATCH | `/api/me` | `{ display_name?, avatar_color?, audio_language?, subtitle_language?, subtitle_mode? }` (`null` clears) | `User` |
| PUT | `/api/me/track-defaults` | `{ audio_language?, subtitle_language?, subtitle_mode? }`: all three at once; a language left out is `null`, a mode left out is `forced` | the profile as [`GET /api/auth/me`](#auth) shows it; `400` for a language that isn't 2 or 3 lower-case letters, or another mode. For apps allowed `play` too |
| PUT | `/api/me/pin` | `{ pin: string \| null, password }` | `204`; 4–8 digits, `null` removes it (`403` with a wrong password) |
| PUT | `/api/me/avatar` | raw image bytes (JPEG, PNG, WebP, GIF, BMP; ≤ 10 MB) | `User`; stored as a 256 px square JPEG |
| DELETE | `/api/me/avatar` | – | `204` |
| GET | `/api/users/{id}/avatar` | – | `image/jpeg` (any signed-in user, for the profile switcher; apps: their own profile's and their household's, else `404`) |
| GET | `/api/me/devices` | – | `Device[]`: `{ id, name, user_agent, ip, created_at, last_seen_at, current }` |
| DELETE | `/api/me/devices/{id}` | – | `204`, signs that device out |

Titles you haven't picked tracks for start with your defaults: the main
audio track in `audio_language` (the file's default without one), and
subtitles as `subtitle_mode` says: `off` never; `forced` (the default) only
forced subtitles in the language of the audio, which translate foreign
dialogue; `foreign` full subtitles in `subtitle_language` (or the audio
language) when the audio is tagged with another language; `always` in that
language whenever the file has them. Every mode but `off` falls back to the
forced ones. A choice made for a show or a movie wins over these.
Subtitles count as forced when flagged so, or when only their title says
so (the word "forced", or "signs" as in "Signs & Songs"), as many files
have them. Languages match in any of their ISO 639 forms (`en`, `eng`;
`fr`, `fre`, `fra`); an untagged or `und` track is in no language.

Apps change these with `PUT /api/me/track-defaults`, never with `PATCH
/api/me`, which also changes the name and picture: those stay in the web app.

## Apps (OAuth 2.1)

Apps sign in on pley's own page, and get tokens that open only what the
person allowed. pley is an OAuth 2.1 authorization server for them
(draft-ietf-oauth-v2-1-16), with PKCE and client metadata documents
(draft-ietf-oauth-client-id-metadata-document-02). It lets apps use pley: it
is no OpenID Connect provider and signs nobody in to other sites.

Every app is public: no client secrets, no registration. An app is known by
its client ID, the https address of a JSON document that describes it.
pley's own apps are built in; apps by other publishers sign in only when an
administrator allows them (see [below](#apps-by-other-publishers)).

### Discovery

`GET /.well-known/oauth-authorization-server` (RFC 8414) says where
everything is, for the address it was asked at (`Cache-Control: no-store`):

```json
{
  "issuer": "http://pley.local:7539",
  "authorization_endpoint": "http://pley.local:7539/oauth/authorize",
  "token_endpoint": "http://pley.local:7539/api/oauth/token",
  "revocation_endpoint": "http://pley.local:7539/api/oauth/revoke",
  "device_authorization_endpoint": "http://pley.local:7539/api/oauth/device",
  "response_types_supported": ["code"],
  "response_modes_supported": ["query"],
  "grant_types_supported": ["authorization_code", "refresh_token",
                            "urn:pley:params:oauth:grant-type:switch-profile",
                            "urn:ietf:params:oauth:grant-type:device_code"],
  "token_endpoint_auth_methods_supported": ["none"],
  "revocation_endpoint_auth_methods_supported": ["none"],
  "code_challenge_methods_supported": ["S256"],
  "scopes_supported": ["library", "play", "playlists", "profiles"],
  "authorization_response_iss_parameter_supported": true,
  "client_id_metadata_document_supported": false,
  "service_documentation": "https://github.com/hhaidar/pley/blob/master/docs/API.md"
}
```

The issuer is the address the request came to: `https` when the first
`X-Forwarded-Proto` value says so (else `http`), then the `Host` header,
with a lower-case host, no default port and no trailing slash. A server
reached at its home and its public address has a different issuer at each.
Apps check that the issuer is the address they used (RFC 8414 §3.3), so a
reverse proxy must pass the original `Host` (deploy/README.md). One that
leaves its port out gets it back for the addresses in the settings'
`proxy_addresses` (`https://media.example.com:9999`): a `Host` without a
port, for that scheme and host name, takes that port. A missing
`Host`, or one with a path or a user name, answers `400`.
`client_id_metadata_document_supported` is the setting for apps by other
publishers. Every other `/.well-known/` path answers `404`.

### Signing in

1. The app opens `/oauth/authorize`, a page of the web app, in the browser,
   with these query parameters, each at most once (one sent without a value
   is as if left out, OAuth 2.1 §3.1):

   | Parameter | |
   |---|---|
   | `response_type` | `code` |
   | `client_id` | the app's client ID |
   | `redirect_uri` | one of the app's return addresses, exactly as registered; a loopback one (`http://127.0.0.1/…`, `http://[::1]/…`) with any port |
   | `code_challenge` | `BASE64URL(SHA-256(code_verifier))`: 43 characters (PKCE, RFC 7636) |
   | `code_challenge_method` | `S256`, the only one |
   | `scope` | optional: scopes separated by spaces; none means `library` |
   | `state` | optional, at most 1,024 bytes; sent back as it came |
   | `resource` | optional: this server's issuer, with or without a trailing `/` (RFC 8707) |
   | `response_mode` | optional: `query` |

   Other parameters are ignored, given twice or not, but `request` and
   `request_uri` are refused.
2. The page signs the person in first when they aren't (two-step sign-in
   included), then shows the app, the profile it will act as (with "Use
   another profile"), what it will be able to do and where it goes back to.
   People are asked every time, for pley's own apps too, and allow all that
   was asked for or nothing. Allow works once the page has been seen for a
   second.
3. The browser goes back to the return address by a `303`, the address's
   own query kept, with `code`, `state` (when the app sent one) and `iss`
   (the issuer, RFC 9207) when the person allowed the app, or with
   `error=access_denied`, `state` and `iss` when they didn't.
4. The app exchanges the code at the [token endpoint](#tokens) within 60
   seconds.

An app pley can't accept (unknown, refused, or apps by other publishers
off) or a return address it didn't register is shown on the page and never
sent to the app: pley can't know the address is the app's. Other mistakes go
back to the app, with `error`, `error_description`, `state` and `iss`, but
only when the person clicks "Back to the app", on a page that says where
that goes (RFC 9700 §4.11.2):

| `error` | When |
|---|---|
| `unsupported_response_type` | `response_type` is something other than `code` |
| `invalid_request` | `response_type` is missing; `response_mode` isn't `query`; `request` or `request_uri` is given; the challenge is missing or not S256; `state` is longer than 1,024 bytes |
| `invalid_scope` | a scope pley doesn't offer (`openid` among them); `profiles` for an app that isn't pley's own; more than the app's description allows |
| `invalid_target` | `resource` names another server |

### Tokens

`POST /api/oauth/token`, `POST /api/oauth/revoke` and
[`POST /api/oauth/device`](#tvs) take a form
(`application/x-www-form-urlencoded`) of at most 16 KiB and never read
cookies. Each parameter pley reads comes at most once; one sent without a
value is as if left out, and others are ignored (OAuth 2.1 §3.2). They
answer with `Cache-Control: no-store` and `Pragma: no-cache`, and errors as
`{ "error": "invalid_grant", "error_description": "…" }` (RFC 6749 §5.2),
`400` unless said otherwise. A `client_secret` answers `400`
`invalid_client`, and an `Authorization: Basic` header `401`
`invalid_client` with `WWW-Authenticate: Basic realm="pley"`: pley's apps
have no secrets. `resource`, when sent, must be this server
(`invalid_target`). A missing `grant_type` is `invalid_request`; another one
is `unsupported_grant_type`.

Tokens look like this:

```json
{
  "access_token": "pley_at_…",
  "token_type": "Bearer",
  "expires_in": 3600,
  "scope": "library play",
  "refresh_token": "pley_rt_…"
}
```

`scope` is everything the app was allowed. `refresh_token` comes only for
apps whose description lists the `refresh_token` grant type (pley's own
apps do). pley keeps tokens as SHA-256 hashes; the prefixes make leaked ones
easy to recognise.

**`grant_type=authorization_code`**: `code`, `client_id` and
`code_verifier` (43 to 128 of `A-Z a-z 0-9 . _ ~ -`); `redirect_uri`, when
sent, must be the one the code was sent to. An unknown or expired code,
another app's, another return address or a wrong verifier is
`invalid_grant` and leaves the code as it was. A code exchanged a second
time with everything right is `invalid_grant` ("the code was used
already"), and the access the first exchange got ends: someone else has the
code. A code whose account was disabled in the meantime, or of an app by
another publisher once they were turned off, is `invalid_grant` too. So is a
code whose browser session has signed out since, or whose account's
sign-ins ended (a new password, "Sign out everywhere"): `invalid_grant`,
"the person who allowed it signed out since". Nothing allowed before
those signs an app in after them.

**`grant_type=refresh_token`**: `refresh_token` and `client_id`; `scope`
optional. Each refresh token works once: the answer has a new one. One used
again ends the app's access (`invalid_grant`, "refresh token used again: the
app's access is ended"), however many refreshes later, and so do two
refreshes with the same token at once: apps refresh one at a time. A
refresh answer lost on the network means signing in again. A wider `scope`
than allowed is `invalid_scope`; a narrower one is ignored. An unknown,
expired or other app's refresh token, a disabled account (the profile's or
the one that allowed it), or apps by other publishers turned off is
`invalid_grant`, as is a grant with 10,000 refresh tokens on record, which
ends. Refreshes never read an app's description again.

**`grant_type=urn:pley:params:oauth:grant-type:switch-profile`**: pley's
own apps, allowed `profiles`, move to another profile of the household:
`client_id`, `refresh_token`, `profile` (an id from
[`/api/oauth/profiles`](#profiles)) and `pin` when the profile has one. The
answer is new tokens acting as that profile; the app keeps what it was
allowed and who allowed it, and its old tokens stop at once. The rules are
the web app's [switch](#auth), from the rights of the profile the app acts
as, without the administrators' exemption. A refusal spends nothing.

The profile the app acts as already, with its `pin`, checks that PIN: a TV
asks "Who's watching?" as it starts. The rules are a switch's to that
profile (the same wrong-PIN answers, throttle and lock, counted with the
switches to it), and a right PIN answers as a switch does, with a new pair,
the same scope and the old tokens stopped. Without a `pin`, or for a
profile that has no PIN, it is `invalid_request` without a `reason`, as
servers before this check answered every switch to the current profile:
apps take that to mean the server can't check it.

| Answer | When |
|---|---|
| `unauthorized_client` | not one of pley's own apps, or not allowed `profiles` |
| `invalid_request` | `profile` is missing or not an id, or the app acts as that profile already and there is no PIN to check (none given, or none on the profile) |
| `invalid_grant`, `"reason": "wrong_pin"` | the profile has a PIN, and it is missing or wrong |
| `invalid_grant`, `"reason": "pin_locked"` | its PIN stopped working after 100 wrong ones in a row |
| `invalid_grant`, `"reason": "throttled"` | too many wrong PINs for it lately (5 in 15 minutes) |
| `invalid_grant`, `"reason": "browser"` | it can see more than the app's profile and has no PIN: sign in to the app again and pick it in the browser (apps never see passwords) |
| `invalid_grant`, `"reason": "not_allowed"` | not a profile of the household, disabled, or freer with neither PIN nor password |
| `invalid_grant` | the refresh token doesn't work, as for a refresh (a spent one ends the grant) |

**`POST /api/oauth/revoke`** (RFC 7009): `token` and `client_id`
(`invalid_request` without them; `token_type_hint` is ignored). An access or
refresh token of that app, current, spent or expired, ends its whole grant,
and the answer is `200` with an empty body, as it is for a token pley
doesn't know. Another app's token is refused with `400`
`unauthorized_client` and stays (§2.1). When the database fails, `503`
`temporarily_unavailable` with `Retry-After`: the token still works, and the
app tries again later (§2.2.1). `POST /api/auth/logout` with an app's token
ends its grant too.

There is no rate limit on the token and revocation endpoints: codes and
tokens carry 256 random bits, and a limit per address behind a reverse
proxy would lock every app out. TVs have [their own limits](#tvs).

### TVs

TVs and other devices without a usable browser sign in with a code they
show (OAuth 2.0 Device Authorization Grant, RFC 8628). pley's own apps only:
anyone may start one, and pley reads no app's description for someone who
isn't signed in. Today that is pley for Android, which also runs on Android
TV and Fire TV, pley for Kodi, pley for Roku, pley for Samsung TV and pley
for LG TV.

1. The TV asks **`POST /api/oauth/device`** with `client_id` and an optional
   `scope` (as at `/oauth/authorize`: none means `library`). The answer:

   ```json
   {
     "device_code": "…",
     "user_code": "BCDF-GHJK",
     "verification_uri": "http://pley.local:7539/link",
     "verification_uri_complete": "http://pley.local:7539/link?code=BCDFGHJK",
     "expires_in": 600,
     "interval": 5
   }
   ```

   The TV keeps `device_code` (256 random bits) to itself and shows
   `user_code` and `verification_uri`, or a QR code of
   `verification_uri_complete`. A TV that can't draw one shows
   **`GET /api/oauth/device/qr?code=BCDF-GHJK`**: that address as a PNG
   (black on white, 8 pixels a module, with a quiet zone of 4 modules;
   `Cache-Control: no-store`), for anyone who asks, since it looks no
   request up; `400` for something that isn't a code. User codes are 8 of
   RFC 8628 §6.1's 20 consonants (`BCDFGHJKLMNPQRSTVWXZ`, about 2.6 × 10¹⁰
   codes), without vowels or digits so they spell nothing and look like
   nothing else.
   `invalid_client` for an app that isn't pley's own, `unauthorized_client`
   for one of pley's own that doesn't sign in this way (the desktop app),
   `invalid_scope` as at `/oauth/authorize`, `429`
   `temporarily_unavailable` past the limits below.
2. The person opens **`/link`**, a page of the web app, on a phone or
   computer, signs in there if they aren't, and types the code (in any
   case, with or without the dash and spaces). From the QR code it comes
   filled in, and is sent only once they continue. The page shows the same
   consent as `/oauth/authorize`, saying the answer goes to the TV showing
   that code, what that device said it is and where it asked from (two TVs
   with the same app look alike otherwise), and to allow it only when the
   code is on a screen in front of them. Nothing goes back through the
   browser.
3. Meanwhile the TV asks the token endpoint every `interval` seconds with
   **`grant_type=urn:ietf:params:oauth:grant-type:device_code`**,
   `device_code` and `client_id`:

   | Answer | When |
   |---|---|
   | `authorization_pending` | nobody has answered yet |
   | `slow_down` | it asked sooner than its interval, which is now 5 seconds longer, for good |
   | tokens | the person allowed it: as for a code, acting as the profile they use; once |
   | `access_denied` | the person didn't allow it, their sign-ins ended before the TV had its tokens (a new password, "Sign out everywhere", disabling, deletion), or the browser session that allowed it signed out since; once |
   | `expired_token` | 10 minutes went by; once |
   | `invalid_grant` | an unknown device code, another app's, one whose answer was given already, or a disabled account |

A device code never appears in an address or a page, so one shown again
after its answer is only unknown: nothing has to end, unlike a code.

Limits: at most 20 sign-ins in progress from one address (an IPv6 /64
counts as one) and 1,000 in all, and nothing in progress is dropped to make
room. The address is the connection's, or the client's a reverse proxy on
the same computer or network names last in `X-Forwarded-For`, never one a
caller on the internet writes. Wrong codes count: past 10 by one person, or
300 by everyone, in 10 minutes, no code is looked up (`429`).

### Lifetimes and limits

| What | Lasts |
|---|---|
| Access token | 1 hour |
| Refresh tokens | rotated on every use; the app's access ends 90 days after its last token request |
| Apps by other publishers | asked about again a year after they were allowed |
| Code | 60 seconds, once |
| A request on the consent page | 10 minutes (a TV's: no longer than the TV's own) |
| A TV's code | 10 minutes, once |

An access token never outlasts its grant. A grant keeps its 10 newest access
tokens (older ones stop working) and ends with 10,000 refresh tokens on
record. A profile has at most 100 apps signed in; past that the least
recently used is signed out.

Apps are signed out when they or the person sign them out (the
[Apps](#signed-in-apps) lists), with a new password, "Sign out everywhere",
`pley reset-password` and when the account is deleted: every app acting as
the account or allowed by it, and the apps it was signing in (codes not
exchanged yet, TVs allowed that haven't asked for their answer yet). A
running server honors app tokens it checked in the last minute for up to a
minute after `pley reset-password`, as it does sessions. The apps of a
disabled account, or allowed by one, don't work while it is disabled.
Turning apps by other publishers off signs all of them out.

### Scopes

| Scope | Allows |
|---|---|
| `library` | see the profile's libraries, playlists and watch history |
| `play` | play titles, and keep the profile's place, watched marks, ratings, track choices and library order |
| `playlists` | make, change and delete the profile's playlists |
| `profiles` | switch to the household's other profiles; pley's own apps only, since a profile's PIN is typed into the app |

An app's token opens these routes and no other (`HEAD` as `GET`). A route
missing here answers `403` `insufficient_scope`, administration and
sign-in settings among them.

| Method | Path | Needs |
|---|---|---|
| GET | `/api/auth/me` | any app |
| GET | `/api/users/{id}/avatar` | any app |
| GET | `/api/libraries` | `library` |
| GET | `/api/libraries/{id}` | `library` |
| GET | `/api/libraries/{id}/items` | `library` |
| GET | `/api/libraries/{id}/filters` | `library` |
| GET | `/api/libraries/{id}/hubs` | `library` |
| GET | `/api/libraries/{id}/collections` | `library` |
| GET | `/api/collections/{id}` | `library` |
| GET | `/api/collections/{id}/images/{kind}` | `library` |
| GET | `/api/playlists` | `library` |
| GET | `/api/playlists/{id}` | `library` |
| GET | `/api/items/{id}` | `library` |
| GET | `/api/items/{id}/children` | `library` |
| GET | `/api/items/{id}/on-deck` | `library` |
| GET | `/api/items/{id}/next` | `library` |
| GET | `/api/items/{id}/related` | `library` |
| GET | `/api/items/{id}/images/{kind}` | `library` |
| GET | `/api/items/{id}/collections` | `library` |
| GET | `/api/people/{id}` | `library` |
| GET | `/api/people/{id}/photo` | `library` |
| GET | `/api/history` | `library` |
| GET | `/api/hubs/home` | `library` |
| GET | `/api/watchlist` | `library` |
| GET | `/api/search` | `library` |
| GET | `/api/events` | `library` |
| POST | `/api/playback` | `play` |
| GET | `/api/playback/{session}` | `play` |
| DELETE | `/api/playback/{session}` | `play` |
| GET | `/api/playback/{session}/{file}` | `play` |
| GET | `/api/files/{id}/stream` | `play` |
| POST | `/api/conversions` | `play` |
| GET | `/api/conversions/{id}` | `play` |
| DELETE | `/api/conversions/{id}` | `play` |
| GET | `/api/conversions/{id}/file` | `play` |
| GET | `/api/files/{id}/subtitles/{file}` | `play` |
| GET | `/api/files/{id}/trickplay` | `play` |
| GET | `/api/files/{id}/trickplay/{sheet}` | `play` |
| GET | `/api/files/{id}/chapters/{image}` | `play` |
| POST | `/api/items/{id}/prefetch` | `play` |
| POST | `/api/items/{id}/progress` | `play` |
| POST | `/api/items/{id}/watched` | `play` |
| POST | `/api/items/{id}/unwatched` | `play` |
| PUT | `/api/items/{id}/streams` | `play` |
| PUT | `/api/items/{id}/rating` | `play` |
| DELETE | `/api/history/{id}` | `play` |
| DELETE | `/api/continue-watching/{id}` | `play` |
| PUT | `/api/watchlist/{id}` | `play` |
| DELETE | `/api/watchlist/{id}` | `play` |
| DELETE | `/api/sessions/{id}` | `play` |
| PUT | `/api/me/track-defaults` | `play` |
| PUT | `/api/me/libraries/order` | `play` |
| DELETE | `/api/me/libraries/order` | `play` |
| POST | `/api/playlists` | `playlists` |
| PATCH | `/api/playlists/{id}` | `playlists` |
| DELETE | `/api/playlists/{id}` | `playlists` |
| POST | `/api/playlists/{id}/items` | `playlists` |
| DELETE | `/api/playlists/{id}/items/{entry_id}` | `playlists` |
| PUT | `/api/playlists/{id}/order` | `playlists` |
| GET | `/api/oauth/profiles` | `profiles` |

What stays in the web app: name, picture (`PATCH /api/me`), password, PIN,
two-step sign-in, devices, apps, activity, switching profiles by
`/api/auth/switch`, and all administration.

An app's token never has administrator rights. On an administrator's
account it sees every library without an age limit, as the account does, and
administers nothing: history, sessions and collections answer it as they
would anyone else. Apps also see less: libraries' `paths` are empty, files'
`path` is the file name only, `/api/users/{id}/avatar` shows only the
pictures of the app's own profile and its household, and when the
transcoder can't read a file, its words (which name the file) are replaced
by pley's own ("this title's file can't be read right now", and "the
conversion failed" for a session's `last_error`). No address pley hands out carries a token:
players send the header with every request, HLS playlists and segments
included. The event stream ends when its token expires or the app is signed
out; the app connects again with a new token.

### Signed-in apps

| Method | Path | Response |
|---|---|---|
| GET | `/api/me/apps` | `AppGrant[]`: apps acting as you or allowed by you, most recently used first |
| DELETE | `/api/me/apps/{id}` | `204`, signs that app out. `404` unless it acts as you or you allowed it |
| GET | `/api/users/{id}/apps` *(admin)* | `AppGrant[]` of that user |
| DELETE | `/api/users/{id}/apps/{grant}` *(admin)* | `204`; `404` as above |

These take pley's own sign-in only, never an app's token.

```ts
interface AppGrant {
  id: Id;
  client_id: string;
  name: string | null;              // pley's own apps: their name; others: what the app called itself (unchecked)
  first_party: boolean;             // one of pley's own apps
  host: string;                     // the client ID's host
  scopes: string[];
  profile: { id: Id; name: string };     // the profile it acts as
  approved_by: { id: Id; name: string };
  device: string | null;            // of its last token request, e.g. "Chrome on Windows"; pley for Android, Kodi, Roku, Samsung TV and LG TV: their platform, e.g. "Android 15", "Kodi 21.2; Windows", "Tizen 7.0", "webOS 23"
  ip: string | null;
  created_at: Timestamp;
  last_used_at: Timestamp;
}
```

### Profiles

`GET /api/oauth/profiles` (an app's token with `profiles`) lists the
enabled profiles of the household when the app's profile is in it, and
nothing otherwise:
`{ id, name, avatar_url, avatar_color, current, needs }[]`. `current` is the
profile the app acts as; `needs` is what switching to it takes, by the
rules above: `none`, `pin`, `password` (in the browser) or `locked`.
Usernames, roles, libraries, age limits and sign-in settings are left out.

### pley's own apps

| App | Client ID | Return address |
|---|---|---|
| pley for Android | `https://hhaidar.github.io/pley/oauth/android.json` | `app.pley.android:/oauth/callback` |
| pley for desktop | `https://hhaidar.github.io/pley/oauth/desktop.json` | `http://127.0.0.1/oauth/callback`, `http://[::1]/oauth/callback` (any port) |
| pley for Kodi | `https://hhaidar.github.io/pley/oauth/kodi.json` | none: a code only |
| pley for Roku | `https://hhaidar.github.io/pley/oauth/roku.json` | none: a code only |
| pley for Samsung TV | `https://hhaidar.github.io/pley/oauth/tizen.json` | none: a code only |
| pley for LG TV | `https://hhaidar.github.io/pley/oauth/webos.json` | none: a code only |

Their descriptions are built into the server and never fetched, so they
sign in to servers without internet access. All get refresh tokens and may
ask for every scope; pley for Android may also sign in with a code, on a TV
(see [TVs](#tvs)), and pley for Kodi, Roku, Samsung TV and LG TV sign in
only that way: with no return address, `/oauth/authorize` never sends them
a code. The Samsung and LG apps can't set their own User-Agent: the TV's
web engine names the TV ("Samsung TV", "LG TV", and for an app's grant its
system, "Tizen 7.0", "webOS 23").
The consent page marks them "pley's own app"; no other app may use a client
ID on their host.

### Apps by other publishers

Off until an administrator turns them on (**Settings → Server → Apps**,
`third_party_apps`). Then anyone signed in may allow one for their own
profile. Turning it off signs them all out; codes and refreshes check it
too.

The client ID must be an https address of at most 512 characters, written
the one way an address parser writes it (lower-case host, punycode, no
default port, no dot segments, no stray spaces), on a domain name with a dot
(not an IP address, `localhost` or `*.localhost`), with a path other than
`/`, and with no port, user name, query or fragment. It may not be on the
host of pley's own apps or on this server's.

The description must be a JSON object of at most 5,120 bytes, no field given
twice:

| Field | |
|---|---|
| `client_id` | the address it was read from, exactly |
| `redirect_uris` | 1 to 10 return addresses of at most 512 characters, each in its standard form and one of: https on exactly the client ID's host, without a port; a private-use scheme that is the client ID's host reversed (`apps.example.com` → `com.example.apps`), written `scheme:/path`; `http://127.0.0.1/…` or `http://[::1]/…`. Never `localhost`, a fragment or a user name |
| `client_name` | optional: shown without control and direction characters, at most 64 characters, as something pley can't check |
| `grant_types` | optional (`["authorization_code"]`): must list `authorization_code`; refresh tokens only with `refresh_token` |
| `response_types` | optional: must list `code` |
| `token_endpoint_auth_method` | optional: `none` |
| `scope` | optional: the most the app may ask for (`profiles` never) |

`client_secret` and `client_secret_expires_at` are refused. Other fields
(`logo_uri`, `client_uri`, `tos_uri`, `policy_uri`, `jwks`, `jwks_uri`, …)
are ignored and never fetched.

pley reads a description only when someone signed in opens the consent
page for the app: over https on port 443, from public addresses only (the
check is made by the resolver, so the address checked is the one connected
to), without a proxy, redirects, cookies or compression, with 3 seconds to
connect and 5 in all. The answer must be a `200` of JSON (`application/json`
or `application/*+json`) of at most 5,120 bytes. At most 4 descriptions are
read at once, one per client ID, and 10 a minute for each person (then
`429`). Valid descriptions are kept for their `Cache-Control: max-age`
between 5 minutes and 24 hours (an hour without one, not at all with
`no-store` or `no-cache`), at most 256; failures never are. Refreshes never
read them: changes apply at the app's next sign-in.

The consent page names such an app by its client ID's host, then what it
calls itself, and says it isn't one of pley's own. Logos aren't shown.

### The consent page's endpoints

pley's own page uses these; apps never do. They take the session cookie
only, never `Authorization`, `X-Pley-Token` or `?token=`. A `POST` needs an
`Origin` that is this server, and `Sec-Fetch-Site: same-origin` when the
browser sends it; the `GET` takes `Sec-Fetch-Site` `same-origin` or `none`.
Anything else is `403` ("this page must be opened from pley itself"). None
of them answers CORS.

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/oauth/requests` | `{ query }`: the page's query without `?`, at most 8 KiB | `{ status: "consent", id, expires_at, client: { id, host, name, first_party }, redirect: { kind: "web" \| "app" \| "loopback", to }, scopes }`; or a mistake that goes back to the app, `{ status: "error", error, message, back_url, client, redirect }`, `redirect` saying where "Back to the app" goes. `400` for an app or return address pley can't accept, `429` with too many descriptions read, `503` with too many sign-ins in progress |
| POST | `/api/oauth/device/verify` | `{ user_code }`: the code a TV shows, as typed | the consent as above, with `redirect: { kind: "device", to: "BCDF-GHJK", device, address }`: `device` is what the TV said it is when it asked for the code (pley's apps give their platform in their User-Agent's brackets, "Kodi 21.3; Windows"; anything else is named as the sign-ins list names it; unchecked, since anyone may ask), `address` where it asked from, each `null` when unknown. The page shows both, so the person can tell which device is signing in. `400` for something that can't be a code, `404` for a code no TV shows (it counts as a wrong one), `409` for one answered already or shown on another session's page, `429` past the wrong codes' limit, `503` with too many sign-ins in progress. A code shown on a page whose session ended since (signed out, or switched profile) goes to whoever types it next; typed again in the same session, the page before can't answer any more |
| POST | `/api/oauth/requests/{id}` | `{ allow: boolean }` | `{ continue_url }`, or `{ done: true }` for a TV's request, whose answer the TV gets when it next asks. The same answer again gets the same reply, the other one `409`; `404` when the request is over (a TV's too once the TV's code expired) |
| GET | `/api/oauth/requests/{id}/continue` | – | `303` to the app with the answer. It repeats while the code works and wasn't exchanged, and for a minute after a refusal or a mistake. After that a browser following the link is sent (`303`) to the web app's `/oauth/over` page, which says the request is over; a script gets `404`. Never for a TV's request |

A request belongs to the person and the browser session that opened the
page, and lasts 10 minutes. Each person has at most 10 waiting (the oldest
goes), the server 1,000. They end with the account's sign-ins (a new
password, "Sign out everywhere", disabling, deletion).

## Access rules

Administrators see everything. Other users see every library or only the
ones in `library_ids`, and with `max_age` only movies and shows whose content
rating allows that age (US, UK, DE, FR, AU, CA and numeric certifications are
understood; seasons and episodes go by their show; unrated titles are hidden;
music is exempt). Hidden libraries and items answer `404` everywhere: lists,
hubs, search, artwork, streams and the compatibility API.

## Libraries

Libraries are listed in one order, the same for every user: the one
administrators choose (new libraries go last). Lists of libraries, the home
page's rows and the compatibility API's sections all follow it; users who
may open only some libraries see those, in that order.

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/libraries` | query: `order=shared` | `Library[]` in the user's own order when they chose one (the ones it places first, any others after them in the shared order), else in the shared order; `order=shared`: in the shared order whatever the user chose (Settings → Libraries) |
| POST | `/api/libraries` *(admin)* | `{ name, kind, paths: string[] }` | `Library` (a scan is queued automatically) |
| PUT | `/api/libraries/order` *(admin)* | `{ ids: Id[] }` (every library, once) | `Library[]` in the new order: the shared order, which everyone who hasn't chosen their own sees. `400` when an id is listed twice, `409` when `ids` aren't the libraries there are (one was added or deleted since they were listed) |
| PUT | `/api/me/libraries/order` | `{ ids: Id[] }` (every library the user may open, once) | `Library[]` in the new order: the user's own, on every list of their libraries (Home's rows and the Plex-compatible API's sections too), whatever the shared order becomes. Libraries they are given later, and new ones, come after it. `400` when an id is listed twice, `409` when `ids` aren't the libraries the user may open. `libraries_changed` to the user's streams. For apps allowed `play` too |
| DELETE | `/api/me/libraries/order` | – | `Library[]` in the shared order, which the user sees again; `libraries_changed` to the user's streams |
| GET | `/api/libraries/{id}` | – | `Library` |
| PATCH | `/api/libraries/{id}` *(admin)* | `{ name?, paths? }` | `Library` |
| DELETE | `/api/libraries/{id}` *(admin)* | – | `204` |
| POST | `/api/libraries/{id}/scan` *(admin)* | `{ force?: boolean }` (force = re-probe files and refresh metadata) | `202` |
| POST | `/api/libraries/scan` *(admin)* | – | `202` (scan all) |
| GET | `/api/libraries/{id}/items` | query: `sort=title\|year\|added\|rating\|user_rating\|release\|duration\|last_played\|bitrate` (default `title`), `order=asc\|desc`, `watched=true\|false`, `genre=<name>`, `year=<yyyy>`, `offset`, `limit`, `summaries=1` (with synopses, see ItemSummary) | `{ total: number, items: ItemSummary[] }` — top-level items only; `total` counts every match, not just the page; `watched=true` matches shows whose every episode is watched. Items leave out `backdrop_url` and `thumb_url` (grids show posters). Answers carry an `ETag` and `Cache-Control: private, no-cache`; `If-None-Match` with the same tag answers `304` while the library, the user's watch state and their access are unchanged |
| GET | `/api/libraries/{id}/filters` | – | `{ genres: string[], years: number[] }` |
| GET | `/api/libraries/{id}/hubs` | – | `{ hubs: Hub[] }` (continue watching, recently added, recently released, top rated – empty hubs omitted) |

## Items

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/items/{id}` | – | `ItemDetail`. For a movie or episode, the server also reads its video files ahead from where *Play* starts for the user (as `POST …/prefetch` does), without waiting for it |
| POST | `/api/items/{id}/prefetch` | – | `202` at once. *Play* is about to be pressed (a card's Play button has the pointer or the focus): the transcoder reads ahead the parts of the video a playback from the user's resume point reads first (see `POST /v1/prefetch` in TRANSCODER.md), so they come from a cache when it starts; worth up to seconds for files on a network share. Movies, episodes and extras; a show or season: its on-deck episode; other kinds: nothing. At most once every two minutes per file. |
| GET | `/api/items/{id}/children` | – | `{ items: ItemSummary[] }` (seasons of a show, episodes of a season, albums of an artist, tracks of an album), ordered by index |
| GET | `/api/items/{id}/on-deck` | – | `{ item: ItemSummary \| null }` – for show/season: the episode to play when pressing *Play* (in progress, else next up as in [Continue Watching](#continue-watching), else the first unwatched, else the first) |
| GET | `/api/items/{id}/related` | `limit` (default 20, max 60) | `{ items: ItemSummary[] }` – "More Like This" for movies and shows: the same library's titles sharing collections, directors, creators and top-billed cast, genres and a period, best first (empty for other kinds) |
| GET | `/api/items/{id}/next` | – | `{ item: ItemSummary \| null }` – episode/track following this one (for autoplay) |
| POST | `/api/items/{id}/progress` | `{ position_ms, duration_ms?, state: "playing"\|"paused"\|"stopped", session_id?, age_ms? }` | `204`. ≥ 90 % marks the item watched and clears the resume point. `age_ms`: how long ago a report sent late was made (a download played while the server was out of reach, up to 90 days); it changes nothing when the item's state changed since (played elsewhere, marked watched). A report that changes what the user's devices show (watched, a resume point started, cleared, or moved by a pause or stop) sends `user_state_changed` (see "Activity & events") |
| POST | `/api/items/{id}/watched` | – | `204` (containers: all leaves); `user_state_changed` to the user's streams, unless nothing changed |
| PATCH | `/api/items/{id}` *(admin)* | `{ title?, sort_title?, original_title?, year?, summary?, tagline?, studio?, content_rating?, release_date?, genres? }` | `ItemDetail`. A value edits and locks the field (refreshes and scans keep it); `null` unlocks it. `locked_fields` in `ItemDetail` lists locked fields. |
| GET | `/api/items/{id}/matches?title=&year=` *(admin)* | – | TMDB candidates `{ tmdb_id, title, original_title, year, overview, poster_url }[]` for movies and shows (needs a TMDB key) |
| POST | `/api/items/{id}/match` *(admin)* | `{ tmdb_id }` | `ItemDetail` after re-matching and refreshing. A show whose TV source is TVmaze or TheTVDB is found there by the chosen entry's IMDb and TheTVDB ids. |
| GET | `/api/items/{id}/artwork/{poster\|backdrop\|thumb}` *(admin)* | – | TMDB images `{ url, preview_url, width, height, language, votes }[]` |
| PUT | `/api/items/{id}/artwork/{kind}` *(admin)* | `{ url }` (a TMDB image) | `ItemDetail`; the picture is downloaded and locked |
| POST | `/api/items/{id}/artwork/{kind}/upload` *(admin)* | raw image bytes (≤ 25 MB) | `ItemDetail`; stored as JPEG and locked |
| DELETE | `/api/items/{id}/artwork/{kind}` *(admin)* | – | `204`; back to local or downloaded artwork |
| PUT | `/api/items/{id}/rating` | `{ rating: number \| null }` (0-10, rounded to half stars; `null` clears it) | `204`; the user's own rating, shown as `user_state.rating` |
| PUT | `/api/items/{id}/streams` | `{ file_id?, audio_index?, subtitle_index }` (`subtitle_index: null` = off) | `204`. Remembers the choice for the whole show (episodes) or the movie, as language/title/forced, so every episode starts with its own matching tracks. `audio_index` left out keeps the audio chosen before (subtitles alone were picked); `null` remembers none, so each play chooses it as for a title without a choice. Players send it only when the person picked an audio track. |
| POST | `/api/items/{id}/unwatched` | – | `204`; `user_state_changed` to the user's streams, unless nothing changed |
| POST | `/api/items/{id}/refresh` *(admin)* | – | `202` re-fetch metadata from the item's source (movies: TMDB; shows, seasons, episodes: the TV source; artists, albums, tracks: MusicBrainz). `400` when that source is off or lacks its key. |
| POST | `/api/items/{id}/analyze` *(admin)* | – | `202` find the intro and credits again: a movie's chapters, or every episode of an episode's season, a season or a show. `400` for other kinds. |
| GET | `/api/items/{id}/images/{poster\|backdrop\|thumb}` | query: `w=<px>` (optional resize, bucketed), `v=` (cache buster, ignored) | image bytes (`image/jpeg`), long-lived cache headers. Falls back to parent artwork (season → show). `404` if none. |

## Home & search

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/api/hubs/home` | – | `{ hubs: Hub[] }` – `continue_watching` (in-progress + next up), then `my_list` ("My List", its 20 newest), then `recently_added:<library_id>` per library, in the libraries' order. Empty hubs are left out |
| DELETE | `/api/continue-watching/{id}` | – | `204`. Takes a row out of the user's Continue Watching (see below) |
| GET | `/api/search` | `q`, `limit` (default 40) | `{ items: ItemSummary[], people: PersonSummary[] }`: items ranked by relevance; people whose name has words starting with each word of `q`, most credited first |

### Continue Watching

Continue Watching holds the movies and episodes the user is part way
through, and for each show they've been watching lately its *next up*
episode, which is also what the show's *Play* starts (`on-deck`) when no
episode is in progress. Next up is the episode right after the one watched
most recently, when that one is unwatched. When it was seen before:

- **a rewatch goes on in order:** the episode watched most recently was a
  rewatch (seen twice or more), and either nothing after it is unwatched or
  the episode before it was rewatched too, after anything later was seen.
  Next up is then the episode right after it, seen or not, so going through
  a show again never jumps over episodes;
- **otherwise** (a gap filled, or one favourite seen again): the first
  unwatched episode after it, where the user had got to.

A seen episode stopped part of the way stays out of the in-progress rows
(apps play a seen episode from its start), and can still be next up.

`DELETE /api/continue-watching/{id}` takes what a Continue Watching row
shows out of the user's own Continue Watching: a movie, or for an episode
(in progress or next up) its whole show (a show's or a season's id does the
same). It stays out of every Continue Watching (Home's, the libraries' hubs,
the compatibility API's On Deck and `continueWatching` hubs) until the user plays
that movie or any episode of that show again: a progress report, or a
watched mark, newer than the hide. Then it comes back as it would have.
Progress and watched marks stay as they are. Other kinds answer `400`. The
compatibility API's `PUT /actions/removeFromContinueWatching?ratingKey=` does the
same. Either sends `user_state_changed` for the movie or show to the
user's event streams, so their other devices fetch Home again.

### My List

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/watchlist` | – | `{ items: ItemSummary[] }`, newest added first |
| PUT | `/api/watchlist/{id}` | – | `204`; adds a movie or a show (again: nothing changes). Other kinds: `400` |
| DELETE | `/api/watchlist/{id}` | – | `204`; takes it out (not there: nothing changes) |

Each user's own list. Titles the user can't see (library access, age
limit) are left out of it, and come back when they can again. A movie
leaves it once it is watched (played to the end, or marked watched here or
through the compatibility API); a show stays until it is removed. `ItemDetail` says
`in_watchlist`. Every change, a movie leaving by itself included, sends
`watchlist_changed` to the user's event streams (only theirs), so their
other devices fetch Home again.

## Cast and crew

Credits come with the rest of the metadata. From TMDB: movies' cast and
crew; shows' everyone in any episode and the creators; episodes' guest stars,
directors and writers. From TVmaze or TheTVDB: shows' main cast and creators.
A person is known by the id of the source that credited them, so the same
actor from two sources is two people. Items matched before an upgrade get
theirs fetched in the background once. People only appear where the user can
see an item they are credited in.

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/api/people/{id}` | – | `PersonDetail`; `404` when the person has no credits this user can see |
| GET | `/api/people/{id}/photo` | `w=<px>`, `v=` | image bytes; downloaded from the source (TMDB, TVmaze, TheTVDB) the first time. `404` without a photo |

## Collections

Groups of movies or shows of one library, shared by every user. *Automatic*
collections mirror TMDB's movie collections: the metadata agent files each
matched movie under its `belongs_to_collection` (with that collection's
poster and backdrop) and drops collections left empty; one is listed once two
of its movies are visible to the user. Administrators make their own and can
edit either kind; an edited title is kept over TMDB's. The [access
rules](#access-rules) apply: hidden items are left out, and a collection with
nothing left to show answers `404` (empty manual collections are shown to
administrators only).

```ts
interface Collection {
  id: Id;
  library_id: Id;
  title: string;
  summary: string | null;
  automatic: boolean;               // from TMDB
  sort: "release" | "title" | "custom";
  smart_filter: string | null;      // smart collections: the library filter that fills them
  item_count: number;               // items this user can see
  poster_url: string;               // own artwork, else the first item's that has some
  backdrop_url: string;
  created_at: Timestamp;
  updated_at: Timestamp;
}
```

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/libraries/{id}/collections` | – | `{ collections: Collection[] }` by title |
| POST | `/api/libraries/{id}/collections` *(admin)* | `{ title, summary?, item_ids?: Id[], smart_filter? }` | `201` `Collection & { items: ItemSummary[] }`; `400` for music libraries |
| GET | `/api/collections/{id}` | – | `Collection & { items: ItemSummary[] }` in the collection's order |
| PATCH | `/api/collections/{id}` *(admin)* | `{ title?, summary? (null clears), sort?, smart_filter? }` | `Collection & { items }` |
| DELETE | `/api/collections/{id}` *(admin)* | – | `204` (an automatic one comes back when its movies are refreshed) |
| POST | `/api/collections/{id}/items` *(admin)* | `{ item_ids: Id[] }` | `{ added: number }`; only movies and shows of the same library, `400` if none could be added |
| DELETE | `/api/collections/{id}/items/{item_id}` *(admin)* | – | `204` |
| PUT | `/api/collections/{id}/order` *(admin)* | `{ item_ids: Id[] }` | `204`; these first in this order, the rest after; switches `sort` to `custom` |
| GET | `/api/collections/{id}/images/{poster\|backdrop}` | `w`, `v` as for items | image bytes, or a `307` to the item artwork it falls back to |
| GET | `/api/items/{id}/collections` | – | `{ collections: { id, title }[] }` the movie or show is in ("Part of …") |

Smart collections hold no items: `smart_filter` is a library listing in the
compatibility API's syntax (the query string of
`/library/sections/{id}/all`, such as `genre=Comedy&unwatched=1&sort=year:desc`;
see docs/COMPATIBILITY.md), and the collection shows what it finds, in its order, as
each user may see it. Their items can't be added, removed or reordered
(`400`); the filter is changed instead.

## Playlists

Ordered lists of a user's own, private to them (administrators included).
A playlist is for `video` (movies and episodes) or `audio` (tracks); adding a
show, season, album or artist adds its episodes or tracks in playing order,
and the same item can be in a playlist more than once. Entries the user can
no longer see are skipped in every response but kept, so they come back if
access is restored. At most 5 000 entries.

```ts
interface Playlist {
  id: Id;
  title: string;
  kind: "video" | "audio";
  item_count: number;               // visible entries
  duration_ms: number;
  artwork: string[];                // artwork urls of the first entries (≤ 4), for a mosaic
  smart_filter: string | null;      // smart playlists: the listing of library_id they play
  library_id: Id | null;
  created_at: Timestamp;
  updated_at: Timestamp;
}

interface PlaylistEntry {
  entry_id: Id;                     // identifies this entry (an item may appear twice)
  item: ItemSummary;
}
```

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/playlists` | – | `{ playlists: Playlist[] }` by title |
| POST | `/api/playlists` | `{ title, kind?, item_ids?: Id[] }`, or `{ title, library_id, smart_filter }` | `201` `Playlist & { entries: PlaylistEntry[] }`; without `kind`, the first visible item decides |
| GET | `/api/playlists/{id}` | – | `Playlist & { entries }` |
| PATCH | `/api/playlists/{id}` | `{ title?, smart_filter? }` | `Playlist & { entries }` |
| DELETE | `/api/playlists/{id}` | – | `204` |
| POST | `/api/playlists/{id}/items` | `{ item_ids: Id[] }` | `{ added: number }`; `400` when an item doesn't fit the playlist's kind |
| DELETE | `/api/playlists/{id}/items/{entry_id}` | – | `204` |
| PUT | `/api/playlists/{id}/order` | `{ entry_ids: Id[] }` | `Playlist & { entries }`; these first in this order, the rest after |

A smart playlist plays what a listing of its library finds (`smart_filter`,
the same syntax as smart collections'), shows and albums as their episodes
and tracks, in the filter's order, at most 5,000 entries, as its owner may
see them. Its entries are keyed by item and can't be added, removed or
reordered (`400`).

Another user's playlist answers `404`. The web player plays a video playlist
in order (its *Next* is the next entry); music playlists go to the audio
queue.

## Playback

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/playback` | see below | `PlaybackInfo` |
| DELETE | `/api/playback/{session_id}` | – | `204` (stops any transcode) |
| GET | `/api/playback/{session_id}/index.m3u8` | – | HLS VOD playlist (`application/vnd.apple.mpegurl`) |
| GET | `/api/playback/{session_id}/master.m3u8` | – | multivariant playlist naming the codecs (see [Dolby Vision remuxes](#dolby-vision-remuxes)), one variant: `index.m3u8` |
| GET | `/api/playback/{session_id}/{n}.ts` | – | MPEG-TS segment |
| GET | `/api/files/{id}/stream` | – | raw file, supports `Range` (direct play) |
| GET | `/api/media/{key}/{file}` | – | a keyed session's media, for a player that can't send the Authorization header: `master.m3u8`, `index.m3u8` and `{n}.ts` of a stream, `file.{ext}` of a direct play (see [Keyed URLs](#keyed-urls)). No token |
| GET | `/api/files/{id}/subtitles/{index}.vtt` | query: `offset_ms` | WebVTT (text subtitles; extracted/converted on demand and cached). `offset_ms` moves every cue that much later (earlier when negative, never before 0; at most ±600000), for players that can't move a track themselves (Roku's subtitle sync); `400` when it isn't a whole number. Any embedded track may come partial: one still being read out of its file comes with the cues found so far and `X-Pley-Complete: 0` (not cached), so check that header and ask again for the rest. A Matroska file's track usually comes whole at once, read through its Cues when they list it. Requests for a subtitle file being converted share that conversion, which goes on (to the cache) when they stop waiting; one that failed is tried again on the next request |
| GET | `/api/files/{id}/subtitles/{index}.mks` | – | a picture subtitle track (PGS, VobSub with its palette, DVB; in the file or a file next to it) copied as it is into a Matroska file of its own (`video/x-matroska`), for players that show it next to an HLS stream instead of having it drawn onto the video (`picture_subtitles_sideload`). Its times are the original's from the start of the file, so it lines up with the playback position (an HLS stream started at a seek point included). Made once per file version by the transcoder (which reads the whole file for it), cached, then served with `Range` support. `404` for a text track or anything but subtitles; `503` with a transcoder too old to make it |
| GET | `/api/files/{id}/trickplay` | – | `{ id, interval_ms, width, height, columns, rows, count, sheets }` seek-bar previews; `404` while they are being made (asking, or opening the item, queues them). They are made one file at a time, never during a playback's first 30 s: a playback that starts puts off the file being done, which is made again later |
| GET | `/api/files/{id}/trickplay/{n}.jpg?v={id}` | – | sprite sheet `n`: thumbnail `k` is at column `k % columns`, row `k / columns` of sheet `k / (columns × rows)` and shows the video at `k × interval_ms` |
| GET | `/api/files/{id}/chapters/{n}.jpg` | – | picture of chapter `n` (0-based), cut from the seek previews (a few seconds into the chapter); `404` until the file has previews |

`POST /api/playback` body:

```ts
{
  item_id: Id;
  file_id?: Id;                   // default: first file of the item
  audio_index?: number | null;    // ffprobe stream index; left out: the server picks (below). Players name one only when the person picked it
  subtitle_index?: number | null; // only needed for image-based subs (forces burn-in transcode)
  max_bitrate?: number | null;    // kbps, null/absent = original quality
  max_height?: number | null;     // e.g. 1080, 720, 480
  start_ms?: number;              // resume position; transcodes start producing there
  mode?: "auto" | "direct" | "remux" | "transcode"; // default "auto"
  client_id?: string;             // identifies the player (browser tab, device)
  keyed_urls?: boolean;           // the player can't send the Authorization header: its addresses carry a key (Keyed URLs)
  caps: {                         // what the client can decode natively
    containers: string[];         // e.g. ["mp4","m4v","mov","webm","mp3","m4a","flac","ogg","wav"]
    video_codecs: string[];       // e.g. ["h264","hevc","vp9","av1"]
    audio_codecs: string[];       // e.g. ["aac","mp3","opus","flac","vorbis"]
    hdr?: boolean;                // shows HDR10 / HLG without tone mapping
    dolby_vision?: boolean;       // decodes Dolby Vision (needed for profile 5)
    // Version 2 (the native Android app); every field optional. Codec
    // names are ffprobe's.
    caps_version?: 2;
    video?: {                     // every video decoder; a stream plays when one entry takes it
      codec: string;              // "h264", "hevc", "vp9", "av1"…
      hardware?: boolean;
      profiles?: string[];        // as ffprobe names them: "Main", "Main 10", "High", "Profile 2"…
      max_width?: number;         // limits left out aren't checked
      max_height?: number;
      max_fps?: number;
      max_bit_depth?: number;
      max_level?: number;         // numbered as a stream's `level` (HEVC 5.1 = 153, H.264 5.1 = 51)
    }[];
    hdr_types?: ("hdr10" | "hdr10plus" | "hlg" | "dolby_vision")[]; // what the screen shows, a decoder feeding it
    dolby_vision_profiles?: { profile: number; max_level?: number }[]; // only with a Dolby Vision screen
    audio_decode?: string[];      // decoded on the device
    audio_passthrough?: string[]; // sent as they are to a receiver: "ac3", "eac3", "truehd", "dts"
    audio_channels?: number;      // the most channels the output takes: 2, 6, 8
    subtitle_codecs?: string[];   // shown by the player itself, pictures included ("hdmv_pgs_subtitle")
    picture_subtitles_sideload?: boolean; // shows picture subtitles from their `.mks` next to HLS too
    audio_track_switching?: boolean; // switches a file's audio tracks itself
    hls?: { master_playlist?: boolean; segments?: string[] }; // "ts", "fmp4"
    display?: { width: number; height: number; refresh_rates?: number[] };
    device?: { form?: "phone" | "tablet" | "tv" | "unknown"; tier?: "low" | "mid" | "high";
               fire_tv?: boolean; model?: string };
  };
}
```

**The audio track.** Without `audio_index`, the server picks, and
`PlaybackInfo.audio_index` says which track plays (a player that switches
tracks itself plays that one in direct play). The language: the one the
profile chose for the show or movie (a track it picked), else its audio
language, else the language of the file's default track. Of that
language's tracks, the main sound (never a commentary or an audio
description, unless picked), then the one this player plays best:

1. as it is (decoded, or passed through), rather than converted;
2. a picked track's title (a stereo mix picked on purpose stays);
3. with the file as it is: the file's default, or any track for a
   player that switches tracks itself (`audio_track_switching`); other
   tracks need a stream;
4. as many channels as `audio_channels` says the output takes, mixed for
   no more than that (a stereo mix beats a 5.1 one folded down on
   stereo), lossless (TrueHD, DTS-HD MA, FLAC, PCM), Atmos or DTS:X when
   passed through, the bitrate;
5. the file's default, then the first.

Tracks whose language is undetermined (`und`) or missing are never
swapped for one another. A TV whose receiver takes TrueHD gets the TrueHD
Atmos track; with a 5.1 soundbar, the Dolby Digital track as it is rather
than the TrueHD converted; a phone, a stereo mix; Chrome, an AAC track
copied into a stream. Item details (`selected_audio`) don't know the
player, so they give the language's default track.

Decision (`mode: "auto"`): **direct** when container + codecs are all supported,
the dynamic range can be shown, the requested audio track is the default one,
nothing must be burned in and the bitrate fits; otherwise **remux** (HLS, video
stream copied) or **transcode** (HLS, H.264/AAC).

- HDR10 and HLG need `caps.hdr`. Dolby Vision profile 5 needs
  `caps.dolby_vision`; profiles 7 and 8 play directly with either, because their
  base layer is HDR10 (8.1) or HLG (8.4). Otherwise the video is tone-mapped to
  SDR. Profile 5 plays only as it is: version 1 remuxes never name Dolby
  Vision, and unnamed it shows the wrong colours, so a profile 5 file that
  needs repackaging (Safari and Matroska) is converted instead. (This is version 1. Media3, which the Android app plays with, has no
  fallback for profile 7: version 2 below decides it differently.)
- Converted audio is stereo AAC; AAC and MP3 the player decodes are copied.
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
playing (with `session_id`), on pause and every ~30 s while paused, and with
`state: "stopped"` when closing, then `DELETE /api/playback/{session_id}`. A
session not heard from for 90 s has lost its player: it leaves "Now playing"
and its conversion stops (segments then answer `404`, "playback session
expired"); after 15 minutes it is forgotten. A downloaded file played as it is
reports without a session; reports made while the server was out of reach are
sent once it is back, each with its `age_ms`.

### Keyed URLs

An app's token goes only in the Authorization header, and some players
can't send headers with their media requests: Samsung's AVPlay sets only a
cookie and a User-Agent, and LG's media pipeline only what the web engine
gives it. A play request with `keyed_urls: true` gets a session whose
addresses carry a key of their own instead:

- `PlaybackInfo.url` is `/api/media/{key}/master.m3u8` (or `index.m3u8`)
  for a stream, whose playlists name their segments relatively, so they
  carry the key too; and `/api/media/{key}/file.mkv` for a direct play,
  named for the file's container, with `Range` and `Cache-Control:
  private, no-store`.
- The key is `pley_mk_` and 256 random bits, kept in memory only. It opens
  that session's playlists and segments, or its file, and nothing else:
  no API route takes it as a token.
- It ends with the session (closed by its player, or forgotten after 15
  quiet minutes: `404` "playback session expired", as segments say), with
  the account (disabled) and with the app's sign-in (signed out, a new
  password): `404`. A session stopped from elsewhere answers `410
  session_stopped` here too, with its message.
- Logs leave the key out (`/api/media/…/12.ts`).

Everything else the player asks for (subtitles, seek previews, its
reports) goes with the token as usual. pley's Samsung and LG TV apps ask
for keyed URLs.

### Version 2

`caps_version: 2` turns on these rules; a report without it (the web app,
the Android app's web view, compatibility API clients) decides as above, whatever else it
sends. Version 2 reports still send the version 1 fields: `containers`
always counts, and `video_codecs` or `audio_codecs` stand in only when
`video`, or both `audio_decode` and `audio_passthrough`, are left out.

- **Video.** Direct only when an entry of `video` takes the stream: its
  codec; its profile, when the entry lists profiles (10-bit HEVC needs
  `Main 10`); its size (either way round), frame rate (0.01 fps of slack),
  bit depth and level. A codec may have several entries (4K at 30 fps and
  1080p at 60): any one will do. `max_bitrate` and `max_height` in the
  request are only the quality the person picked.
- **Dynamic range.** HDR10 needs `hdr10` in `hdr_types`; HDR10+ needs
  `hdr10plus` or `hdr10`; HLG needs `hlg`. Without them the video is
  converted: to the HDR format the screen shows when the player takes it
  (see [HDR transcodes](#hdr-transcodes)), else tone-mapped to SDR, as for
  an SDR screen. The reason says which ("HLG is converted to HDR10 for this
  screen.").
- **Dolby Vision** plays as it is when `hdr_types` has `dolby_vision` and
  `dolby_vision_profiles` lists the profile at or above its level.
  Otherwise:
  - 8.1 and 8.4 (and 9, and 10 on an AV1 entry) play their HDR10, HLG or
    SDR base layer when the screen shows it, by the rules above;
  - 7 is **repackaged** when the screen shows HDR10: the video is copied
    into HLS and plays as its HDR10 base layer (Media3 has no profile 7
    fallback), or is converted to 8.1 for a Dolby Vision player that reads
    master playlists (below). Converted otherwise;
  - 5, which has no base layer, is converted (to HDR10 or HLG where the
    screen shows it, else to SDR), never repackaged without a master
    playlist: MPEG-TS segments lose the Dolby Vision signalling, which
    leaves its colours wrong.

  Players that read master playlists keep Dolby Vision in remuxes, and
  Dolby Vision + HDR10+ streams change: see
  [Dolby Vision remuxes](#dolby-vision-remuxes).
- **Audio** plays as it is when its codec is in `audio_decode` or
  `audio_passthrough`. With `audio_track_switching`, another track than the
  file's default no longer needs HLS: the player switches to it itself.
- **Picture subtitles** (PGS, VobSub, DVB) in `subtitle_codecs` are shown by
  the player when the file plays as it is; when it doesn't, they are drawn
  onto the video as for version 1 (HLS segments carry no subtitles).
  Subtitle files next to the media are always drawn. A player that says
  `picture_subtitles_sideload` shows them next to the HLS stream from
  `/api/files/{id}/subtitles/{index}.mks` instead: it asks for no
  `subtitle_index`, so nothing is drawn onto the video (and HDR can stay
  HDR). A `subtitle_index` it does send is drawn as for any player.
- **Converting.** Video is converted to H.264, no bigger than the player's
  largest H.264 entry (a 4K file is made in 1080p for a 1080p stick, also
  when a higher quality is picked); HDR video the player can show stays
  HDR, in HEVC Main 10 ([HDR transcodes](#hdr-transcodes)). Audio in AAC,
  MP3, AC-3 or E-AC-3 that the player takes is copied; anything else is
  converted to:
  - E-AC-3 5.1 at 640 kbit/s when `audio_channels` is 6 or more, the source
    has more than two channels, and `eac3` is decoded or passed through;
  - else AC-3 5.1 at 640 kbit/s when `ac3` is;
  - else AAC with as many channels as the output takes, up to 5.1 (stereo
    for stereo outputs; a stereo source stays stereo).

### HDR transcodes

HDR video that a version 2 player has converted (a lower quality picked,
a stream its decoders don't take, Dolby Vision profile 5 or HLG its
screen doesn't show) stays HDR when:

- an entry of `video` is HEVC, `hardware`, lists `Main 10` (or no profiles)
  with a `max_bit_depth` of 10 or more (or none), and takes the video's
  frame rate;
- the screen shows HDR10 (`hdr10` or `hdr10plus` in `hdr_types`) or HLG
  (`hlg`);
- the server's transcoder can (`hdr_transcode` in its capabilities: a
  10-bit HEVC encoder works);
- no picture subtitles are drawn onto the video.

The video is then HEVC Main 10, no bigger than the largest such decoder
takes:

| Source | Screen shows HDR10 | Screen shows HLG only | Screen shows both |
|---|---|---|---|
| HDR10, HDR10+ | HDR10, as it is | HLG (libplacebo) | HDR10 |
| Dolby Vision 7, 8.1 | HDR10: the base layer | HLG (libplacebo) | HDR10 |
| Dolby Vision 5 | HDR10 (libplacebo applies the RPU) | HLG (libplacebo) | HDR10 |
| HLG, Dolby Vision 8.4 | HDR10 (libplacebo) | HLG, as it is | HLG |

The HDR10 static metadata of the source (mastering display, MaxCLL and
MaxFALL) is kept; Dolby Vision's RPUs and HDR10+ are not (converted video
never carries Dolby Vision). Video converted from HLG or Dolby Vision
profile 5 has no static metadata unless the source had some. Everything
else (no such decoder, an SDR screen, an older transcoder, picture
subtitles, a transcoder without libplacebo for a conversion) is tone-mapped
to H.264 SDR as before; version 1 players always are. The master
playlist names it `hvc1.2.4.LLL.B0` with `VIDEO-RANGE=PQ` or `HLG`, and the
player's details say `video_out: "HEVC 3840×2160 HDR10"`.

### Dolby Vision remuxes

MPEG-TS segments carry a Dolby Vision stream's metadata (its RPUs) but
can't say that they do: Media3's `TsExtractor` sees plain HEVC, which plays
as its base layer at best and with wrong colours for profile 5. A
multivariant playlist can say it. A version 2 player sending
`hls: { master_playlist: true }` gets `PlaybackInfo.url` pointing to
`master.m3u8` for every HLS session, remux or transcode, with one variant:

```
#EXTM3U
#EXT-X-VERSION:3
#EXT-X-INDEPENDENT-SEGMENTS
#EXT-X-STREAM-INF:BANDWIDTH=85293600,AVERAGE-BANDWIDTH=43050000,CODECS="hvc1.2.4.L153.B0,ec-3",SUPPLEMENTAL-CODECS="dvh1.08.06/db1p",RESOLUTION=3840x2160,FRAME-RATE=23.976,VIDEO-RANGE=PQ
index.m3u8
```

- `CODECS` per RFC 6381: `avc1.PPCCLL`, `hvc1.P.C.LLLL.B0`, `mp4a.40.2`
  (AAC), `mp4a.40.34` (MP3), `ac-3`, `ec-3`. Dolby Vision kept in the
  remux is named: profile 5 as `dvh1.05.LL` alone (no base layer anything
  else decodes), profile 8 as the HEVC codec with
  `SUPPLEMENTAL-CODECS="dvh1.08.LL/db1p"` (8.1, `VIDEO-RANGE=PQ`), `/db4h`
  (8.4, `HLG`) or `/db2g` (8.2, `SDR`), which Media3 turns into a
  `video/dolby-vision` track; profile 7 as `dvh1.07.LL`.
- `VIDEO-RANGE` is `SDR`, `PQ` or `HLG`; `FRAME-RATE` has three decimals;
  `BANDWIDTH` is twice the copied video's average (discs peak about there),
  or the encoder's rate. TS segments are the same as `index.m3u8`'s.
- Without `master_playlist` (and for every version 1 player, the web
  player and compatibility API clients) the session starts at `index.m3u8` as before,
  and remuxes name nothing.

For those players, the Dolby Vision rules of version 2 become:

- A **listed profile** (with a Dolby Vision screen, at or above the
  stream's level) still plays as it is; when the file needs repackaging
  anyway (the container, the audio, another track), the video is copied
  with Dolby Vision kept and named, **profile 5 included** (before, it was
  tone-mapped).
- **Profile 7** for a player that lists 8 but not 7 (every Fire TV, Google
  TV devices) is **converted to 8.1** while repackaged: the enhancement
  layer is left out and each RPU rewritten (no residual, no enhancement
  layer resampling, P8.1 colour coefficients; a full enhancement layer's
  curves become no-ops), as `dovi_tool`'s mode 2 does. The reason says
  "Dolby Vision P7 is converted to profile 8.1 for this player.", with
  ", without its enhancement layer" for a full one (`dv_el: "fel"`, whose
  12-bit detail is lost). Profile 8 must take the stream's level; otherwise
  the HDR10 base layer, as before.
- **Dolby Vision + HDR10+** (`hdr10plus: true` on the stream, found when the
  file is analysed) never plays as it is for a player that would decode it
  as Dolby Vision, on any model: Dolby Vision decoders show black or freeze
  with HDR10+ in the stream (androidx/media#1895). It is repackaged with the
  HDR10+ messages taken out and Dolby Vision kept ("…plays as Dolby Vision
  without its HDR10+…"); without a master playlist, as its base layer with
  HDR10+ ("…plays as its HDR10+ base layer, repackaged…"). Players without Dolby
  Vision play these files as before. Files analysed by older servers don't
  say: they play as before until a forced scan of their library analyses
  them again.
- A server whose transcoder predates master playlists (its capabilities
  lack `hls_master_playlist`) decides as if no player read them.

The player's details (`GET /api/playback/{session}`) say what became of
it: `transcode.dolby_vision` (`"kept"`, `"converted to 8.1"`) and
`transcode.hdr10plus_stripped`.

## Downloads

A device that keeps a title to play away from the server downloads its
original (`GET /api/files/{id}/stream`, with ranges) when it plays it as it
is, else a file the server makes for it: the streams a playback would
stream, in one Matroska file, or MP4 for a player whose `caps.containers`
name `mp4` but not `mkv` (QuickTime).

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/conversions` | `POST /api/playback`'s body (`item_id`, `file_id?`, `caps`, `audio_index?`, `max_height?`, `max_bitrate?`, `mode?`) | `DownloadInfo` |
| GET | `/api/conversions/{id}` | – | `DownloadInfo` |
| DELETE | `/api/conversions/{id}` | – | `204`: stopped, unless done |
| GET | `/api/conversions/{id}/file` | – | the converted file (`video/x-matroska`, or `video/mp4`) once whole, with `Range` |

```ts
interface DownloadInfo {
  id: string | null;         // the conversion; null for the original
  state: "direct" | "queued" | "running" | "done" | "failed";
  progress: number;          // 0 to 1 (1 for the original)
  size: number | null;       // bytes: the original's, or the converted file's once done
  error: string | null;      // why it failed
  url: string;               // the original, or the converted file
}
```

The server decides as for a playback with the same body: what the player
plays as it is downloads as it is (`direct`); otherwise the transcoder makes
the file (the video copied or made anew at `max_height` and `max_bitrate`,
the sound made AAC), one conversion at a time, from the start and at full
speed. Video made anew is no bigger than a version 2 player's screen
(`display`: 1080p for a phone, also of a 4K film) and never has more than
1.5 times the original's bits, which a playback's quality for a file with
few bits would. Asked again for the same file and choices, from any
device, the same conversion answers (or the file it made); a failed one
is tried again. Look at `GET /api/conversions/{id}` every few seconds
until it is `done`, then fetch `url`. Converted files are kept a week
after their last download (a download cut short goes on with `Range`),
within the transcoder's limit; once gone, asking again makes them again.
A conversion is known to the server once asked for since it started (a
`404` otherwise: ask again), and to those who may see its title. `503`
while the transcoder isn't running or the file can't be read (ask again
later); `501` with a transcoder too old to convert, or to make MP4. Video is copied into
a download's file only when the player plays it as it is (a Dolby Vision
profile it doesn't decode would stay in the file); otherwise it is made
anew. Apps aren't told why a conversion failed (`error` is `null`): the
reason names the server's files, and the server's log keeps it. A download deleted before its file
is made stops the conversion (`DELETE`); a file made stays for whoever
asks next. One nobody asks about for an hour while it waits its turn is
dropped then (the device that wanted it went): asking again starts it
again.

## Intro and credits detection

Markers come from three sources, best first:

1. **Chapters** titled like an intro or credits (*Intro*, *Opening*, *OP*,
   *Opening Credits*, *Ending*, *ED*, *Credits*, *End Credits*, …), for movies
   and episodes. *Credits* chapters shorter than 15 s (a logo at the very end)
   are ignored.
2. **Audio fingerprints** of episodes. The transcoder computes Chromaprint
   fingerprints of the first half (at most 10 minutes) and the last 35 % (at
   most 8 minutes) of every episode. An intro is audio of 12 s to 3 minutes
   near the start that the nearest episodes of the season (or of the show, for
   a season with one episode) share; credits are shared audio near the end.
   Each episode is compared with up to eight others of its season and the
   boundaries agreed on by most comparisons win; an episode its season shares
   nothing with (a premiere, a short season) is compared with up to six
   episodes of the show's other seasons whose fingerprints are cached.
   Matching follows Jellyfin's Intro Skipper: alignments are found from
   matching values, and parts of a title sequence split by a gag that changes
   every episode count as one. Fingerprints are cached per file version in
   the cache folder, so a new episode only needs its own.
3. **Key frames** (`source: "frames"`), for credits no other episode shares:
   the key frames of the last 35 % (at most 8 minutes) are measured, and the
   last stretch of at least 15 s that is mostly text on black, ending less
   than 2 minutes before the end of the file, is the credits.

A background worker analyses one season at a time after library scans (and
30 s after start-up), waits while anything plays, and records the algorithm
version so improvements re-run automatically. A season that isn't analysed
yet goes first as soon as one of its episodes plays, during playback: the
episodes playing first, then the rest, each followed by a `library_changed`
event so players pick up the markers. It is skipped when
`detect_markers` is off or the transcoder's FFmpeg lacks Chromaprint (chapters
are still used).

## Watch history

A play is recorded once it reaches the point where the item counts as watched
(90 %), with the device it played on. Marking something watched by hand is not
a play.

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/api/history` | `offset`, `limit` (default 50, max 200), `item_id` (the item, or the episodes or tracks under it), `library_id`, `since` (unix seconds); administrators: `user_id=<id>` or `all=1` | `{ total: number, entries: HistoryEntry[] }`, newest first. Your own plays unless asked otherwise; entries of items you can't see are left out. |
| DELETE | `/api/history/{id}` | – | `204`; your own entries, or any for administrators |

```ts
interface HistoryEntry {
  id: Id;
  viewed_at: Timestamp;
  device?: string;                  // "Chrome on Windows", a compatibility API client's name
  user: { id: Id; name: string; avatar_url?: string; avatar_color?: string };
  item: ItemSummary;
}
```

## Activity & events

| Method | Path | Response |
|---|---|---|
| GET | `/api/activity` | `{ scans: ScanStatus[], sessions: SessionInfo[], analysis: AnalysisStatus \| null }` (non-admins only see their own sessions and no analysis) |
| DELETE | `/api/sessions/{id}` | `204`; body optional `{ message?: string }` (at most 200 characters, else `400`). Stops a playback session at once (see below) |
| GET | `/api/events` | Server-Sent Events, `data:` is JSON `ServerEvent` |

### Stopping a stream

`DELETE /api/sessions/{id}` ends a playback session from elsewhere than its
player: administrators stop any session (the dashboard's *Stop*), anyone
else their own (another of their devices); someone else's answers `403`, an
unknown one `404`, one stopped already `204`. Its transcode and HLS end at
once. Then:

- the user's event streams get `{ type: "session_stopped", session_id,
  message, by_admin }`: `message` is the one given (trimmed) or `null`;
  `by_admin` is true when an administrator stopped another user's session.
  Only that user's streams get it; `sessions_changed` goes to everyone, as
  when any session ends;
- the session's next requests answer `410` `{ code: "session_stopped",
  error: "session_stopped", message }` (its status, playlists and segments,
  and progress reports naming it, which still keep the place), for 30
  minutes, so a player without the event stream stops too. Closing it
  (`DELETE /api/playback/{id}`) answers `204`;
- the player stops, leaves, and says "Playback was stopped by the server's
  administrator" with the message, if any.

The compatibility API's `GET /status/sessions/terminate?sessionId=&reason=`
(Tautulli's) does the same for administrators, `reason` as the message. A
player's progress reports through the compatibility API don't start its
stopped session again.

### Watch state on other devices

`{ type: "user_state_changed", item_ids, children }` goes to the user's own
event streams (only theirs, the device that made the change included) when
their watch state changes in a way their screens show:

- something marked watched or unwatched (`POST /api/items/{id}/watched`,
  `…/unwatched`, the compatibility API's `/:/scrobble` and `/:/unscrobble`), unless
  nothing changed (marked watched again);
- a progress report (pley's, or a compatibility API player's `/:/timeline` and
  `/:/progress`) that crosses 90 %, starts a resume point (15 s in) or
  clears one, or that pauses or stops the player somewhere new: where *Play*
  on another device starts. Not the reports that only keep the place every
  ~10 s, nor late ones that change nothing (`age_ms`);
- a title taken out of Continue Watching, or back in it as it plays again.

`item_ids` is the item, then its parent and grandparent: an episode with its
season and show, a track with its album and artist, whose counts and On
Deck follow it. A show, season, album or artist marked as a whole sends one
event for itself, with `children: true`: everything under the first id
changed too, without listing it. Clients ask again for Home and the
libraries' rows (Continue Watching) and for what shows those items: their
pages, children and On Deck, and lists holding them. A play reaching 90 %
also sends `library_changed`, as before; pley's apps gather both for 0.3 s
and ask once, and leave alone what they fetched since the event came (their
own change).

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
  video_in: string | null;      // e.g. "HEVC 3840×2160 Dolby Vision P8.1", "… Dolby Vision P7 FEL", "… HDR10+"
  video_out: string | null;     // e.g. "H.264 1920×1080"; "HEVC 3840×2160 HDR10" when HDR is kept
  audio_in: string | null;      // e.g. "EAC3 5.1"
  audio_out: string | null;     // e.g. "AAC 2.0"
  encoder: string | null;       // e.g. "h264_nvenc", "libx264"
  decoder: string | null;       // "cuda", "vulkan", "cpu", …
  pipeline: string | null;      // "gpu" (zero-copy), "hybrid", "cpu" or "copy"
  tonemap: string | null;       // "libplacebo" | "opencl" | "zscale" when HDR/DV is tone-mapped to SDR
  dolby_vision: string | null;  // remuxes: "kept" (named in master.m3u8), "converted to 8.1" (profile 7)
  hdr10plus_stripped: boolean;  // the remux leaves out HDR10+ (Dolby Vision + HDR10+ streams)
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
  | { type: "libraries_changed" }                    // library created/updated/deleted, or the order changed (the user's own: to their streams only)
  | { type: "sessions_changed" }
  | { type: "watchlist_changed" }                    // the user's My List changed (their streams only)
  | { type: "user_state_changed"; item_ids: Id[]; children: boolean } // the user's watch state changed (their streams only)
  | { type: "session_stopped"; session_id: string; message: string | null; by_admin: boolean }; // their streams only
```

## Users *(admin)*

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/users` | – | `User[]` |
| POST | `/api/users` | `{ username, password?, display_name?, is_admin?, managed?, home? (true), all_libraries? (true), library_ids?, max_age?, pin? }` | `User`; managed profiles need no password |
| PATCH | `/api/users/{id}` | `{ password?, display_name?, is_admin?, home?, disabled?, all_libraries?, library_ids?, max_age?, pin? }` (`null` clears nullable fields) | `User`. pley always keeps one enabled administrator. A new `password` signs the user out on every device (an administrator's own device stays) and in every app using the account or allowed by it |
| DELETE | `/api/users/{id}` | – | `204` (cannot delete yourself); their apps end with them |
| GET | `/api/users/{id}/devices` | – | `Device[]` |
| DELETE | `/api/users/{id}/devices` | – | `204`, signs the user out everywhere, apps included |
| GET | `/api/users/{id}/apps` | – | `AppGrant[]` (see [Apps](#signed-in-apps)) |
| DELETE | `/api/users/{id}/apps/{grant}` | – | `204`, signs that app out |
| DELETE | `/api/users/{id}/two-step` | – | `204`, turns two-step sign-in off (someone who lost their phone and recovery codes) |

## Invitations

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/invites` *(admin)* | – | `Invite[]`: `{ code, created_at, expires_at, uses_left, all_libraries, library_ids, max_age, note }` |
| POST | `/api/invites` *(admin)* | `{ expires_in_days? (7, 0 = never), uses? (1), all_libraries?, library_ids?, max_age?, note? }` | `Invite`; share `/invite/{code}` |
| DELETE | `/api/invites/{code}` *(admin)* | – | `204` |
| GET | `/api/invites/{code}` | – | `{ server_name, expires_at }`, `404` when invalid, used up or expired |
| POST | `/api/invites/{code}/accept` | `{ username, password, display_name? }` | `{ user, token }`: a new (non-home) account with the invitation's access, signed in |

## System *(admin)*

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/system` | – | `SystemInfo` (with `database: { ok, problem?, checked_at }`, the integrity check made at start) |
| GET | `/api/system/resources` | `since` | `{ available, interval_ms, cpus, memory_total, samples: [{ at, cpu, pley_cpu, memory, pley_memory }] }`: the processors' and memory's use, sampled every 2 s for the last five minutes (only the samples after `since`, in Unix milliseconds, when given). `cpu` and `pley_cpu` are shares of all the processors' time, 0–100; `memory` and `pley_memory` bytes. pley is the server and every process it started: a managed transcoder and its FFmpegs. `available` is false where the system can't be sampled (Windows, Linux and macOS can) |
| GET | `/api/system/storage` | – | `{ blobs, blob_bytes, media_folders, media_bytes, cache_bytes }`: space used by stored pictures, generated media files and the cache (docs/STORAGE.md) |
| POST | `/api/system/storage/clean` | – | `{ blobs_removed, media_folders_removed, bytes_freed, cache_files_removed, cache_bytes_freed }`: removes what nothing uses any more, and old cache files |
| GET | `/api/system/backups` | – | `{ health: { ok, problem?, checked_at }, backups: { name, path, bytes, created_at }[] }`, newest first |
| POST | `/api/system/backups` | – | the new backup (a checked `VACUUM INTO` copy; the last 3 are kept) |
| GET | `/api/transcoder` | – | `TranscoderStatus` – live metrics of the transcoder component (all jobs, counters) |
| GET | `/api/settings` | – | `Settings` |
| PUT | `/api/settings` | partial `Settings` | `Settings` |
| POST | `/api/settings/test` | `{ service: "tmdb" \| "tvdb" \| "tvmaze" \| "musicbrainz" \| "transcoder", api_key?, pin? }` | `{ ok, message, ms }`: one request to the service, with the key and PIN given (not saved) or else the saved ones. `message` says what happened, for people |
| GET | `/api/fs` | `path` (omit for roots/drives) | `{ path: string \| null, parent: string \| null, entries: { name: string, path: string }[] }` – directories only |
| GET | `/api/network` | – | `Network`: where pley listens, and could |
| PUT | `/api/network` | `{ web, plex, discovery }` | `Network`, once the new listeners listen or have tried for a moment (3 s at most). Saved in the config file for the next start. `400` for addresses pley can't listen on together, and for a change of what the command line or the environment sets |

```ts
interface Network {
  web: string[];          // the web app and API: "127.0.0.1:7539", "192.168.1.10:7539", "0.0.0.0:7539"
  plex: string[];         // the Plex-compatible API on addresses of its own (32400); [] = off or on pley's port
  plex_on_web: boolean;   // the Plex-compatible API answers on pley's port, to its clients
  discovery: boolean;     // answer the pley apps on the networks (never on loopback addresses)
  fixed: { web: boolean; plex: boolean; discovery: boolean }; // set by --bind/PLEY_BIND, --plex-bind/PLEY_PLEX_BIND, --no-discovery
  listeners: { kind: "web" | "plex" | "discovery"; address: string; error: string | null }[]; // error: why not (yet); tried again every 30 s
  adapters: { name: string; description: string | null; up: boolean; addresses: string[]; // IPv4, loopback left out
              network: { name: string; kind: "public" | "private" | "domain" } | null }[]; // as Windows knows it
  config_file: string;    // where a change is saved
  windows: boolean;       // Windows Firewall decides which kinds of network reach pley
}
```

```ts
interface Settings {
  server_name: string;
  tmdb_api_key: string;             // "" = movies only from file names / local artwork
  metadata_language: string;        // e.g. "en-US" (TMDB and TheTVDB; TVmaze is English)
  tv_metadata: "auto" | "tmdb" | "tvdb" | "tvmaze"; // TV source; "auto" (default) = TMDB if it has a key,
                                    // else TheTVDB if it has one, else TVmaze (no key needed).
                                    // "tmdb"/"tvdb" without their key → 400. A new source refetches every show.
  tvdb_api_key: string;             // TheTVDB v4 key, "" = not set
  tvdb_pin: string;                 // TheTVDB subscriber PIN, for user-supported keys
  music_metadata: boolean;          // MusicBrainz + Cover Art Archive for music libraries (default true)
  hwaccel: "auto" | "none" | "nvenc" | "qsv" | "amf" | "vaapi" | "videotoolbox";
  hardware_decoding: boolean;       // decode on the GPU when possible
  tonemapping: "auto" | "libplacebo" | "opencl" | "zscale" | "none"; // HDR10/HLG/Dolby Vision → SDR
  max_transcodes: number;
  transcode_preset: "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium";
  throttle_transcodes: boolean;     // limit ffmpeg to ~2× realtime once a buffer is built
  scan_interval_minutes: number;    // 0 disables periodic scans
  watch_libraries: boolean;         // pick up file changes automatically
  detect_markers: boolean;          // find intros and credits in the background (default true)
  seek_previews: boolean;           // make seek-bar thumbnails when an item is first opened (default true)
  previews_in_background: boolean;  // also for whole libraries after scans, one file at a time (default true)
  third_party_apps: boolean;        // apps by other publishers may sign in (default false); turning it off
                                    // signs them out. pley's own apps always may
}

interface SystemInfo {
  name: string;
  version: string;
  commit: string;                   // the build's short hash, "unknown" without the repository
  os: string;
  arch: string;
  uptime_secs: number;
  data_dir: string;
  transcoder: {
    available: boolean;
    managed: boolean;               // spawned and supervised by pley (vs. external URL)
    url: string | null;
    version: string | null;
    commit: string | null;
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
