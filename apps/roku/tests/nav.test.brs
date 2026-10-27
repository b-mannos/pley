sub test_nav_names_screens()
    tkEqual(navScreen("home"), "HomeScreen")
    tkEqual(navScreen("player"), "PlayerScreen")
    tkEqual(navScreen("unreachable"), "UnreachableScreen")
    tkEqual(navScreen("sortLibraries"), "SortLibrariesScreen")
    tkEqual(navScreen("nowhere"), "")
end sub

sub test_nav_tiles_open_details_or_their_show()
    tkEqual(navForTile({ kind: "movie", itemId: 12 }), { to: "details", params: { itemId: 12 } })
    tkEqual(navForTile({ kind: "episode", itemId: 101, parentId: 100 }), { to: "details", params: { itemId: 101 } })
    tkEqual(navForTile({ kind: "show", itemId: 99 }), { to: "show", params: { showId: 99, seasonId: 0 } })
    tkEqual(navForTile({ kind: "season", itemId: 100, parentId: 99 }), { to: "show", params: { showId: 99, seasonId: 100 } })
    tkInvalid(navForTile({ kind: "album", itemId: 5 }))
    tkInvalid(navForTile({ kind: "season", itemId: 100, parentId: 0 }))
    tkInvalid(navForTile({ kind: "movie" }))
end sub
