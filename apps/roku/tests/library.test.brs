' A library's tabs, sort and filters, and how they are kept.

sub test_library_tabs_by_kind()
    tkEqual(libTabs("movies"), ["recommended", "library", "collections"])
    tkEqual(libTabs("music"), ["recommended", "library"], "music has no collections")
    tkEqual(libTabLabel("recommended"), "Recommended")
    tkEqual(libTabLabel("library"), "Library")
end sub

sub test_library_sorts_by_kind()
    ids = function(sorts as object) as object
        out = []
        for each sort in sorts
            out.Push(sort.id)
        end for
        return out
    end function
    tkEqual(ids(libSorts("movies")), ["title", "year", "added", "rating", "user_rating", "release", "duration", "last_played", "bitrate"])
    tkEqual(ids(libSorts("shows")), ["title", "year", "added", "rating", "user_rating", "release", "last_played", "bitrate"], "shows: no duration")
    tkEqual(ids(libSorts("music")), ["title", "added", "user_rating", "last_played"])
    tkEqual(libDefaultOrder("title"), "asc")
    tkEqual(libDefaultOrder("added"), "desc")
    tkEqual(libSortLabel("title", "asc"), "Title, A to Z")
    tkEqual(libSortLabel("year", "desc"), "Year, newest first")
    tkEqual(libSortLabel("added", "asc"), "Date added, first added first")
end sub

sub test_library_query_and_filters()
    view = libDefaultView()
    tkEqual(libQuery(view, 0, 70), { sort: "title", order: "asc", offset: 0, limit: 70 })
    tkEqual(libFilterLabel(view), "All titles")
    tkFalse(libFiltered(view))
    view.watched = "false"
    view.genre = "Comedy"
    view.year = 1999
    tkEqual(libQuery(view, 70, 70), { sort: "title", order: "asc", offset: 70, limit: 70, watched: "false", genre: "Comedy", year: 1999 })
    tkEqual(libFilterLabel(view), "Unwatched, Comedy, 1999")
    tkTrue(libFiltered(view))
end sub

sub test_library_views_kept_and_checked()
    section = CreateObject("roRegistrySection", "libraryViewsTest")
    key = libViewKey("http://nas:7539", 4, 2)
    tkEqual(key, "http://nas:7539|4|2")
    section.Delete(key)
    tkEqual(libLoadView(section, key, "movies"), libDefaultView(), "nothing kept: the defaults")
    view = libDefaultView()
    view.tab = "collections"
    view.sort = "duration"
    view.order = "desc"
    view.genre = "Drama"
    libSaveView(section, key, view)
    tkEqual(libLoadView(section, key, "movies"), view)
    shows = libLoadView(section, key, "shows")
    tkEqual(shows.sort, "title", "a sort the kind doesn't offer")
    tkEqual(shows.order, "desc", "the order kept")
    music = libLoadView(section, key, "music")
    tkEqual(music.tab, "library", "a tab the kind doesn't have")
    section.Write(key, "not json")
    tkEqual(libLoadView(section, key, "movies"), libDefaultView())
    section.Delete(key)
end sub
