' Title pages' rows, people's and collections' pages, and the Options key.

sub test_pages_credits_cast_then_crew_once_each()
    detail = {
        cast: [
            { id: 7, name: "Amy Adams", role: "actor", character: "Louise Banks", photo_url: "/api/people/7/photo?v=1" },
            { id: 8, name: "Jeremy Renner", role: "actor", character: "Ian Donnelly" },
            { id: 7, name: "Amy Adams", role: "actor", character: "Louise (young)" }
        ],
        crew: [
            { id: 9, name: "Denis Villeneuve", role: "director" },
            { id: 9, name: "Denis Villeneuve", role: "writer" },
            { id: 10, name: "Eric Heisserer", role: "writer" },
            { id: 11, name: "Someone", role: "editor" },
            { id: 8, name: "Jeremy Renner", role: "producer" }
        ]
    }
    tiles = pageCredits(detail, "http://nas:7539")
    names = []
    for each tile in tiles
        names.Push(tile.title + ": " + tile.subtitle)
    end for
    tkEqual(names, ["Amy Adams: Louise Banks", "Jeremy Renner: Ian Donnelly, Producer", "Denis Villeneuve: Director, Writer", "Eric Heisserer: Writer"])
    tkEqual(tiles[0].image, "http://nas:7539/api/people/7/photo?v=1&w=220")
    tkEqual(tiles[0].kind, "person")
    tkEqual(tiles[0].itemId, 7)
    tkEqual(tiles[1].image, "", "no photo: the name shows")
end sub

sub test_pages_extras_say_what_they_are()
    detail = { extras: [{ id: 40, kind: "extra", title: "Official Trailer", extra_type: "trailer", duration_ms: 150000, thumb_url: "/api/items/40/images/thumb?v=1", user_state: { position_ms: 0, watched: false } }, { id: 41, kind: "extra", title: "Making of", extra_type: "odd" }] }
    tiles = pageExtras(detail, "http://nas:7539")
    tkEqual(tiles[0].title, "Official Trailer")
    tkEqual(tiles[0].subtitle, "Trailer · 3 min")
    tkEqual(tiles[0].kind, "extra")
    tkTrue(tiles[0].wide)
    tkEqual(tiles[1].subtitle, "Extra")
    rows = pageDetailRows(detail, "http://nas:7539")
    tkEqual(rows.Count(), 1, "no cast: no row for it")
    tkEqual(rows[0].key, "extras")
end sub

sub test_pages_person_credits_say_the_part()
    person = { id: 7, name: "Amy Adams", credits: [
        { item: { id: 12, kind: "movie", title: "Arrival", year: 2016, user_state: { position_ms: 0, watched: true } }, roles: [{ role: "actor", character: "Louise Banks" }] },
        { item: { id: 99, kind: "show", title: "Sharp Objects", year: 2018 }, roles: [{ role: "actor", character: "Camille", episodes: 8 }, { role: "producer" }] },
        { item: { id: 5, kind: "album", title: "Blue" }, roles: [] }
    ] }
    tiles = pagePersonCredits(person, "http://nas:7539")
    tkEqual(tiles.Count(), 2, "titles this version shows")
    tkEqual(tiles[0].subtitle, "Louise Banks")
    tkTrue(tiles[0].watched)
    tkEqual(tiles[1].subtitle, "Camille, Producer")
end sub

sub test_pages_collections_and_seasons_as_tiles()
    tile = pageCollectionTile({ id: 3, title: "Dune Collection", item_count: 2 }, "http://nas:7539")
    tkEqual(tile.image, "http://nas:7539/api/collections/3/images/poster?w=220", "the server's picture of it")
    tkEqual(tile.subtitle, "2 titles")
    tkEqual(tile.kind, "collection")
    tkEqual(pageCollectionTile({ id: 4, title: "One", item_count: 1, poster_url: "/api/collections/4/images/poster?v=2" }, "http://nas:7539").image, "http://nas:7539/api/collections/4/images/poster?v=2&w=220")
    seasons = pageSeasonTiles([{ id: 100, kind: "season", index: 1, parent_id: 99, leaf_count: 9 }, { id: 101, kind: "season", index: 0, title: "", leaf_count: 1 }, { id: 5, kind: "episode" }], "http://nas:7539")
    tkEqual(seasons.Count(), 2)
    tkEqual(seasons[0].title, "Season 1")
    tkEqual(seasons[0].subtitle, "9 episodes")
    tkEqual(seasons[0].parentId, 99)
    tkEqual(seasons[1].title, "Specials")
    tkEqual(seasons[1].subtitle, "1 episode")
end sub

sub test_pages_tile_options_by_kind()
    labels = function(options as object) as object
        out = []
        for each option in options
            out.Push(option.label)
        end for
        return out
    end function
    tkEqual(labels(pageTileOptions({ kind: "movie", itemId: 12, watched: false }, false, false)), ["Mark as watched", "Add to My List"])
    tkEqual(labels(pageTileOptions({ kind: "movie", itemId: 12, watched: true }, true, true)), ["Mark as unwatched", "Remove from My List", "Remove from Continue Watching"])
    tkEqual(labels(pageTileOptions({ kind: "show", itemId: 99 }, false, invalid)), ["Mark as watched"], "My List not known")
    tkEqual(labels(pageTileOptions({ kind: "episode", itemId: 101, grandparentId: 99 }, true, false)), ["Mark as watched", "Remove from Continue Watching", "Go to show"])
    tkEqual(pageTileOptions({ kind: "person", itemId: 7 }, false, false).Count(), 0)
end sub

sub test_pages_actions_ask_the_server()
    tkEqual(pageActionRequest("watched", 12), { method: "POST", path: "/api/items/12/watched" })
    tkEqual(pageActionRequest("unwatched", 12), { method: "POST", path: "/api/items/12/unwatched" })
    tkEqual(pageActionRequest("listAdd", 12), { method: "PUT", path: "/api/watchlist/12" })
    tkEqual(pageActionRequest("listRemove", 12), { method: "DELETE", path: "/api/watchlist/12" })
    tkEqual(pageActionRequest("hide", 101), { method: "DELETE", path: "/api/continue-watching/101" })
    tkInvalid(pageActionRequest("show", 101))
end sub

sub test_pages_my_list_as_home_saw_it()
    tkInvalid(pageInList(invalid, 12), "not known yet")
    tkTrue(pageInList([5, 12], 12))
    tkFalse(pageInList([5], 12))
    tkEqual(pageListWith([5, 12], 12, false), [5])
    tkEqual(pageListWith([5], 12, true), [12, 5], "added first, as the server lists it")
end sub
