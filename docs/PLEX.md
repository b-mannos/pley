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
parameter, and the pley way (`Authorization: Bearer`, `X-Pley-Token` or
`?token=`). The web UI's `pley_token` cookie is refused here: browsers send
it to this port too, and some Plex endpoints change things on a GET.
Tokens of apps signed in on pley's own page (`pley_at_…`) are refused as
well, and so are the HLS session addresses of playback those apps started:
Plex tools use a token from `POST /api/auth/login`. Every endpoint needs a
token except `/identity`.

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
- **Order.** Sections, and the home hubs of each, come in the order
  administrators choose in pley (Settings → Libraries); new ones come last.
- **Types.** Movies and episodes are `Video` elements, shows, seasons, artists
  and albums are `Directory` elements, and tracks are `Track` elements, with
  Plex's `type` values.
- **GUIDs.** Every item gets a `plex://<type>/<id>` guid. With
  `includeGuids=1`, and always in details, `Guid` children carry the ids
  metadata found, as Plex reports them: `imdb://`, `tmdb://` (movies and
  shows), `tvdb://` (shows) and `mbid://` (artists and albums). The `guid`
  filter finds items by any of them.
- **Editions.** Movies whose versions are tagged like Plex's
  (`{edition-Director's Cut}` in the file or folder name) carry the default
  version's `editionTitle`.
- **Watch state.** `viewCount` is the play count of watched items and 0 once an
  item is marked unplayed. `viewOffset` is the resume position.
  `/:/scrobble`, `/:/unscrobble`, `/:/progress` and `/:/timeline` use the same
  code as pley's player.
- **Sessions.** A client that reports `/:/timeline` while playing becomes a
  pley playback session. It is listed in `/status/sessions` (with `User`,
  `Player` and `Session`, plus `TranscodeSession` for HLS) and in pley's
  dashboard.
- **Notifications.** `/:/websockets/notifications` (token as `X-Plex-Token`
  query parameter or header) streams JSON `NotificationContainer`s:
  `playing` when a session starts, pauses, resumes or stops, and every 10 s
  while its position moves (`PlaySessionStateNotification` with the
  `sessionKey` of `/status/sessions`); `timeline` for items added, first
  `state` 0 with `metadataState` `created`, then 5 (what Tautulli's recently
  added notifications wait for); `activity` for library scans; `status`
  (`LIBRARY_UPDATE`) when a scan ends. Each user hears about their own
  sessions (administrators about all) and the libraries they may open; the
  token is checked again every 10 s and the socket closes once it is revoked.
- **Streaming.** `/library/parts/{id}/{version}/file.ext` serves the file with
  range requests (direct play; `?download=1` adds an attachment header).
  `/video/:/transcode/universal/start.m3u8` (and `/audio/…` for tracks) starts
  a pley HLS session, remuxed when the video can be copied and transcoded
  otherwise (`directStream=0` forces transcoding). It honors `offset`,
  `maxVideoBitrate`, `videoResolution`, `audioStreamID` and burned-in
  `subtitleStreamID`. The master playlist points at
  `session/<id>/base/index.m3u8`. Session URLs need no token: the 128-bit
  session id is the credential, as with Plex.
- **Preview thumbnails and chapters.** Parts of files with seek previews have
  `indexes="sd"`; the BIF index is cut from pley's previews. Details list the
  first version's `Chapter`s, with a `thumb` once the file has previews.
- **Markers.** With `includeMarkers=1`, details list the intro and credits
  pley found in the first version as `Marker`s (`type`, `startTimeOffset`,
  `endTimeOffset` in ms; credits are `final`), for Skip Intro and Skip
  Credits in clients.
- **Cast and crew.** Details carry `Role` (with the part as `role`),
  `Director`, `Writer` and `Producer` tags. Their `id` is the pley person id and
  `thumb` the TMDB photo.
- **Artwork.** `thumb` and `art` attributes point at
  `/library/metadata/{id}/thumb/{version}` and `/art/{version}`.
  `/photo/:/transcode?url=…&width=…` resizes them through pley's image cache.

## Endpoints

| Area | Endpoints |
|---|---|
| Server | `/`, `/identity`, `/servers`, `/accounts`, `/myplex/account`, `/system`, `/prefs`, `/:/prefs`, `/media/providers`, `/:/websockets/notifications` |
| Sections | `/library`, `/library/sections`, `/library/sections/{id}`, `…/all`, `…/albums`, `…/recentlyAdded`, `…/onDeck`, `…/search`, `…/firstCharacter`, `…/collections`, `…/{genre,year,decade,contentRating,studio,actor,director,writer,producer}`, `…/refresh`, `/library/sections/all/refresh` |
| Items | `/library/metadata/{id}` (comma-separated ids allowed), `…/extras` (trailers and other bonus material as `clip`s with Plex's `extraType` and `subtype`), `…/children`, `…/allLeaves`, `…/grandchildren`, `…/related` ("More Like This" hub), `…/similar` (the same items), `…/thumb`, `…/art`, `…/refresh` |
| Lists | `/library/all`, `/library/onDeck`, `/library/recentlyAdded`, `/hubs`, `/hubs/sections/{id}`, `/hubs/continueWatching/items`, `/hubs/home/recentlyAdded`, `/hubs/search`, `/search`, `PUT /actions/removeFromContinueWatching?ratingKey=` (the movie, or the episode's show, leaves the user's Continue Watching until it is played again, as `DELETE /api/continue-watching/{id}`) |
| Collections | `/library/sections/{id}/collections` (also `…/all?type=18`), `/library/collections/{id}`, `…/children`, `…/{thumb,art,composite}[/{version}]` |
| Playlists | `GET`/`POST /playlists` (`?type=&title=&uri=…/library/metadata/1,2`, or `smart=1&uri=…/library/sections/{id}/all?…` for a smart playlist of that listing; `?smart=0|1` filters the list), `GET`/`PUT` (`?title=`)/`DELETE /playlists/{id}`, `GET`/`PUT /playlists/{id}/items` (`?uri=`), `DELETE /playlists/{id}/items/{playlistItemID}`, `PUT …/{playlistItemID}/move?after=`, `/playlists/{id}/composite/{version}` |
| Previews | `/library/parts/{id}/indexes/sd` (BIF), `/library/parts/{id}/indexes/sd/{offset}` (the frame at `offset` ms), `/library/media/{id}/chapterImages/{n}` |
| Playback | `/library/parts/…`, `/library/streams/{id}` (subtitles), `/:/timeline`, `/:/progress`, `/:/scrobble`, `/:/unscrobble`, `/status/sessions`, `/status/sessions/terminate?sessionId=&reason=` (administrators: stops the session whose `Session` `id` it is, `reason` as the message its player shows, as `DELETE /api/sessions/{id}`; that client's reports don't start it again), `/transcode/sessions`, `/{video,audio}/:/transcode/universal/{start.m3u8,decision,stop,ping,session/…}` |
| Play queues | `POST /playQueues` (`uri=` items, a container, a section listing or a playlist, or `playlistID=`; `key=` the item to start with; `shuffle=1`; `continuous=1` plays the rest of an episode's show after it), `GET`/`PUT /playQueues/{id}` (`uri=`/`playlistID=` adds, `next=1` right after the playing item), `DELETE /playQueues/{id}/items[/{playQueueItemID}]`, `PUT …/items/{playQueueItemID}/move?after=`, `PUT /playQueues/{id}/shuffle`, `…/unshuffle`; kept in memory for a day, for the user who made them |
| History | `/status/sessions/history/all` (`accountID`, `librarySectionID`, `metadataItemID`, `viewedAt>=`; administrators see every account, others their own), `DELETE /status/sessions/history/{id}` |
| Images | `/photo/:/transcode` |

## Editing

Administrators edit metadata the way python-plexapi (and so Kometa) does:

- `PUT /library/sections/{id}/all?type=1&id=1,2&title.value=…&title.locked=1`,
  or `PUT /library/metadata/{id}?…`, for `title`, `titleSort`,
  `originalTitle`, `year`, `summary`, `tagline`, `studio`, `contentRating`
  and `originallyAvailableAt`. Edited fields are kept through refreshes
  (pley's locked fields, listed in details as `Field` elements with
  `locked="1"`); `field.locked=1` alone keeps the current value,
  `field.locked=0` gives the field back to the metadata agent, and a value
  sent with `field.locked=0` is written but not kept.
- Tags: `genre[0].tag.tag=Horror` adds a genre, `genre[].tag.tag-=Horror,Drama`
  removes (each name percent-encoded once more, as python-plexapi sends
  them). The same for `label`, which details list as `Label` and the
  `label` filter and `/library/sections/{id}/label` find, and for
  `collection`: the item joins or leaves the collection of that name in its
  library, which is made when missing. Details list it as `Collection`.
- Posters and backgrounds: `POST /library/metadata/{id}/posters` (or
  `/arts`) with the picture as the body (25 MB at most, re-encoded), or
  `?url=` and `PUT /library/metadata/{id}/poster?url=` (or `/art`) with a
  TMDB image. Other URLs are refused: the server doesn't fetch arbitrary
  addresses.
- Collections: their title, sort title (kept as set, and listed as
  `titleSort`) and summary through their section (`type=18&id=…`, as
  python-plexapi edits them), their poster and background through
  `POST /library/collections/{id}/posters` (or `/arts`).
- `POST /library/collections?type=1&title=…&sectionId=…&smart=0&uri=server://…/library/metadata/1,2`
  makes a collection of those items; with `smart=1` and
  `uri=server://…/library/sections/{id}/all?genre=…` a smart collection,
  filled by that listing (any filter and sort of `…/all`), as each user may
  see it. Collections report `smart` and, for smart ones, the listing as
  `content`. `PUT /library/collections/{id}/items?uri=…` adds items (or
  gives a smart collection a new listing), `DELETE …/items/{item}` removes
  one and `DELETE /library/collections/{id}` deletes the collection.

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
- tags (`genre`, `label`, `contentRating`), booleans (`unwatched`, `inProgress`), dates
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
  refresh libraries;
- `editTitle()`, `editSummary()` and their `locked` argument, `fields`,
  `addGenre()`/`removeGenre()`, `addLabel()`/`removeLabel()`,
  `search(label=…)`, `listFilterChoices('label')`, `addCollection()`,
  `collections()` and `uploadPoster()`;
- collections' `editSummary()`, `editSortTitle()`, `uploadPoster()`,
  `createCollection()` (also `smart=True`), `filters()`, `updateFilters()`,
  `removeItems()` and `delete()`; `createPlaylist(smart=True)`;
- play queues: `PlayQueue.create()` (from items, a section listing or a
  playlist, with `continuous`), `get()`, `addItem(playNext=True)`,
  `moveItem()`, `removeItem()` and `clear()`.

## Not supported

- plex.tv: sign-in, server claiming, sharing, remote access and the official
  Plex apps.
- Collections' order and mode are set in pley, not through the Plex API.
- Editing episodes' and seasons' artwork from URLs other than TMDB images
  (upload the file instead), and per-item preferences (`/prefs`).
- Filters on descendants (`episode.title` on shows) and on `resolution`, `hdr`,
  audio or subtitle language: ignored, so they don't narrow results.
- Player remote control, photos and live TV.
- Notifications about transcodes (`transcodeSession.*`), deleted items and
  reachability.
- Per-client capability profiles: universal transcode streams are H.264/AAC
  HLS.
