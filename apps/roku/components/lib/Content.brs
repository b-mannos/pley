' The server's JSON as the screens show it: plain fields, which become
' ContentNodes (lib/Nodes.brs). This version shows movies and TV shows;
' music libraries and their titles are left out.

function contentShown(kind as string) as boolean
    return kind = "movie" or kind = "show" or kind = "season" or kind = "episode"
end function

function contentPlayable(kind as string) as boolean
    return kind = "movie" or kind = "episode"
end function

' Picture widths asked of the server, those of the cells that show them
' (the server resizes to buckets): small pictures load fast and keep a
' Roku Express's texture memory free.
function contentPosterWidth() as integer
    return 220
end function

function contentWideWidth() as integer
    return 400
end function

function contentEpisodeWidth() as integer
    return 300
end function

' Backdrops are drawn faint (15 % and 40 %) behind text: a third of the
' screen's width is plenty.
function contentBackdropWidth() as integer
    return 640
end function

' The poster beside a title's details.
function contentDetailPosterWidth() as integer
    return 420
end function

' One tile of a row or a grid. wide: a 16:9 picture (Continue Watching,
' episodes) instead of a poster.
function contentTile(item as object, server as string, wide as boolean) as object
    kind = httpString(item.kind)
    title = httpSentence(item.title)
    subtitle = ""
    if kind = "movie" or kind = "show"
        year = httpNumber(item.year, 0)
        if year > 0 then subtitle = Int(year).ToStr()
    else if kind = "season"
        subtitle = httpSentence(item.parent_title)
    else if kind = "episode"
        show = httpSentence(item.grandparent_title)
        code = fmtEpisode(item.parent_index, item.index)
        parts = []
        if code <> "" then parts.Push(code)
        if title <> "" then parts.Push(title)
        if show <> ""
            subtitle = parts.Join(" · ")
            title = show
        else
            subtitle = code
        end if
    end if
    state = httpObject(item.user_state)
    position = 0
    watched = false
    if state <> invalid
        position = httpNumber(state.position_ms, 0)
        watched = httpTrue(state.watched)
    end if
    duration = httpNumber(item.duration_ms, 0)
    progress = 0.0
    if not watched then progress = fmtProgress(position, duration)
    return {
        title: title,
        subtitle: subtitle,
        image: contentTileImage(item, server, wide, 0),
        wide: wide,
        progress: progress,
        watched: watched,
        itemId: httpNumber(item.id, 0),
        kind: kind,
        parentId: httpNumber(item.parent_id, 0),
        grandparentId: httpNumber(item.grandparent_id, 0),
        positionMs: position,
        durationMs: duration
    }
end function

' The picture of a tile, width pixels wide (0: the tile's own size): for
' wide tiles an episode's still or a backdrop, else the poster. Seasons
' and episodes without a poster of their own ask for theirs anyway: the
' server answers with the show's.
function contentTileImage(item as object, server as string, wide as boolean, width as integer) as string
    kind = httpString(item.kind)
    if wide
        if width = 0 then width = contentWideWidth()
        for each key in ["thumb_url", "backdrop_url", "poster_url"]
            url = urlImage(server, item[key], width)
            if url <> "" then return url
        end for
        return ""
    end if
    if width = 0 then width = contentPosterWidth()
    url = urlImage(server, item.poster_url, width)
    if url = "" and (kind = "season" or kind = "episode")
        url = urlImage(server, "/api/items/" + urlValue(item.id) + "/images/poster", width)
    end if
    return url
end function

' The libraries this version opens, in the server's order:
' [{ id, name, kind, itemCount }]. (A key named count would hide the
' array's Count().)
function contentLibraries(libraries as dynamic) as object
    out = []
    for each library in httpArray(libraries)
        kind = httpString(library.kind)
        if kind = "movies" or kind = "shows"
            out.Push({ id: httpNumber(library.id, 0), name: httpSentence(library.name), kind: kind, itemCount: httpNumber(library.item_count, 0) })
        end if
    end for
    return out
end function

' Home's rows from GET /api/hubs/home: [{ key, title, wide, tiles }].
' Titles of kinds this version doesn't show are left out, and rows left
' empty with them (a music library's); libraries, when given, drop music
' libraries' hubs outright.
function contentHomeRows(hubs as dynamic, libraries as dynamic, server as string) as object
    music = {}
    for each library in httpArray(libraries)
        if httpString(library.kind) = "music" then music[urlValue(library.id)] = true
    end for
    rows = []
    for each hub in httpArray(hubs)
        libraryId = urlValue(hub.library_id)
        if libraryId = "" or not music.DoesExist(libraryId)
            key = httpString(hub.key)
            wide = key = "continue_watching"
            items = []
            for each item in httpArray(hub.items)
                if GetInterface(item, "ifAssociativeArray") <> invalid and contentShown(httpString(item.kind)) then items.Push(contentTile(item, server, wide))
            end for
            if items.Count() > 0 then rows.Push({ key: key, title: httpSentence(hub.title), wide: wide, tiles: items })
        end if
    end for
    return rows
end function

' A short fingerprint of a hub as the server sent it.
function contentSignature(value as object) as string
    bytes = CreateObject("roByteArray")
    bytes.FromAsciiString(FormatJson(value))
    return bytes.GetCRC32().ToStr() + ":" + bytes.Count().ToStr()
end function

' Whether Home asks for its rows again as it shows: after something played
' here, or when they are a minute old, for what the person's other devices
' played or marked meanwhile. Roku's HTTP can't read the server's event
' stream as it comes; rows that didn't change cost
' the server's answer only.
function contentHomeStale(played as integer, playedSeen as integer, seconds as integer) as boolean
    return played <> playedSeen or seconds >= 60
end function

' Home's rows from GET /api/hubs/home (as contentHomeRows), made again only
' where they changed: known holds the fingerprint of each row Home shows
' ({ key: signature }), taken from the server's JSON, so a hub sent as
' before isn't even read. { summary: [{ key, wide, sig, built }] for every
' row in order, changed: the rows to build, in order }.
function contentRowChanges(hubs as dynamic, libraries as dynamic, server as string, known as dynamic) as object
    shown = httpObject(known)
    summary = []
    changed = []
    for each hub in httpArray(hubs)
        if GetInterface(hub, "ifAssociativeArray") <> invalid
            key = httpString(hub.key)
            sig = contentSignature(hub)
            if shown <> invalid and shown.DoesExist(key) and shown[key] = sig
                summary.Push({ key: key, wide: key = "continue_watching", sig: sig, built: false })
            else
                for each row in contentHomeRows([hub], libraries, server)
                    summary.Push({ key: row.key, wide: row.wide, sig: sig, built: true })
                    changed.Push(row)
                end for
            end if
        end if
    end for
    return { summary: summary, changed: changed }
end function

' Tiles of the titles in a listing (a library page, search) this version
' shows.
function contentTiles(items as dynamic, server as string) as object
    tiles = []
    for each item in httpArray(items)
        if GetInterface(item, "ifAssociativeArray") <> invalid and contentShown(httpString(item.kind)) then tiles.Push(contentTile(item, server, false))
    end for
    return tiles
end function

' A show's seasons: [{ itemId, title, index, episodes, watchedEpisodes }].
function contentSeasons(children as dynamic) as object
    seasons = []
    for each item in httpArray(children)
        if GetInterface(item, "ifAssociativeArray") <> invalid and httpString(item.kind) = "season"
            title = httpSentence(item.title)
            index = httpNumber(item.index, -1)
            if title = ""
                if index = 0
                    title = "Specials"
                else
                    title = "Season " + urlValue(index)
                end if
            end if
            seasons.Push({ itemId: httpNumber(item.id, 0), title: title, index: index, episodes: httpNumber(item.leaf_count, 0), watchedEpisodes: httpNumber(item.viewed_leaf_count, 0) })
        end if
    end for
    return seasons
end function

' A season's episodes, as rows: { itemId, kind, title ("1. Pilot"),
' subtitle, summary, image, progress, watched, positionMs, durationMs,
' parentId, grandparentId }.
function contentEpisodes(children as dynamic, server as string) as object
    episodes = []
    for each item in httpArray(children)
        if GetInterface(item, "ifAssociativeArray") <> invalid and httpString(item.kind) = "episode"
            tile = contentTile(item, server, true)
            tile.image = contentTileImage(item, server, true, contentEpisodeWidth())
            title = httpSentence(item.title)
            index = httpNumber(item.index, -1)
            if index >= 0 then title = urlValue(index) + ". " + title
            tile.title = title
            parts = []
            duration = fmtDuration(tile.durationMs)
            if tile.progress > 0
                parts.Push(fmtLeft(tile.positionMs, tile.durationMs))
            else if duration <> ""
                parts.Push(duration)
            end if
            if tile.watched then parts.Push("Watched")
            tile.subtitle = parts.Join(" · ")
            tile.summary = httpClean(item.summary, 300, false)
            episodes.Push(tile)
        end if
    end for
    return episodes
end function

' A movie's or an episode's details screen: what it shows and which
' buttons it has. actions match buttons: "resume", "start".
function contentDetail(detail as object, server as string) as object
    kind = httpString(detail.kind)
    title = httpSentence(detail.title)
    heading = title
    line = fmtMeta(detail)
    if kind = "episode"
        show = httpSentence(detail.grandparent_title)
        code = fmtEpisode(detail.parent_index, detail.index)
        if show <> "" then heading = show
        parts = []
        if code <> "" then parts.Push(code)
        if show <> "" then parts.Push(title)
        meta = fmtMeta(detail)
        if meta <> "" then parts.Push(meta)
        line = parts.Join(" · ")
    end if
    tagline = httpSentence(detail.tagline)
    state = httpObject(detail.user_state)
    position = 0
    if state <> invalid then position = httpNumber(state.position_ms, 0)
    duration = httpNumber(detail.duration_ms, 0)
    files = httpArray(detail.files)
    buttons = []
    actions = []
    note = ""
    if not contentPlayable(kind)
        note = "pley for Roku doesn't play this kind of title yet."
    else if files.Count() = 0
        note = "This item has no file to play."
    else if fmtResumable(position, duration)
        buttons = [fmtResume(position), "Play from start"]
        actions = ["resume", "start"]
    else
        buttons = ["Play"]
        actions = ["start"]
    end if
    backdrop = urlImage(server, detail.backdrop_url, contentBackdropWidth())
    if backdrop = "" then backdrop = urlImage(server, detail.thumb_url, contentBackdropWidth())
    genres = []
    for each genre in httpArray(detail.genres)
        name = httpSentence(genre)
        if name <> "" and genres.Count() < 3 then genres.Push(name)
    end for
    return {
        itemId: httpNumber(detail.id, 0),
        kind: kind,
        heading: heading,
        line: line,
        tagline: tagline,
        genres: genres.Join(", "),
        summary: httpClean(detail.summary, 1200, true),
        backdrop: backdrop,
        poster: contentTileImage(detail, server, false, contentDetailPosterWidth()),
        positionMs: position,
        durationMs: duration,
        buttons: buttons,
        actions: actions,
        note: note
    }
end function

' The file a playback plays: the one with fileId, else the first.
function contentFile(detail as object, fileId as dynamic) as dynamic
    files = httpArray(detail.files)
    wanted = httpNumber(fileId, -1)
    for each file in files
        if GetInterface(file, "ifAssociativeArray") <> invalid and httpNumber(file.id, -2) = wanted then return file
    end for
    if files.Count() > 0 then return files[0]
    return invalid
end function

' The Video node's streamFormat for a PlaybackInfo: HLS for remuxes and
' transcodes, else the original file's container.
function contentStreamFormat(info as object, detail as object) as string
    if httpTrue(info.hls) then return "hls"
    file = contentFile(detail, info.file_id)
    container = ""
    if file <> invalid then container = LCase(httpString(file.container))
    if container = "mkv" or container = "webm" then return "mkv"
    if container = "mp3" then return "mp3"
    return "mp4"
end function

' What the Video node opens for a PlaybackInfo: { url, streamFormat,
' title, playStart, subtitleTracks, subtitle }. subtitle is the TrackName
' of the subtitles the person's choices pick ("" for none); the server
' sends text tracks as WebVTT.
function contentStream(info as object, detail as object, server as string, startMs as dynamic) as object
    url = httpString(info.url)
    if Left(url, 1) = "/" then url = server + url
    file = contentFile(detail, info.file_id)
    selected = -1
    if file <> invalid then selected = httpNumber(file.selected_subtitle, -1)
    tracks = []
    subtitle = ""
    for each track in httpArray(info.subtitles)
        trackUrl = httpString(track.url)
        if trackUrl <> ""
            if Left(trackUrl, 1) = "/" then trackUrl = server + trackUrl
            language = httpString(track.language)
            if language = "" then language = "und"
            tracks.Push({ Language: language, TrackName: trackUrl, Description: contentSubtitleName(track, language) })
            if httpNumber(track.index, -2) = selected then subtitle = trackUrl
        end if
    end for
    return {
        url: url,
        streamFormat: contentStreamFormat(info, detail),
        title: contentPlayerTitle(detail),
        playStart: Int(httpNumber(startMs, 0) / 1000),
        subtitleTracks: tracks,
        subtitle: subtitle
    }
end function

' What Roku's subtitle menu calls a track: its title, else its language;
' "Unknown" for neither, "Forced" for forced ones of no language.
function contentSubtitleName(track as object, language as string) as string
    name = httpSentence(track.title)
    if name = "" and language <> "und" then name = language
    forced = httpTrue(track.forced)
    if name = ""
        if forced then return "Forced"
        return "Unknown"
    end if
    if forced then return name + " (Forced)"
    return name
end function

' "Show · S1 E2 · Pilot" for an episode, the title for a movie.
function contentPlayerTitle(detail as object) as string
    title = httpSentence(detail.title)
    if httpString(detail.kind) <> "episode" then return title
    parts = []
    show = httpSentence(detail.grandparent_title)
    if show <> "" then parts.Push(show)
    code = fmtEpisode(detail.parent_index, detail.index)
    if code <> "" then parts.Push(code)
    if title <> "" then parts.Push(title)
    return parts.Join(" · ")
end function

' Up Next's card for the next episode: { itemId, title, line, image,
' startMs }.
function contentUpNext(item as object, server as string) as object
    tile = contentTile(item, server, true)
    line = fmtEpisode(item.parent_index, item.index)
    title = httpSentence(item.title)
    if title <> ""
        if line <> "" then line = line + " · "
        line = line + title
    end if
    return { itemId: tile.itemId, title: tile.title, line: line, image: tile.image, startMs: playerStartOf(tile.positionMs, tile.durationMs) }
end function
