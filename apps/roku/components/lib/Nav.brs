' Where the screens go: their names, and the screen a tile opens.

' The component of a screen by its name in a navigate request, or "".
function navScreen(name as string) as string
    screens = {
        servers: "ServersScreen",
        link: "LinkScreen",
        profiles: "ProfilesScreen",
        home: "HomeScreen",
        library: "LibraryScreen",
        show: "ShowScreen",
        details: "DetailsScreen",
        grid: "GridScreen",
        album: "AlbumScreen",
        nowPlaying: "NowPlayingScreen",
        playlist: "PlaylistScreen",
        player: "PlayerScreen",
        search: "SearchScreen",
        settings: "SettingsScreen",
        history: "HistoryScreen",
        sortLibraries: "SortLibrariesScreen",
        unreachable: "UnreachableScreen"
    }
    if not screens.DoesExist(name) then return ""
    return screens[name]
end function

' The screen a tile opens: { to, params }, or invalid for kinds this
' version doesn't open. A season opens its show on that season; a person
' and a collection, a grid of their titles, an artist of their albums; a
' playlist, its entries; a track, its album; an extra plays at once.
function navForTile(tile as object) as dynamic
    kind = httpString(tile.kind)
    id = httpNumber(tile.itemId, 0)
    if id = 0 then return invalid
    if kind = "movie" or kind = "episode" or kind = "show" then return { to: "details", params: { itemId: id } }
    if kind = "season"
        showId = httpNumber(tile.parentId, 0)
        if showId = 0 then return invalid
        return { to: "show", params: { showId: showId, seasonId: id } }
    end if
    if kind = "person" or kind = "collection" or kind = "artist" then return { to: "grid", params: { kind: kind, id: id, title: httpString(tile.title) } }
    if kind = "album" then return { to: "album", params: { id: id, title: httpString(tile.title) } }
    if kind = "track"
        albumId = httpNumber(tile.parentId, 0)
        if albumId = 0 then return invalid
        return { to: "album", params: { id: albumId, title: "", trackId: id } }
    end if
    if kind = "playlist" then return { to: "playlist", params: { id: id, title: httpString(tile.title) } }
    if kind = "extra" then return { to: "player", params: { itemId: id, startMs: 0 } }
    return invalid
end function
