' What a title's page, a person's and a collection's show besides the
' title itself (docs/API.md): cast and crew, extras, More Like This, the
' collections it is part of, playlists, and what the Options key offers
' on a tile.
' Plain fields, which the API task makes into ContentNodes (lib/Nodes.brs).

' A person as a tile: their photo, their name, and the part they played
' or what they did (subtitle).
function pagePersonTile(person as object, server as string, subtitle as string) as object
    return {
        title: httpSentence(person.name),
        subtitle: subtitle,
        image: urlImage(server, person.photo_url, contentPosterWidth()),
        wide: false,
        progress: 0.0,
        watched: false,
        itemId: httpNumber(person.id, 0),
        kind: "person"
    }
end function

' What a crew member did: "Director", "Creator"…; "" for an actor.
function pageRoleLabel(role as string) as string
    labels = { creator: "Creator", director: "Director", writer: "Writer", producer: "Producer" }
    if labels.DoesExist(role) then return labels[role]
    return ""
end function

' Cast and crew of a title, as tiles: the actors in billing order with
' the part they played, then creators, directors, writers and producers,
' someone credited twice once with both ("Director, Writer"). At most 30.
function pageCredits(detail as object, server as string) as object
    tiles = []
    seen = {}
    for each credit in httpArray(detail.cast)
        id = 0
        if GetInterface(credit, "ifAssociativeArray") <> invalid then id = httpNumber(credit.id, 0)
        key = urlValue(id)
        if id > 0 and not seen.DoesExist(key) and tiles.Count() < 30
            seen[key] = tiles.Count()
            tiles.Push(pagePersonTile(credit, server, httpSentence(credit.character)))
        end if
    end for
    for each credit in httpArray(detail.crew)
        id = 0
        if GetInterface(credit, "ifAssociativeArray") <> invalid then id = httpNumber(credit.id, 0)
        key = urlValue(id)
        role = pageRoleLabel(httpString(credit.role))
        if id > 0 and role <> ""
            if seen.DoesExist(key)
                tile = tiles[seen[key]]
                if tile.subtitle = ""
                    tile.subtitle = role
                else if Instr(1, tile.subtitle, role) = 0
                    tile.subtitle = tile.subtitle + ", " + role
                end if
            else if tiles.Count() < 30
                seen[key] = tiles.Count()
                tiles.Push(pagePersonTile(credit, server, role))
            end if
        end if
    end for
    return tiles
end function

' What an extra is: "Trailer", "Behind the scenes"…
function pageExtraLabel(extraType as dynamic) as string
    labels = { trailer: "Trailer", featurette: "Featurette", behind_the_scenes: "Behind the scenes", deleted_scene: "Deleted scene", interview: "Interview", scene: "Scene", short: "Short" }
    kind = httpString(extraType)
    if labels.DoesExist(kind) then return labels[kind]
    return "Extra"
end function

' A title's extras as 16:9 tiles: their name, and what they are.
function pageExtras(detail as object, server as string) as object
    tiles = []
    for each extra in httpArray(detail.extras)
        if GetInterface(extra, "ifAssociativeArray") <> invalid
            tile = contentTile(extra, server, true)
            tile.kind = "extra"
            tile.subtitle = pageExtraLabel(extra.extra_type)
            duration = fmtDuration(extra.duration_ms)
            if duration <> "" then tile.subtitle = tile.subtitle + " · " + duration
            tiles.Push(tile)
        end if
    end for
    return tiles
end function

' The rows under a title's details that come with it: [{ key, title,
' wide, tiles }], cast and crew then extras, rows without tiles left out.
function pageDetailRows(detail as object, server as string) as object
    rows = []
    credits = pageCredits(detail, server)
    if credits.Count() > 0 then rows.Push({ key: "credits", title: "Cast & crew", wide: false, tiles: credits })
    extras = pageExtras(detail, server)
    if extras.Count() > 0 then rows.Push({ key: "extras", title: "Extras", wide: true, tiles: extras })
    return rows
end function

' A collection as a tile: its poster (the server's own picture of it when
' the answer names none), its name and how many titles it holds.
function pageCollectionTile(collection as object, server as string) as object
    id = httpNumber(collection.id, 0)
    image = urlImage(server, collection.poster_url, contentPosterWidth())
    if image = "" then image = urlImage(server, "/api/collections/" + urlValue(id) + "/images/poster", contentPosterWidth())
    subtitle = ""
    count = httpNumber(collection.item_count, -1)
    if count = 1
        subtitle = "1 title"
    else if count >= 0
        subtitle = urlValue(count) + " titles"
    end if
    return { title: httpSentence(collection.title), subtitle: subtitle, image: image, wide: false, progress: 0.0, watched: false, itemId: id, kind: "collection" }
end function

function pageCollectionTiles(collections as dynamic, server as string) as object
    tiles = []
    for each collection in httpArray(collections)
        if GetInterface(collection, "ifAssociativeArray") <> invalid and httpNumber(collection.id, 0) > 0 then tiles.Push(pageCollectionTile(collection, server))
    end for
    return tiles
end function

' A show's seasons as poster tiles: "Season 1", and its episodes.
function pageSeasonTiles(children as dynamic, server as string) as object
    tiles = []
    for each item in httpArray(children)
        if GetInterface(item, "ifAssociativeArray") <> invalid and httpString(item.kind) = "season"
            tile = contentTile(item, server, false)
            seasons = contentSeasons([item])
            tile.title = seasons[0].title
            episodes = seasons[0].episodes
            tile.subtitle = ""
            if episodes = 1
                tile.subtitle = "1 episode"
            else if episodes > 1
                tile.subtitle = urlValue(episodes) + " episodes"
            end if
            tiles.Push(tile)
        end if
    end for
    return tiles
end function

' A person's page: their movies and shows, newest first, each with the
' part they played or what they did ("Director, Writer"; "12 episodes" of
' a show when the server counts them).
function pagePersonCredits(person as object, server as string) as object
    tiles = []
    for each credit in httpArray(person.credits)
        item = invalid
        if GetInterface(credit, "ifAssociativeArray") <> invalid then item = httpObject(credit.item)
        if item <> invalid and contentShown(httpString(item.kind))
            tile = contentTile(item, server, false)
            parts = []
            for each role in httpArray(credit.roles)
                if GetInterface(role, "ifAssociativeArray") <> invalid
                    text = httpSentence(role.character)
                    if text = "" then text = pageRoleLabel(httpString(role.role))
                    if text <> "" and parts.Count() < 2 then parts.Push(text)
                end if
            end for
            if parts.Count() > 0 then tile.subtitle = parts.Join(", ")
            tiles.Push(tile)
        end if
    end for
    return tiles
end function

' What the Options key offers on a tile: [{ label, action }]. inList: the
' title is in My List (true or false), or invalid when not known; a
' Continue Watching tile can be taken out of it. Kinds without actions
' get none.
function pageTileOptions(tile as object, continueWatching as boolean, inList as dynamic) as object
    kind = httpString(tile.kind)
    options = []
    ' Music has no watched marks or My List.
    if contentMusic(kind) then return [{ label: "Add to playlist", action: "playlist" }]
    if kind <> "movie" and kind <> "show" and kind <> "season" and kind <> "episode" then return options
    if httpTrue(tile.watched)
        options.Push({ label: "Mark as unwatched", action: "unwatched" })
    else
        options.Push({ label: "Mark as watched", action: "watched" })
    end if
    if (kind = "movie" or kind = "show") and GetInterface(inList, "ifBoolean") <> invalid
        if inList
            options.Push({ label: "Remove from My List", action: "listRemove" })
        else
            options.Push({ label: "Add to My List", action: "listAdd" })
        end if
    end if
    options.Push({ label: "Add to playlist", action: "playlist" })
    if continueWatching then options.Push({ label: "Remove from Continue Watching", action: "hide" })
    if kind = "episode" and httpNumber(tile.grandparentId, 0) > 0 then options.Push({ label: "Go to show", action: "show" })
    return options
end function

' --- Playlists ---

' "12 titles · 3 h 20 min", "Smart · 40 titles": what a playlist holds.
function pagePlaylistLine(playlist as object) as string
    parts = []
    if httpString(playlist.smart_filter) <> "" then parts.Push("Smart")
    count = httpNumber(playlist.item_count, 0)
    noun = " titles"
    if httpString(playlist.kind) = "audio" then noun = " tracks"
    if count = 1 then noun = Left(noun, Len(noun) - 1)
    parts.Push(urlValue(count) + noun)
    duration = fmtDuration(playlist.duration_ms)
    if duration <> "" then parts.Push(duration)
    return parts.Join(" · ")
end function

' A playlist as a tile: its first entry's picture, its name and what it
' holds.
function pagePlaylistTile(playlist as object, server as string) as object
    image = ""
    artwork = httpArray(playlist.artwork)
    if artwork.Count() > 0 then image = urlImage(server, artwork[0], contentPosterWidth())
    return { title: httpSentence(playlist.title), subtitle: pagePlaylistLine(playlist), image: image, wide: false, progress: 0.0, watched: false, itemId: httpNumber(playlist.id, 0), kind: "playlist" }
end function

function pagePlaylistTiles(playlists as dynamic, server as string) as object
    tiles = []
    for each playlist in httpArray(playlists)
        if GetInterface(playlist, "ifAssociativeArray") <> invalid and httpNumber(playlist.id, 0) > 0 then tiles.Push(pagePlaylistTile(playlist, server))
    end for
    return tiles
end function

' The playlists a title can be added to: the profile's own of its kind
' ("video" or "audio"), smart ones left out: [{ id, title }].
function pagePlaylistChoices(playlists as dynamic, kind as string) as object
    out = []
    for each playlist in httpArray(playlists)
        if GetInterface(playlist, "ifAssociativeArray") <> invalid and httpString(playlist.kind) = kind and httpString(playlist.smart_filter) = ""
            out.Push({ id: httpNumber(playlist.id, 0), title: httpSentence(playlist.title) })
        end if
    end for
    return out
end function

' The kind of playlist a title goes in: "audio" for music, else "video".
function pagePlaylistKind(kind as string) as string
    if kind = "artist" or kind = "album" or kind = "track" then return "audio"
    return "video"
end function

' A playlist's entries as rows (EpisodeRow): a movie's title and year, an
' episode's show and number, with entryId, the entry's own id.
function pagePlaylistEntries(entries as dynamic, server as string) as object
    rows = []
    for each entry in httpArray(entries)
        item = invalid
        if GetInterface(entry, "ifAssociativeArray") <> invalid then item = httpObject(entry.item)
        if item <> invalid
            tile = contentTile(item, server, true)
            parts = []
            if tile.subtitle <> "" then parts.Push(tile.subtitle)
            duration = fmtDuration(tile.durationMs)
            if tile.progress > 0
                parts.Push(fmtLeft(tile.positionMs, tile.durationMs))
            else if duration <> ""
                parts.Push(duration)
            end if
            if tile.watched then parts.Push("Watched")
            tile.subtitle = parts.Join(" · ")
            tile.summary = ""
            tile.entryId = httpNumber(entry.entry_id, 0)
            rows.Push(tile)
        end if
    end for
    return rows
end function

' What the player plays one after the other from a playlist: [{ itemId,
' title, line, image, startMs }], the Up Next cards of its entries.
function pagePlaylistQueue(entries as dynamic, server as string) as object
    queue = []
    for each entry in httpArray(entries)
        item = invalid
        if GetInterface(entry, "ifAssociativeArray") <> invalid then item = httpObject(entry.item)
        if item <> invalid and contentPlayable(httpString(item.kind)) then queue.Push(contentUpNext(item, server))
    end for
    return queue
end function

' Why a playlist didn't change, in a sentence: a sign-in from before
' pley for Roku asked to change playlists can't (403), until signed in
' again.
function pagePlaylistRefusal(answer as object) as string
    if answer.status = 403 then return "This sign-in can't change playlists: sign out in Settings and sign in again to allow it."
    return answer.message
end function

' queue from its entry at, in order or shuffled (the entry at first).
function pageQueueFrom(queue as object, at as integer, shuffled as boolean) as object
    out = []
    for i = at to queue.Count() - 1
        out.Push(queue[i])
    end for
    if shuffled
        for i = 0 to at - 1
            out.Push(queue[i])
        end for
        for i = out.Count() - 1 to 1 step -1
            j = Rnd(i + 1) - 1
            other = out[i]
            out[i] = out[j]
            out[j] = other
        end for
    end if
    return out
end function

' The request an action of the Options key or a title's page makes:
' { method, path }, or invalid for one that only goes somewhere.
function pageActionRequest(action as string, itemId as dynamic) as dynamic
    id = urlValue(itemId)
    if action = "watched" then return { method: "POST", path: "/api/items/" + id + "/watched" }
    if action = "unwatched" then return { method: "POST", path: "/api/items/" + id + "/unwatched" }
    if action = "listAdd" then return { method: "PUT", path: "/api/watchlist/" + id }
    if action = "listRemove" then return { method: "DELETE", path: "/api/watchlist/" + id }
    if action = "hide" then return { method: "DELETE", path: "/api/continue-watching/" + id }
    return invalid
end function

' My List after an action: ids (numbers) with itemId added or taken out.
function pageListWith(ids as dynamic, itemId as dynamic, inList as boolean) as object
    out = []
    id = httpNumber(itemId, 0)
    if inList then out.Push(id)
    for each other in httpArray(ids)
        if other <> id then out.Push(other)
    end for
    return out
end function

' Whether ids (My List as Home last saw it) holds itemId; invalid when Home
' hasn't said.
function pageInList(ids as dynamic, itemId as dynamic) as dynamic
    if GetInterface(ids, "ifArray") = invalid then return invalid
    id = httpNumber(itemId, 0)
    for each other in ids
        if other = id then return true
    end for
    return false
end function
