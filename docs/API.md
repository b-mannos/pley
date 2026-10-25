# pley HTTP API (v1)

All endpoints live under `/api`. Requests and responses are JSON (`snake_case` keys)
unless stated otherwise. Timestamps are **unix seconds**; durations/positions are
**milliseconds**. Ids are integers unless stated otherwise.

## Authentication

* `POST /api/auth/login` (or `/api/setup`, `/api/auth/switch`, accepting an
  invitation, a single sign-on) sets an `HttpOnly` cookie named `pley_token`.
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

Every answer carries `X-Content-Type-Options: nosniff`, `X-Frame-Options:
DENY`, `Referrer-Policy: same-origin` and a `Content-Security-Policy` that
lets pages load only pley's own scripts and styles. Password checks queue
(a few at a time); when one has waited 10 seconds the answer is `503`.

### Errors

Non-2xx responses carry:

```json
{ "code": "not_found", "message": "item 42 not found" }
```

`code` is one of `bad_request`, `unauthorized`, `forbidden`, `not_found`,
`conflict`, `too_many_requests` (sign-in, PIN, password and code attempts:
after 5 failures in 15 minutes, counting attempts still being checked),
`two_step` (`401`: the account has two-step sign-in, and the code is missing
or wrong), `unavailable`, `internal`.

---

## Types

```ts
type Id = number;
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
  has_password: boolean;           // false: a managed profile, or signs in only with single sign-on
  sso: boolean;                    // linked to an identity at the single sign-on provider
  two_step: boolean;               // signs in with a code from an authenticator app after the password
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
  summary?: string;                 // in lists only for episodes; always in ItemDetail
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
  edition: string | null;           // from `{edition-Final Cut}` in the file's or its folder's name
  selected_audio: number | null;    // tracks to start with for this user (remembered
  selected_subtitle: number | null; // choice or their defaults); null = default / off
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
  audio_converted: boolean;         // the audio is converted (also in a remux, where the video is copied)
  subtitles: SubtitleTrack[];       // all subtitle tracks of the file
  reasons: string[];                // why it isn't played as it is, one short sentence each
  reason: string | null;            // the same in one line
  auto_quality: string | null;      // the original converted: the closest quality players offer ("30m" = 30 Mbps 4K,
                                    // "20m", "15m" = 15 Mbps 4K, "12m", "8m", "4m", "2m"); null when a quality was asked for
}
```

---

## Public endpoints

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/health` | – | `{ "status": "ok" }` |
| GET | `/api/server` | – | `{ name: string, version: string, setup_required: boolean, setup_code_required: boolean, sso: { name, start } \| null }`. Setup takes the code from the server's log. `sso`: single sign-on is on; `start` is where browsers go to sign in (`?next=/page`) |
| POST | `/api/setup` | `{ username, password, setup_code }` | `{ user: User, token? }` — creates the first (admin) account and signs in. `409` once a user exists, `403` without the right `setup_code` (5 tries per 15 minutes from each address, then `429`) |
| POST | `/api/auth/login` | `{ username, password, code? }` | `{ user: User, token? }` (`401` on bad credentials). With two-step sign-in, `code` is a code from the authenticator app or a recovery code: without it, or with a wrong one, `401` with `code: "two_step"` |
| GET | `/api/auth/sso/start` | `next=/page` | A page browsers are sent to: `303` to the provider (from pley's public address; elsewhere, first to that). Problems come back as `/login?sso=<code>` |
| GET | `/api/auth/sso/callback` | `code`, `state` (from the provider) | `303` to `next` (or `/profiles?next=…` for a household), signed in; or `/login?sso=<code>`, `/account?sso=<code>` (see below) |
| GET | `/api/auth/sso/client` | – | pley's client metadata document (when it has no client ID): `{ client_id, client_name, redirect_uris, grant_types, response_types, token_endpoint_auth_method: "none", scope }` |

## Auth

| Method | Path | Body | Response |
|---|---|---|---|
| POST | `/api/auth/logout` | – | `204`, cookie cleared |
| GET | `/api/auth/me` | – | `User` |
| PUT | `/api/auth/password` | `{ current_password, new_password }` | `204`; signs the user out everywhere but this device |
| GET | `/api/auth/profiles` | – | `User[]`: home profiles this user can switch to (empty for non-home users) |
| POST | `/api/auth/switch` | `{ user_id, pin?, password?, code? }` | `{ user, token? }`: this device becomes that profile (old session ends). A profile with a PIN needs it; after 100 wrong PINs in a row it stops working until it is set again. Without one, a profile that can see more than yours (an administrator, a higher age limit, libraries yours can't open) needs its password, and its `code` with two-step sign-in, unless you are an administrator; a managed one can't be switched to until it has a PIN. `429` after 5 wrong tries in 15 min. |
| POST | `/api/auth/sso/link` | `{ password, code? }` | `204`: the next single sign-on (`/api/auth/sso/start`, which the web app opens next) links that identity to this account. From pley's public address only (`409` elsewhere); not for managed profiles or accounts without a password. With two-step sign-in, it takes a `code` too (single sign-on skips it) |

Login attempts are throttled the same way (5 failures per username per 15 min).
Disabled accounts get `403` at login and their sessions stop working.

### Single sign-on

With `sso_enabled`, people sign in with an OpenID Connect provider
(authorization code flow with PKCE, the `state` bound to the browser by a
cookie, the ID token's signature, issuer, audience, nonce and expiry
checked). pley asks for the scopes `openid email`, plus `profile` when it
makes new accounts and `groups` when it checks a group, and keeps none of the
provider's tokens. An identity (the issuer and the ID token's `sub`) signs in
to the account it was linked to; names and e-mail addresses never match
accounts. With `sso_new_accounts`, someone new gets an account without
libraries; with `sso_group`, only members of that group get in at all.

`?sso=` codes: `linked` (done), `off`, `expired` (took too long, or started in
another browser), `denied` (cancelled at the provider), `provider` (the
provider couldn't be reached or refused; the log says why), `invalid` (the ID
token was refused), `unknown` (no account is linked to that identity),
`disabled`, `group` (not in the group that may sign in), `signin` (linking:
not signed in as the user who asked any more), `taken` (linked to another
account), `error`.

### Two-step sign-in

Optional for each account with a password: after the password, a 6-digit
code from an authenticator app (TOTP: SHA-1, 30-second steps), or one of ten
recovery codes. Each code works once; wrong ones count like wrong passwords,
and after 100 in a row only a recovery code signs in. Single sign-on skips
it. `pley reset-password` turns it off.

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
| PUT | `/api/me/pin` | `{ pin: string \| null, password }` | `204`; 4–8 digits, `null` removes it (`403` with a wrong password) |
| PUT | `/api/me/avatar` | raw image bytes (JPEG, PNG, WebP, GIF, BMP; ≤ 10 MB) | `User`; stored as a 256 px square JPEG |
| DELETE | `/api/me/avatar` | – | `204` |
| GET | `/api/users/{id}/avatar` | – | `image/jpeg` (any signed-in user, for the profile switcher) |
| GET | `/api/me/devices` | – | `Device[]`: `{ id, name, user_agent, ip, created_at, last_seen_at, current }` |
| GET | `/api/me/sso` | – | `{ enabled, name?, linked?, email?, has_password?, public_url?, start? }`: single sign-on for this account |
| DELETE | `/api/me/sso` | `{ password }` | `204`, unlinks the identity (`403` for accounts without a password: they would be locked out) |
| DELETE | `/api/me/devices/{id}` | – | `204`, signs that device out |

Titles you haven't picked tracks for start with your defaults: the main
audio track in `audio_language` (the file's default without one), and
subtitles as `subtitle_mode` says: `off` never; `forced` (the default) only
forced subtitles in the language of the audio, which translate foreign
dialogue; `foreign` full subtitles in `subtitle_language` (or the audio
language) when the audio is tagged with another language; `always` in that
language whenever the file has them. Every mode but `off` falls back to the
forced ones. A choice made for a show or a movie wins over these.

## Access rules

Administrators see everything. Other users see every library or only the
ones in `library_ids`, and with `max_age` only movies and shows whose content
rating allows that age (US, UK, DE, FR, AU, CA and numeric certifications are
understood; seasons and episodes go by their show; unrated titles are hidden;
music is exempt). Hidden libraries and items answer `404` everywhere: lists,
hubs, search, artwork, streams and the Plex-compatible API.

## Libraries

Libraries are listed in one order, the same for every user: the one
administrators choose (new libraries go last). Lists of libraries, the home
page's rows and the Plex-compatible API's sections all follow it; users who
may open only some libraries see those, in that order.

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/libraries` | – | `Library[]` in their order |
| POST | `/api/libraries` *(admin)* | `{ name, kind, paths: string[] }` | `Library` (a scan is queued automatically) |
| PUT | `/api/libraries/order` *(admin)* | `{ ids: Id[] }` (every library, once) | `Library[]` in the new order. `400` when an id is listed twice, `409` when `ids` aren't the libraries there are (one was added or deleted since they were listed) |
| GET | `/api/libraries/{id}` | – | `Library` |
| PATCH | `/api/libraries/{id}` *(admin)* | `{ name?, paths? }` | `Library` |
| DELETE | `/api/libraries/{id}` *(admin)* | – | `204` |
| POST | `/api/libraries/{id}/scan` *(admin)* | `{ force?: boolean }` (force = re-probe files and refresh metadata) | `202` |
| POST | `/api/libraries/scan` *(admin)* | – | `202` (scan all) |
| GET | `/api/libraries/{id}/items` | query: `sort=title\|year\|added\|rating\|user_rating\|release\|duration\|last_played\|bitrate` (default `title`), `order=asc\|desc`, `watched=true\|false`, `genre=<name>`, `year=<yyyy>`, `offset`, `limit` | `{ total: number, items: ItemSummary[] }` — top-level items only; `total` counts every match, not just the page; `watched=true` matches shows whose every episode is watched. Items leave out `backdrop_url` and `thumb_url` (grids show posters). Answers carry an `ETag` and `Cache-Control: private, no-cache`; `If-None-Match` with the same tag answers `304` while the library, the user's watch state and their access are unchanged |
| GET | `/api/libraries/{id}/filters` | – | `{ genres: string[], years: number[] }` |
| GET | `/api/libraries/{id}/hubs` | – | `{ hubs: Hub[] }` (continue watching, recently added, recently released, top rated – empty hubs omitted) |

## Items

| Method | Path | Body / Query | Response |
|---|---|---|---|
| GET | `/api/items/{id}` | – | `ItemDetail` |
| GET | `/api/items/{id}/children` | – | `{ items: ItemSummary[] }` (seasons of a show, episodes of a season, albums of an artist, tracks of an album), ordered by index |
| GET | `/api/items/{id}/on-deck` | – | `{ item: ItemSummary \| null }` – for show/season: the episode to play when pressing *Play* (in progress, else next unwatched, else first) |
| GET | `/api/items/{id}/related` | `limit` (default 20, max 60) | `{ items: ItemSummary[] }` – "More Like This" for movies and shows: the same library's titles sharing collections, directors, creators and top-billed cast, genres and a period, best first (empty for other kinds) |
| GET | `/api/items/{id}/next` | – | `{ item: ItemSummary \| null }` – episode/track following this one (for autoplay) |
| POST | `/api/items/{id}/progress` | `{ position_ms, duration_ms?, state: "playing"\|"paused"\|"stopped", session_id? }` | `204`. ≥ 90 % marks the item watched and clears the resume point. |
| POST | `/api/items/{id}/watched` | – | `204` (containers: all leaves) |
| PATCH | `/api/items/{id}` *(admin)* | `{ title?, sort_title?, original_title?, year?, summary?, tagline?, studio?, content_rating?, release_date?, genres? }` | `ItemDetail`. A value edits and locks the field (refreshes and scans keep it); `null` unlocks it. `locked_fields` in `ItemDetail` lists locked fields. |
| GET | `/api/items/{id}/matches?title=&year=` *(admin)* | – | TMDB candidates `{ tmdb_id, title, original_title, year, overview, poster_url }[]` for movies and shows (needs a TMDB key) |
| POST | `/api/items/{id}/match` *(admin)* | `{ tmdb_id }` | `ItemDetail` after re-matching and refreshing. A show whose TV source is TVmaze or TheTVDB is found there by the chosen entry's IMDb and TheTVDB ids. |
| GET | `/api/items/{id}/artwork/{poster\|backdrop\|thumb}` *(admin)* | – | TMDB images `{ url, preview_url, width, height, language, votes }[]` |
| PUT | `/api/items/{id}/artwork/{kind}` *(admin)* | `{ url }` (a TMDB image) | `ItemDetail`; the picture is downloaded and locked |
| POST | `/api/items/{id}/artwork/{kind}/upload` *(admin)* | raw image bytes (≤ 25 MB) | `ItemDetail`; stored as JPEG and locked |
| DELETE | `/api/items/{id}/artwork/{kind}` *(admin)* | – | `204`; back to local or downloaded artwork |
| PUT | `/api/items/{id}/rating` | `{ rating: number \| null }` (0-10, rounded to half stars; `null` clears it) | `204`; the user's own rating, shown as `user_state.rating` |
| PUT | `/api/items/{id}/streams` | `{ file_id?, audio_index, subtitle_index }` (`subtitle_index: null` = off) | `204`. Remembers the choice for the whole show (episodes) or the movie, as language/title/forced, so every episode starts with its own matching tracks. |
| POST | `/api/items/{id}/unwatched` | – | `204` |
| POST | `/api/items/{id}/refresh` *(admin)* | – | `202` re-fetch metadata from the item's source (movies: TMDB; shows, seasons, episodes: the TV source; artists, albums, tracks: MusicBrainz). `400` when that source is off or lacks its key. |
| POST | `/api/items/{id}/analyze` *(admin)* | – | `202` find the intro and credits again: a movie's chapters, or every episode of an episode's season, a season or a show. `400` for other kinds. |
| GET | `/api/items/{id}/images/{poster\|backdrop\|thumb}` | query: `w=<px>` (optional resize, bucketed), `v=` (cache buster, ignored) | image bytes (`image/jpeg`), long-lived cache headers. Falls back to parent artwork (season → show). `404` if none. |

## Home & search

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/api/hubs/home` | – | `{ hubs: Hub[] }` – `continue_watching` (in-progress + next up), then `recently_added:<library_id>` per library, in the libraries' order |
| GET | `/api/search` | `q`, `limit` (default 40) | `{ items: ItemSummary[], people: PersonSummary[] }`: items ranked by relevance; people whose name has words starting with each word of `q`, most credited first |

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
Plex-compatible API's syntax (the query string of
`/library/sections/{id}/all`, such as `genre=Comedy&unwatched=1&sort=year:desc`;
see docs/PLEX.md), and the collection shows what it finds, in its order, as
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
| GET | `/api/playback/{session_id}/{n}.ts` | – | MPEG-TS segment |
| GET | `/api/files/{id}/stream` | – | raw file, supports `Range` (direct play) |
| GET | `/api/files/{id}/subtitles/{index}.vtt` | – | WebVTT (text subtitles; extracted/converted on demand and cached). Any embedded track may come partial: one still being read out of its file comes with the cues found so far and `X-Pley-Complete: 0` (not cached), so check that header and ask again for the rest. A Matroska file's track usually comes whole at once, read through its Cues when they list it. Requests for a subtitle file being converted share that conversion, which goes on (to the cache) when they stop waiting; one that failed is tried again on the next request |
| GET | `/api/files/{id}/trickplay` | – | `{ id, interval_ms, width, height, columns, rows, count, sheets }` seek-bar previews; `404` while they are being made (asking, or opening the item, queues them) |
| GET | `/api/files/{id}/trickplay/{n}.jpg?v={id}` | – | sprite sheet `n`: thumbnail `k` is at column `k % columns`, row `k / columns` of sheet `k / (columns × rows)` and shows the video at `k × interval_ms` |
| GET | `/api/files/{id}/chapters/{n}.jpg` | – | picture of chapter `n` (0-based), cut from the seek previews (a few seconds into the chapter); `404` until the file has previews |

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
  device?: string;                  // "Chrome on Windows", a Plex client's name
  user: { id: Id; name: string; avatar_url?: string; avatar_color?: string };
  item: ItemSummary;
}
```

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
  | { type: "libraries_changed" }                    // library created/updated/deleted, or the order changed
  | { type: "sessions_changed" };
```

## Users *(admin)*

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/users` | – | `User[]` |
| POST | `/api/users` | `{ username, password?, display_name?, is_admin?, managed?, home? (true), all_libraries? (true), library_ids?, max_age?, pin? }` | `User`; managed profiles need no password |
| PATCH | `/api/users/{id}` | `{ password?, display_name?, is_admin?, home?, disabled?, all_libraries?, library_ids?, max_age?, pin? }` (`null` clears nullable fields) | `User`. pley always keeps one enabled administrator. |
| DELETE | `/api/users/{id}` | – | `204` (cannot delete yourself) |
| GET | `/api/users/{id}/devices` | – | `Device[]` |
| DELETE | `/api/users/{id}/devices` | – | `204`, signs the user out everywhere |
| DELETE | `/api/users/{id}/sso` | – | `204`, unlinks the user's identities at the single sign-on provider |
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
| GET | `/api/system/storage` | – | `{ blobs, blob_bytes, media_folders, media_bytes, cache_bytes }`: space used by stored pictures, generated media files and the cache (docs/STORAGE.md) |
| POST | `/api/system/storage/clean` | – | `{ blobs_removed, media_folders_removed, bytes_freed, cache_files_removed, cache_bytes_freed }`: removes what nothing uses any more, and old cache files |
| GET | `/api/system/backups` | – | `{ health: { ok, problem?, checked_at }, backups: { name, path, bytes, created_at }[] }`, newest first |
| POST | `/api/system/backups` | – | the new backup (a checked `VACUUM INTO` copy; the last 3 are kept) |
| GET | `/api/transcoder` | – | `TranscoderStatus` – live metrics of the transcoder component (all jobs, counters) |
| GET | `/api/settings` | – | `Settings` (`sso_client_secret` always empty, `sso_client_secret_set` says whether there is one) |
| PUT | `/api/settings` | partial `Settings` | `Settings` |
| POST | `/api/settings/test` | `{ service: "tmdb" \| "tvdb" \| "tvmaze" \| "musicbrainz" \| "transcoder" \| "sso", api_key?, pin?, issuer?, client_id?, public_url? }` | `{ ok, message, ms }`: one request to the service, with the key and PIN given (not saved) or else the saved ones; for `sso`, the provider's discovery document and keys, and how pley would sign in with it. `message` says what happened, for people |
| GET | `/api/fs` | `path` (omit for roots/drives) | `{ path: string \| null, parent: string \| null, entries: { name: string, path: string }[] }` – directories only |

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
  public_url: string;               // where people reach pley ("https://pley.example.com"); single sign-on runs there
  sso_enabled: boolean;             // needs sso_issuer and public_url (https without a client ID)
  sso_name: string;                 // on the sign-in button
  sso_issuer: string;               // the provider's issuer, exactly
  sso_client_id: string;            // "" = pley's client metadata document (CIMD), for providers that take those
  sso_client_secret: string;        // write-only: always "" in answers
  sso_new_accounts: boolean;        // identities pley doesn't know get an account, without libraries (default false)
  sso_group: string;                // only members of this group (the `groups` claim) may sign in; "" = anyone
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
