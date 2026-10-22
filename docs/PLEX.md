# Plex-compatible API

pley speaks enough of the Plex Media Server API for tools written against
Plex to work with it: [python-plexapi](https://github.com/pushingkarmaorg/python-plexapi)
scripts, and tools built the same way (Tautulli, Kometa, direct-play clients).
It runs on its own port and uses the same libraries, users, watch state and
playback sessions as pley's own API, so anything done through it shows up in
the web UI and the other way round.

The official Plex apps are out of scope: they sign in through plex.tv and
discover servers there, which pley does not do.

## Enabling and port

The API listens on `0.0.0.0:32400`, Plex's port, by default.

| Where | Setting |
|---|---|
| Command line | `--plex-bind 127.0.0.1:32400` or `--plex-bind off` |
| Environment | `PLEY_PLEX_BIND` |
| `pley.toml` | `plex_bind = "0.0.0.0:32400"` |

If the port is taken (a real Plex server runs on the same machine), pley logs a
warning and keeps running without this API. Pick another port with
`--plex-bind`.

## Authentication

Use a pley session token as the Plex token. Get one by signing in:

```sh
curl -s -X POST http://localhost:7539/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"…"}'
# {"user":{…},"token":"<token>"}
```

The token is accepted the Plex way, as the `X-Plex-Token` header or query
parameter, and the pley way (`Authorization: Bearer`, the `pley_token` cookie).
Every endpoint needs it except `/identity`.

```python
from plexapi.server import PlexServer
plex = PlexServer('http://localhost:32400', '<token>')
for movie in plex.library.section('Movies').search(unwatched=True):
    print(movie.title, movie.media[0].videoResolution)
```

Each user sees their own watch state, Continue Watching and sessions.
Administrators also see every user's sessions and can refresh libraries.

## Formats

Responses are XML `MediaContainer`s like Plex's. With
`Accept: application/json` they are JSON in Plex's layout
(`{"MediaContainer": {"Metadata": […]}}`). Paging works through
`X-Plex-Container-Start` and `X-Plex-Container-Size`, as headers or query
parameters; containers report `size`, `totalSize` and `offset`.

## Mapping

- **IDs.** `ratingKey` is the pley item id and `librarySectionID` the library
  id. Media and parts use the media file id. Stream ids are
  `file id × 10000 + stream index`.
- **Types.** Movies and episodes are `Video` elements, shows, seasons, artists
  and albums are `Directory` elements, and tracks are `Track` elements, with
  Plex's `type` values.
- **GUIDs.** Every item gets a `plex://<type>/<id>` guid. With
  `includeGuids=1`, and always in details, `Guid` children carry `imdb://` and
  `tmdb://` ids when metadata is known.
- **Watch state.** `viewCount` is the play count of watched items and 0 once an
  item is marked unplayed. `viewOffset` is the resume position.
  `/:/scrobble`, `/:/unscrobble`, `/:/progress` and `/:/timeline` use the same
  code as pley's player.
- **Sessions.** A client that reports `/:/timeline` while playing becomes a
  pley playback session. It is listed in `/status/sessions` (with `User`,
  `Player` and `Session`, plus `TranscodeSession` for HLS) and in pley's
  dashboard.
- **Streaming.** `/library/parts/{id}/{version}/file.ext` serves the file with
  range requests (direct play; `?download=1` adds an attachment header).
  `/video/:/transcode/universal/start.m3u8` (and `/audio/…` for tracks) starts
  a pley HLS session, remuxed when the video can be copied and transcoded
  otherwise (`directStream=0` forces transcoding). It honors `offset`,
  `maxVideoBitrate`, `videoResolution`, `audioStreamID` and burned-in
  `subtitleStreamID`. The master playlist points at
  `session/<id>/base/index.m3u8`. Session URLs need no token: the 128-bit
  session id is the credential, as with Plex.
- **Cast and crew.** Details carry `Role` (with the part as `role`),
  `Director`, `Writer` and `Producer` tags. Their `id` is the pley person id and
  `thumb` the TMDB photo.
- **Artwork.** `thumb` and `art` attributes point at
  `/library/metadata/{id}/thumb/{version}` and `/art/{version}`.
  `/photo/:/transcode?url=…&width=…` resizes them through pley's image cache.

## Endpoints

| Area | Endpoints |
|---|---|
| Server | `/`, `/identity`, `/servers`, `/accounts`, `/myplex/account`, `/system`, `/prefs`, `/:/prefs`, `/media/providers` |
| Sections | `/library`, `/library/sections`, `/library/sections/{id}`, `…/all`, `…/albums`, `…/recentlyAdded`, `…/onDeck`, `…/search`, `…/firstCharacter`, `…/collections`, `…/{genre,year,decade,contentRating,studio,actor,director,writer,producer}`, `…/refresh`, `/library/sections/all/refresh` |
| Items | `/library/metadata/{id}` (comma-separated ids allowed), `…/children`, `…/allLeaves`, `…/grandchildren`, `…/thumb`, `…/art`, `…/refresh` |
| Lists | `/library/all`, `/library/onDeck`, `/library/recentlyAdded`, `/hubs`, `/hubs/sections/{id}`, `/hubs/continueWatching/items`, `/hubs/home/recentlyAdded`, `/hubs/search`, `/search` |
| Collections | `/library/sections/{id}/collections` (also `…/all?type=18`), `/library/collections/{id}`, `…/children`, `…/{thumb,art,composite}[/{version}]` |
| Playlists | `GET`/`POST /playlists` (`?type=&title=&uri=…/library/metadata/1,2`), `GET`/`PUT` (`?title=`)/`DELETE /playlists/{id}`, `GET`/`PUT /playlists/{id}/items` (`?uri=`), `DELETE /playlists/{id}/items/{playlistItemID}`, `PUT …/{playlistItemID}/move?after=`, `/playlists/{id}/composite/{version}` |
| Playback | `/library/parts/…`, `/library/streams/{id}` (subtitles), `/:/timeline`, `/:/progress`, `/:/scrobble`, `/:/unscrobble`, `/status/sessions`, `/transcode/sessions`, `/{video,audio}/:/transcode/universal/{start.m3u8,decision,stop,ping,session/…}` |
| Images | `/photo/:/transcode` |

## Searching and filtering

`/library/sections/{id}/all` accepts `type` (1 movie, 2 show, 3 season, 4
episode, 8 artist, 9 album, 10 track), `sort` (`titleSort`, `addedAt`,
`originallyAvailableAt`, `year`, `rating`, `audienceRating`, `lastViewedAt`,
`viewCount`, `duration`, `index`, `id`, `updatedAt`; with `:desc`, several
comma-separated), `limit` and Plex filters with their operators:

- strings (`title`, `studio`): contains, `!=` does not contain, `==` is, `!==`
  is not, `<=` begins with, `>=` ends with;
- numbers (`year`, `decade`, `viewCount`, `duration`, `rating`, `index`, `id`):
  is, `!=`, `>>=`, `<<=`;
- tags (`genre`, `contentRating`), booleans (`unwatched`, `inProgress`), dates
  (`addedAt`, `lastViewedAt`, `originallyAvailableAt`, as unix time or relative
  like `-30d`) and `guid`;
- people (`actor`, `director`, `writer`, `producer`): a person id (the tag `id`,
  also listed by `/library/sections/{id}/actor` and friends) or a name, `!=`
  excludes.

Filters on ancestors work (`artist.id=5` on albums, `show.title=…` on
episodes). With `includeMeta=1` the response describes these filters, sorts
and fields in a `Meta` element; python-plexapi validates searches against it.

## Tested

With python-plexapi 4.18 the following work against the test media:

- connecting, the library and its sections, `all()`, `get()`, `search()` with
  title, `unwatched`, year, sort and paging, `recentlyAdded()`,
  `listSorts()` and `listFilterChoices()`;
- item details: media, parts, streams, guids, genres, `reload()`;
- shows (`seasons()`, `episodes()`, `episode(season=, episode=)`, `onDeck()`),
  music (`albums()`, `tracks()`, `album()`, `artist()`);
- `markPlayed()` and `markUnplayed()`, `updateProgress()`, `updateTimeline()`;
- `server.search()`, `library.search()`, `onDeck()`, hubs, `continueWatching()`;
- `systemAccounts()`, `account()`, `clients()`, `playlists()`, `history()`,
  `totalDuration` and `totalStorage`;
- `getStreamURL()` with the playlist and segments it leads to, direct-play
  range requests, `sessions()` and `transcodeSessions()`;
- a non-administrator only sees their own sessions and accounts and cannot
  refresh libraries.

## Not supported

- plex.tv: sign-in, server claiming, sharing, remote access and the official
  Plex apps.
- Smart collections and smart playlists, play queues, and creating or editing
  collections through the Plex API (they are read-only here; manage them in
  pley). Playlists can be created and edited.
- Editing metadata, labels, ratings and posters through the Plex API:
  `/:/rate` is accepted and ignored, edit requests fail. Edit in pley.
- Filters on descendants (`episode.title` on shows) and on `resolution`, `hdr`,
  audio or subtitle language: ignored, so they don't narrow results.
- Play history (`/status/sessions/history/all` is empty), the
  `/:/websockets/notifications` event stream, player remote control, markers,
  related and similar items, photos and live TV.
- Per-client capability profiles: universal transcode streams are H.264/AAC
  HLS.
