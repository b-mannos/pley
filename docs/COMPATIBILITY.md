# Compatibility API

Besides its own API, Pley speaks a widely used media-server API, so tools
written against it work with Pley: scripts using
[`python-plexapi`](https://github.com/pushingkarmaorg/python-plexapi), and
tools built the same way (Tautulli, Kometa, direct-play clients). It answers on
Pley's own port and uses the same libraries, users, watch state and playback
sessions as Pley's own API, so anything done through it shows up in the web
UI and the other way round.

Apps that sign in through their vendor's cloud service and find servers
there are out of scope: Pley has no such service.

## Enabling and port

By default the API answers on Pley's own port (7539), wherever Pley does,
through its reverse proxy's HTTPS too: give tools Pley's address,
`http://pley.local:7539` or `https://media.example.com`. The two share the
port this way:

- a request carrying this API's headers (`X-Plex-Token`,
  `X-Plex-Client-Identifier`, …) or `X-Plex-Token` in its query is this
  API's: its clients send them, browsers never do;
- so is a path only this API has (`/identity`, `/library/sections/…`,
  `/:/timeline`, a stream's `/video/:/transcode/…` segments, which players
  fetch without the headers);
- the rest, `/`, `/search`, `/library` and `/playlists` among them, is the
  web app's, and `/api/…` and `/.well-known/…` are always Pley's own.

Its answers there are sandboxed (`Content-Security-Policy: default-src
'none'; sandbox`, `X-Frame-Options: DENY`, `Cache-Control: private` unless
they say otherwise), since they share the web app's origin, and the paths
both answer vary with `X-Plex-Token` and `X-Plex-Client-Identifier` for
caches.
Settings → Network turns it off, or gives it a port of its own:

| Where | Setting |
|---|---|
| Command line | `--plex-bind web` (the default), `--plex-bind 0.0.0.0:32400` (several addresses separated by commas) or `--plex-bind off` |
| Environment | `PLEY_PLEX_BIND` |
| `pley.toml` | `plex_bind = "web"` |

Its own port suits tools that only know Plex's, 32400. If that port is taken
(another media server uses it on the same machine), Pley logs a warning,
keeps running without it there, and tries again every 30 seconds (Settings
→ Network says why it isn't listening).

## Authentication

Use a Pley session token as the API's token. Get one by signing in:

```sh
curl -s -X POST http://localhost:7539/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"…"}'
# {"user":{…},"token":"<token>"}
```

The token is accepted as the API sends it, the `X-Plex-Token` header or
query parameter, and the Pley way (`Authorization: Bearer`, `X-Pley-Token` or
`?token=`). The web UI's `pley_token` cookie is refused here: browsers send
it to this port too, and some of this API's endpoints change things on a GET.
Tokens of apps signed in on Pley's own page (`pley_at_…`) are refused as
well, and so are the HLS session addresses of playback those apps started:
tools of this API use a token from `POST /api/auth/login`. Every endpoint needs a
token except `/identity`.

```python
from plexapi.server import PlexServer
server = PlexServer('http://localhost:7539', '<token>')
for movie in server.library.section('Movies').search(unwatched=True):
    print(movie.title, movie.media[0].videoResolution)
```

Each user sees their own watch state, Continue Watching and sessions.
Administrators also see every user's sessions and can refresh libraries.

## Formats

Responses are XML `MediaContainer`s. With `Accept: application/json` they
are JSON in the same layout
(`{"MediaContainer": {"Metadata": […]}}`). Paging works through
`X-Plex-Container-Start` and `X-Plex-Container-Size`, as headers or query
parameters; containers report `size`, `totalSize` and `offset`.

## Mapping

- **IDs.** `ratingKey` is the Pley item id and `librarySectionID` the library
  id. Media and parts use the media file id. Stream ids are
  `file id × 10000 + stream index`.
- **Order.** Sections, and the home hubs of each, come in the order
  administrators choose in Pley (Settings → Libraries); new ones come last.
- **Types.** Movies and episodes are `Video` elements, shows, seasons, artists
  and albums are `Directory` elements, and tracks are `Track` elements, with
  the API's `type` values.
- **GUIDs.** Every item gets a `plex://<type>/<id>` guid. With
  `includeGuids=1`, and always in details, `Guid` children carry the ids
  metadata found, in the API's usual form: `imdb://`, `tmdb://` (movies and
  shows), `tvdb://` (shows) and `mbid://` (artists and albums). The `guid`
  filter finds items by any of them.
- **Editions.** Movies whose versions are tagged with an edition
  (`{edition-Director's Cut}` in the file or folder name) carry the default
  version's `editionTitle`.
- **Watch state.** `viewCount` is the play count of watched items and 0 once an
  item is marked unplayed. `viewOffset` is the resume position.
  `/:/scrobble`, `/:/unscrobble`, `/:/progress` and `/:/timeline` use the same
  code as Pley's player.
- **Sessions.** A client that reports `/:/timeline` while playing becomes a
  Pley playback session. It is listed in `/status/sessions` (with `User`,
  `Player` and `Session`, plus `TranscodeSession` for HLS) and in Pley's
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
  a Pley HLS session, remuxed when the video can be copied and transcoded
  otherwise (`directStream=0` forces transcoding). It honors `offset`,
  `maxVideoBitrate`, `videoResolution`, `audioStreamID` and burned-in
  `subtitleStreamID`; without `audioStreamID`, Pley picks the track as for
  its own players (docs/API.md, "The audio track"). The master playlist points at
  `session/<id>/base/index.m3u8`. Session URLs need no token: the 128-bit
  session id is the credential.
- **Preview thumbnails and chapters.** Parts of files with seek previews have
  `indexes="sd"`; the BIF index is cut from Pley's previews. Details list the
  first version's `Chapter`s, with a `thumb` once the file has previews.
- **Markers.** With `includeMarkers=1`, details list the intro and credits
  Pley found in the first version as `Marker`s (`type`, `startTimeOffset`,
  `endTimeOffset` in ms; credits are `final`), for Skip Intro and Skip
  Credits in clients.
- **Cast and crew.** Details carry `Role` (with the part as `role`),
  `Director`, `Writer` and `Producer` tags. Their `id` is the Pley person id and
  `thumb` the TMDB photo.
- **Artwork.** `thumb` and `art` attributes point at
  `/library/metadata/{id}/thumb/{version}` and `/art/{version}`.
  `/photo/:/transcode?url=…&width=…` resizes them through Pley's image cache.

## Endpoints

| Area | Endpoints |
|---|---|
| Server | `/`, `/identity`, `/servers`, `/accounts`, `/myplex/account`, `/system`, `/prefs`, `/:/prefs`, `/media/providers`, `/:/websockets/notifications` |
| Sections | `/library`, `/library/sections`, `/library/sections/{id}`, `…/all`, `…/albums`, `…/recentlyAdded`, `…/onDeck`, `…/search`, `…/firstCharacter`, `…/collections`, `…/{genre,year,decade,contentRating,studio,actor,director,writer,producer}`, `…/refresh`, `/library/sections/all/refresh` |
| Items | `/library/metadata/{id}` (comma-separated ids allowed), `…/extras` (trailers and other bonus material as `clip`s with the API's `extraType` and `subtype`), `…/children`, `…/allLeaves`, `…/grandchildren`, `…/related` ("More Like This" hub), `…/similar` (the same items), `…/thumb`, `…/art`, `…/refresh` |
| Lists | `/library/all`, `/library/onDeck`, `/library/recentlyAdded`, `/hubs`, `/hubs/sections/{id}`, `/hubs/continueWatching/items`, `/hubs/home/recentlyAdded`, `/hubs/search`, `/search`, `PUT /actions/removeFromContinueWatching?ratingKey=` (the movie, or the episode's show, leaves the user's Continue Watching until it is played again, as `DELETE /api/continue-watching/{id}`) |
| Collections | `/library/sections/{id}/collections` (also `…/all?type=18`), `/library/collections/{id}`, `…/children`, `…/{thumb,art,composite}[/{version}]` |
| Playlists | `GET`/`POST /playlists` (`?type=&title=&uri=…/library/metadata/1,2`, or `smart=1&uri=…/library/sections/{id}/all?…` for a smart playlist of that listing; `?smart=0|1` filters the list), `GET`/`PUT` (`?title=`)/`DELETE /playlists/{id}`, `GET`/`PUT /playlists/{id}/items` (`?uri=`), `DELETE /playlists/{id}/items/{playlistItemID}`, `PUT …/{playlistItemID}/move?after=`, `/playlists/{id}/composite/{version}` |
| Previews | `/library/parts/{id}/indexes/sd` (BIF), `/library/parts/{id}/indexes/sd/{offset}` (the frame at `offset` ms), `/library/media/{id}/chapterImages/{n}` |
| Playback | `/library/parts/…`, `/library/streams/{id}` (subtitles), `/:/timeline`, `/:/progress`, `/:/scrobble`, `/:/unscrobble`, `/status/sessions`, `/status/sessions/terminate?sessionId=&reason=` (administrators: stops the session whose `Session` `id` it is, `reason` as the message its player shows, as `DELETE /api/sessions/{id}`; that client's reports don't start it again), `/transcode/sessions`, `/{video,audio}/:/transcode/universal/{start.m3u8,decision,stop,ping,session/…}` |
| Play queues | `POST /playQueues` (`uri=` items, a container, a section listing or a playlist, or `playlistID=`; `key=` the item to start with; `shuffle=1`; `continuous=1` plays the rest of an episode's show after it), `GET`/`PUT /playQueues/{id}` (`uri=`/`playlistID=` adds, `next=1` right after the playing item), `DELETE /playQueues/{id}/items[/{playQueueItemID}]`, `PUT …/items/{playQueueItemID}/move?after=`, `PUT /playQueues/{id}/shuffle`, `…/unshuffle`; kept in memory for a day, for the user who made them |
| History | `/status/sessions/history/all` (`accountID`, `librarySectionID`, `metadataItemID`, `viewedAt>=`; administrators see every account, others their own), `DELETE /status/sessions/history/{id}` |
| Images | `/photo/:/transcode` |

## Editing

Administrators edit metadata the way `python-plexapi` (and so Kometa) does:

- `PUT /library/sections/{id}/all?type=1&id=1,2&title.value=…&title.locked=1`,
  or `PUT /library/metadata/{id}?…`, for `title`, `titleSort`,
  `originalTitle`, `year`, `summary`, `tagline`, `studio`, `contentRating`
  and `originallyAvailableAt`. Edited fields are kept through refreshes
  (Pley's locked fields, listed in details as `Field` elements with
  `locked="1"`); `field.locked=1` alone keeps the current value,
  `field.locked=0` gives the field back to the metadata agent, and a value
  sent with `field.locked=0` is written but not kept.
- Tags: `genre[0].tag.tag=Horror` adds a genre, `genre[].tag.tag-=Horror,Drama`
  removes (each name percent-encoded once more, as `python-plexapi` sends
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
  `python-plexapi` edits them), their poster and background through
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
comma-separated), `limit` and the API's filters with their operators:

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
and fields in a `Meta` element; `python-plexapi` validates searches against it.

## Tested

With `python-plexapi` 4.18 the following work against the test media:

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

- The vendor's cloud service: its sign-in, server claiming, sharing and
  remote access, and the apps that depend on it.
- Collections' order and mode are set in Pley, not through this API.
- Editing episodes' and seasons' artwork from URLs other than TMDB images
  (upload the file instead), and per-item preferences (`/prefs`).
- Filters on descendants (`episode.title` on shows) and on `resolution`, `hdr`,
  audio or subtitle language: ignored, so they don't narrow results.
- Player remote control, photos and live TV.
- Notifications about transcodes (`transcodeSession.*`), deleted items and
  reachability.
- Per-client capability profiles: universal transcode streams are H.264/AAC
  HLS.
