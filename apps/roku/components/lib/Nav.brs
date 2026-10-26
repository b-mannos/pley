' Where the screens go: their names, and the screen a tile opens.

' The component of a screen by its name in a navigate request, or "".
function navScreen(name as string) as string
    screens = {
        servers: "ServersScreen",
        link: "LinkScreen",
        home: "HomeScreen",
        library: "LibraryScreen",
        show: "ShowScreen",
        details: "DetailsScreen",
        player: "PlayerScreen",
        search: "SearchScreen",
        settings: "SettingsScreen",
        unreachable: "UnreachableScreen"
    }
    if not screens.DoesExist(name) then return ""
    return screens[name]
end function

' The screen a tile opens: { to, params }, or invalid for kinds this
' version doesn't open. A season opens its show on that season.
function navForTile(tile as object) as dynamic
    kind = httpString(tile.kind)
    id = httpNumber(tile.itemId, 0)
    if id = 0 then return invalid
    if kind = "movie" or kind = "episode" then return { to: "details", params: { itemId: id } }
    if kind = "show" then return { to: "show", params: { showId: id, seasonId: 0 } }
    if kind = "season"
        showId = httpNumber(tile.parentId, 0)
        if showId = 0 then return invalid
        return { to: "show", params: { showId: showId, seasonId: id } }
    end if
    return invalid
end function
