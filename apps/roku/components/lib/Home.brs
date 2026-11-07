' Home's hero and the info block over its rows, as on Android TV and the
' native Samsung app (their HomeState and Home.cs): the titles they show,
' and what they say about them.

' How many titles the hero takes turns showing, as on the other apps.
function homeFeaturedCount() as integer
    return 6
end function

' The titles the hero features, as the web picks them (apps/web/src/lib/
' featured.ts) and the other apps: the newest unwatched movies and shows
' with a backdrop, taken in turn from each library's Recently Added (so
' movies and shows alternate), then from My List when that's too few;
' when everything has been watched, the newest all the same.
' packages/tv/test/fixtures/featured.json pins it. From GET
' /api/hubs/home's hubs, count at most: [{ item, fromList }].
function homeFeatured(hubs as dynamic, count as integer) as object
    picked = homeFeaturedPick(hubs, count, true)
    if picked.Count() = 0 then picked = homeFeaturedPick(hubs, count, false)
    return picked
end function

' homeFeatured's pick, of the unwatched titles only (unwatched) or of all.
function homeFeaturedPick(hubs as dynamic, count as integer, unwatched as boolean) as object
    sources = []
    listed = invalid
    for each hub in httpArray(hubs)
        if GetInterface(hub, "ifAssociativeArray") <> invalid
            key = httpString(hub.key)
            if Left(key, 14) = "recently_added"
                source = []
                for each item in httpArray(hub.items)
                    if homeFeaturable(item, unwatched) then source.Push(item)
                end for
                sources.Push(source)
            else if key = "my_list" and listed = invalid
                listed = httpArray(hub.items)
            end if
        end if
    end for
    picked = []
    seen = {}
    ' The newest of each library, then the next of each, and so on.
    i = 0
    while picked.Count() < count
        more = false
        for each source in sources
            if i < source.Count()
                more = true
                homeFeaturedTake(picked, seen, source[i], false, count)
            end if
        end for
        if not more then exit while
        i = i + 1
    end while
    if listed <> invalid
        for each item in listed
            if homeFeaturable(item, unwatched) then homeFeaturedTake(picked, seen, item, true, count)
        end for
    end if
    return picked
end function

' A movie or a show with a backdrop to fill the hero (and not watched,
' with unwatched).
function homeFeaturable(item as dynamic, unwatched as boolean) as boolean
    if GetInterface(item, "ifAssociativeArray") = invalid then return false
    kind = httpString(item.kind)
    if kind <> "movie" and kind <> "show" then return false
    if httpString(item.backdrop_url) = "" then return false
    if not unwatched then return true
    state = httpObject(item.user_state)
    return state = invalid or not httpTrue(state.watched)
end function

' Puts item among the picked, once, while there's room.
sub homeFeaturedTake(picked as object, seen as object, item as object, fromList as boolean, count as integer)
    key = urlValue(httpNumber(item.id, 0))
    if picked.Count() >= count or seen.DoesExist(key) then return
    seen[key] = true
    picked.Push({ item: item, fromList: fromList })
end sub

' Why a featured title is there, as the other apps say it: "On your list"
' (fromList), else "New in Movies", its library's name (libraryId among
' libraries, [{ id, name }]), or "New" without it.
function homeFeaturedLabel(fromList as boolean, libraryId as dynamic, libraries as dynamic) as string
    if fromList then return t("browse.home.featured.onList")
    id = httpNumber(libraryId, 0)
    if id > 0
        for each library in httpArray(libraries)
            if GetInterface(library, "ifAssociativeArray") <> invalid and httpNumber(library.id, 0) = id
                name = httpString(library.name)
                if name <> "" then return t("browse.home.featured.newIn", { library: name })
            end if
        end for
    end if
    return t("browse.home.featured.new")
end function

' How long a featured title stays in the hero while no key is pressed, as
' on the other apps.
function homeTurnSeconds() as integer
    return 9
end function

' Whether the hero turns to its next title now: it has several (titles),
' its buttons have the focus (focused: not the rows, the rail, a dialog or
' a screen over Home), and the last key (idleSeconds ago, as Roku's
' TimeSinceLastKeypress counts them) and the last turn (shownMs ago) are
' both 9 s old.
function homeTurnsNow(titles as integer, focused as boolean, idleSeconds as integer, shownMs as integer) as boolean
    if titles < 2 or not focused then return false
    return idleSeconds >= homeTurnSeconds() and shownMs >= homeTurnSeconds() * 1000
end function

' The title the hero shows with nothing featured, from GET
' /api/hubs/home's hubs: the first of Continue Watching, else the newest
' video in the rows, else the newest title; invalid with nothing to show.
function homeHeroItem(hubs as dynamic) as dynamic
    for each hub in httpArray(hubs)
        if GetInterface(hub, "ifAssociativeArray") <> invalid and contentContinueWatching(httpString(hub.key))
            for each item in httpArray(hub.items)
                if GetInterface(item, "ifAssociativeArray") <> invalid and contentShown(httpString(item.kind)) then return item
            end for
        end if
    end for
    newest = invalid
    newestVideo = invalid
    for each hub in httpArray(hubs)
        if GetInterface(hub, "ifAssociativeArray") <> invalid
            for each item in httpArray(hub.items)
                if GetInterface(item, "ifAssociativeArray") <> invalid and contentShown(httpString(item.kind))
                    added = httpNumber(item.added_at, 0)
                    if newest = invalid or added > httpNumber(newest.added_at, 0) then newest = item
                    if homeVideo(httpString(item.kind)) and (newestVideo = invalid or added > httpNumber(newestVideo.added_at, 0)) then newestVideo = item
                end if
            end for
        end if
    end for
    if newestVideo <> invalid then return newestVideo
    return newest
end function

' Movies, shows, seasons and episodes: the hero prefers them to music.
function homeVideo(kind as string) as boolean
    return kind = "movie" or kind = "show" or kind = "season" or kind = "episode"
end function

' What the hero or the info block says about item: an ItemSummary from the
' rows, or the ItemDetail once asked for (complete), which alone brings a
' movie's summary, the content rating and the quality badges. { itemId,
' kind, title (a show's name for an episode), context ("S2 · E5 · The Long
' Night"), meta ("2019 · 2h 2m · PG-13 · ★ 7.4 · 4K · HDR"), summary,
' progress (0 to 1), timeLeft ("26 min left" while in progress), video
' (it plays itself), watched, positionMs, durationMs, backdrop, complete }.
function homeInfo(item as object, server as string, complete as boolean) as object
    kind = httpString(item.kind)
    title = httpSentence(item.title)
    context = ""
    if kind = "episode"
        showTitle = httpSentence(item.grandparent_title)
        parts = []
        code = fmtEpisode(item.parent_index, item.index)
        if code <> "" then parts.Push(code)
        if showTitle <> ""
            if title <> "" then parts.Push(title)
            title = showTitle
        end if
        context = parts.Join(" · ")
    end if
    meta = []
    year = httpNumber(item.year, 0)
    if year > 0 then meta.Push(urlValue(year))
    if kind = "show"
        seasons = Int(httpNumber(item.child_count, 0))
        if seasons > 0 then meta.Push(t("browse.counts.seasons", { count: seasons }))
    else if kind = "season"
        episodes = Int(httpNumber(item.leaf_count, 0))
        if episodes > 0 then meta.Push(t("browse.counts.episodes", { count: episodes }))
    else
        duration = fmtDuration(item.duration_ms)
        if duration <> "" then meta.Push(duration)
    end if
    rating = httpSentence(item.content_rating)
    if rating <> "" then meta.Push(rating)
    score = httpNumber(item.rating, 0)
    if score > 0 then meta.Push("★ " + fmtOneDecimal(score))
    if complete
        for each badge in homeBadges(item)
            meta.Push(badge)
        end for
    end if
    state = httpObject(item.user_state)
    position = 0
    watched = false
    if state <> invalid
        position = httpNumber(state.position_ms, 0)
        watched = httpTrue(state.watched)
    end if
    duration = httpNumber(item.duration_ms, 0)
    inProgress = not watched and fmtResumable(position, duration)
    progress = 0.0
    timeLeft = ""
    if inProgress
        progress = fmtProgress(position, duration)
        timeLeft = fmtLeft(position, duration)
    end if
    return {
        itemId: httpNumber(item.id, 0),
        kind: kind,
        title: title,
        context: context,
        meta: meta.Join(" · "),
        summary: httpClean(item.summary, 600, true),
        progress: progress,
        timeLeft: timeLeft,
        video: contentPlayable(kind),
        watched: watched,
        positionMs: position,
        durationMs: duration,
        backdrop: homeBackdrop(item, server),
        complete: complete
    }
end function

' The quality badges of a title's details: its media summary's where the
' server sends one, else its first file's ("4K", "DV").
function homeBadges(item as object) as object
    media = httpObject(item.media)
    if media <> invalid then return contentQualityBadges(media)
    files = httpArray(item.files)
    if files.Count() = 0 then return []
    file = httpObject(files[0])
    if file = invalid then return []
    range = invalid
    for each stream in httpArray(file.streams)
        if GetInterface(stream, "ifAssociativeArray") <> invalid and httpString(stream.kind) = "video" and range = invalid then range = stream.dynamic_range
    end for
    return contentQualityBadges({ width: file.width, height: file.height, dynamic_range: range })
end function

' The backdrop behind a title: its own, else its show's or its season's.
function homeBackdrop(item as object, server as string) as string
    width = homeBackdropWidth()
    url = urlImage(server, item.backdrop_url, width)
    if url <> "" then return url
    for each key in ["grandparent_id", "parent_id"]
        id = httpNumber(item[key], 0)
        if id > 0 then return urlImage(server, "/api/items/" + urlValue(id) + "/images/backdrop", width)
    end for
    return ""
end function

' The hero's backdrop fills most of the screen beside the menu: sharper
' than the faint ones behind a title's details, yet small enough for a
' Roku Express's texture memory.
function homeBackdropWidth() as integer
    return 960
end function

' The hero's buttons: { labels, actions }. A movie or an episode plays
' (Resume and From start while in progress); a show or a season plays its
' next episode ("next"), as on Android TV; anything else opens. Info opens
' the title's page; then My List for a movie or a show (Add to My List,
' or Remove when inList), the watched mark for an episode or a season.
function homeHeroButtons(info as object, inList = false as boolean) as object
    labels = []
    actions = []
    if info.video
        if info.timeLeft <> ""
            labels = [t("common.actions.resume"), t("browse.home.fromStart")]
            actions = ["resume", "start"]
        else
            labels = [t("common.actions.play")]
            actions = ["start"]
        end if
        labels.Push(t("browse.home.info"))
        actions.Push("info")
    else if info.kind = "show" or info.kind = "season"
        labels = [t("common.actions.play"), t("browse.home.info")]
        actions = ["next", "info"]
    else
        labels = [t("common.actions.open")]
        actions = ["info"]
    end if
    if info.kind = "movie" or info.kind = "show"
        if inList
            labels.Push(t("browse.title.removeFromMyList"))
            actions.Push("listRemove")
        else
            labels.Push(t("browse.title.addToMyList"))
            actions.Push("listAdd")
        end if
    else if info.kind = "episode" or info.kind = "season"
        if info.watched
            labels.Push(t("browse.title.markUnwatched"))
            actions.Push("unwatched")
        else
            labels.Push(t("browse.title.markWatched"))
            actions.Push("watched")
        end if
    end if
    return { labels: labels, actions: actions }
end function

' The icon of a rail entry (homeMenu's), as Android TV's rail has them
' (RailItems.kt): its name in images/icons.
function homeRailIcon(entry as object) as string
    icons = {
        search: "search", home: "house", discover: "compass", requests: "list-video", live: "tv-minimal",
        libraries: "layers", playlists: "list-music", collections: "layers", nowPlaying: "audio-lines", drive: "usb", settings: "settings"
    }
    if entry.action <> "library" then return icons[entry.action]
    kind = ""
    if entry.library <> invalid then kind = entry.library.kind
    if kind = "movies" then return "clapperboard"
    if kind = "shows" then return "tv"
    if kind = "music" then return "music"
    return "film"
end function

' How the hero's buttons (homeHeroButtons' actions) look, as on Android
' TV's hero: [{ icon, round }]. The first is a pill with its words, Play's
' icon before Resume or Play (a show's next episode too); the others are
' round, an icon alone (From start, Info, My List's plus or check, the
' watched mark), their words shown under them while focused. icon: its
' name in images/icons, "" for none.
function homeHeroLooks(actions as object) as object
    icons = { start: "rotate-ccw", info: "info", watched: "circle-check", unwatched: "circle-check", listAdd: "plus", listRemove: "check" }
    looks = []
    for i = 0 to actions.Count() - 1
        action = actions[i]
        if i = 0
            icon = ""
            if action = "resume" or action = "start" or action = "next" then icon = "play-fill"
            looks.Push({ icon: icon, round: false })
        else
            icon = icons[action]
            if icon = invalid then icon = ""
            looks.Push({ icon: icon, round: icon <> "" })
        end if
    end for
    return looks
end function

' What Home says with no rows: "No libraries yet" when the profile sees
' none, "Scanning your libraries…" while one scans, else "Nothing here
' yet", as on Android TV. libraries is GET /api/libraries' answer, invalid
' until it comes. { title, text }.
function homeEmpty(libraries as dynamic) as object
    if libraries <> invalid and httpArray(libraries).Count() = 0 then return { title: t("browse.home.noLibraries"), text: t("browse.home.noLibrariesHint") }
    for each library in httpArray(libraries)
        if GetInterface(library, "ifAssociativeArray") <> invalid and httpTrue(library.scanning) then return { title: t("browse.home.scanning"), text: t("browse.home.scanningHint") }
    end for
    return { title: t("browse.home.empty"), text: t("browse.home.emptyHint") }
end function

' Home's menu, in the order of the other TVs' rail: Search, Home, the way
' into requests (requests: its entry, homeRequestsEntries), Live TV where
' the server has it, the libraries (six at most, then Libraries, which
' lists them all, as on Android TV), Playlists, Collections where there is
' a library of movies or shows (the page says when none has any) and
' Libraries isn't, Now Playing while music plays, USB drive while one is
' plugged in (drive), and Settings last.
' [{ title, action, library, count, danger }].
function homeMenu(libraries as object, live as boolean, requests as object, music as boolean, drive = false as boolean) as object
    entries = [{ title: t("browse.menu.search"), action: "search" }, { title: t("browse.menu.home"), action: "home" }]
    for each entry in requests
        entries.Push(entry)
    end for
    if live then entries.Push({ title: t("browse.menu.liveTv"), action: "live" })
    videos = false
    shown = 0
    ' A library's name is the server's, never translated.
    for each library in libraries
        if shown < homeMenuLibraries() then entries.Push({ title: library.name, action: "library", library: library })
        shown = shown + 1
        if library.kind = "movies" or library.kind = "shows" then videos = true
    end for
    many = libraries.Count() > homeMenuLibraries()
    if many then entries.Push({ title: t("browse.menu.libraries"), action: "libraries" })
    entries.Push({ title: t("browse.menu.playlists"), action: "playlists" })
    if videos and not many then entries.Push({ title: t("browse.menu.collections"), action: "collections" })
    if music then entries.Push({ title: t("browse.menu.nowPlaying"), action: "nowPlaying" })
    if drive then entries.Push({ title: t("drive.title"), action: "drive" })
    entries.Push({ title: t("browse.menu.settings"), action: "settings" })
    return entries
end function

' How many libraries the menu lists before Libraries takes their place, as
' on the other TVs' rail.
function homeMenuLibraries() as integer
    return 6
end function

' The menu's one way into requests, as on Android TV's rail, from the
' requests' summary (lib/Requests.brs): Discover where the profile may
' ask, else the requests (Your requests, a manager's Requests) where it
' has some to see, else none; it carries their number (Discover leads to
' them). [] or [{ title, action, count, danger }].
function homeRequestsEntries(summary as object) as object
    entry = invalid
    if reqShowsDiscover(summary)
        entry = { title: t("browse.menu.discover"), action: "discover" }
    else if reqShowsRequests(summary)
        entry = { title: reqPageName(summary), action: "requests" }
    end if
    if entry = invalid then return []
    if reqShowsRequests(summary)
        badge = reqBadge(summary)
        entry.count = badge.count
        entry.danger = badge.danger
    end if
    return [entry]
end function
