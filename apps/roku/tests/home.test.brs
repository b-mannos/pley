function homeTestEpisode() as object
    return { id: 103, kind: "episode", title: "The Long Night", grandparent_title: "Fixture Show", grandparent_id: 90, parent_id: 100, parent_index: 2, index: 5, duration_ms: 3600000, year: 2019, added_at: 10, user_state: { position_ms: 2040000, watched: false } }
end function

sub test_home_hero_is_the_first_of_continue_watching()
    hubs = [
        { key: "my_list", items: [{ id: 1, kind: "movie", title: "Newest", added_at: 99 }] },
        { key: "continue_watching", items: [homeTestEpisode(), { id: 2, kind: "movie", title: "Second", added_at: 50 }] }
    ]
    tkEqual(homeHeroItem(hubs).id, 103)
end sub

sub test_home_hero_without_continue_watching_is_the_newest_video()
    hubs = [
        { key: "recently_added:3", items: [{ id: 7, kind: "album", title: "An album", added_at: 900 }] },
        { key: "recently_added:1", items: [{ id: 1, kind: "movie", title: "Older", added_at: 100 }, { id: 2, kind: "movie", title: "Newer", added_at: 200 }] },
        { key: "continue_watching", items: [] }
    ]
    tkEqual(homeHeroItem(hubs).id, 2)
    tkEqual(homeHeroItem([{ key: "recently_added:3", items: [{ id: 7, kind: "album", title: "An album", added_at: 900 }] }]).id, 7, "music when there is nothing else")
    tkEqual(homeHeroItem([]), invalid)
    tkEqual(homeHeroItem(invalid), invalid)
end sub

sub test_home_info_names_the_show_over_the_episode()
    info = homeInfo(homeTestEpisode(), "http://s", false)
    tkEqual(info.title, "Fixture Show")
    tkEqual(info.context, "S2 E5 · The Long Night")
    tkEqual(info.meta, "2019 · 1 h")
    tkEqual(info.timeLeft, "26 min left")
    tkTrue(info.progress > 0.56 and info.progress < 0.57)
    tkTrue(info.video)
    tkFalse(info.complete)
    tkEqual(info.backdrop, "http://s/api/items/90/images/backdrop?w=960", "the show's, without one of its own")
end sub

sub test_home_info_from_details_has_the_rating_and_badges()
    detail = { id: 5, kind: "movie", title: "Dune", year: 2021, duration_ms: 9300000, content_rating: "PG-13", rating: 7.9, summary: "Paul goes to Arrakis.", backdrop_url: "/api/items/5/images/backdrop?v=1", user_state: { position_ms: 0, watched: true }, files: [{ width: 3840, height: 2160, streams: [{ kind: "video", dynamic_range: "dolby_vision" }] }] }
    info = homeInfo(detail, "http://s", true)
    tkEqual(info.title, "Dune")
    tkEqual(info.context, "")
    tkEqual(info.meta, "2021 · 2 h 35 min · PG-13 · ★ 7.9 · 4K · DV")
    tkEqual(info.summary, "Paul goes to Arrakis.")
    tkEqual(info.timeLeft, "", "watched: not in progress")
    tkTrue(info.watched)
    tkEqual(info.backdrop, "http://s/api/items/5/images/backdrop?v=1&w=960")
end sub

sub test_home_info_counts_a_shows_seasons()
    info = homeInfo({ id: 9, kind: "show", title: "A show", year: 2008, child_count: 5, user_state: {} }, "http://s", false)
    tkEqual(info.meta, "2008 · 5 seasons")
    tkFalse(info.video)
    tkEqual(homeInfo({ id: 9, kind: "season", title: "Season 1", leaf_count: 1, user_state: {} }, "http://s", false).meta, "1 episode")
end sub

sub test_home_hero_buttons()
    inProgress = homeInfo(homeTestEpisode(), "http://s", false)
    tkEqual(homeHeroButtons(inProgress), { labels: ["Resume", "From start", "Info", "Mark as watched"], actions: ["resume", "start", "info", "watched"] })
    watched = homeInfo({ id: 5, kind: "movie", title: "Dune", user_state: { watched: true } }, "http://s", false)
    tkEqual(homeHeroButtons(watched), { labels: ["Play", "Info", "Mark as unwatched"], actions: ["start", "info", "unwatched"] })
    show = homeInfo({ id: 9, kind: "show", title: "A show", user_state: {} }, "http://s", false)
    tkEqual(homeHeroButtons(show), { labels: ["Play", "Info", "Mark as watched"], actions: ["next", "info", "watched"] }, "a show plays its next episode, as on Android TV")
    season = homeInfo({ id: 10, kind: "season", title: "Season 1", user_state: {} }, "http://s", false)
    tkEqual(homeHeroButtons(season).actions, ["next", "info", "watched"])
    album = homeInfo({ id: 7, kind: "album", title: "An album", user_state: {} }, "http://s", false)
    tkEqual(homeHeroButtons(album), { labels: ["Open"], actions: ["info"] })
end sub

sub test_home_says_why_it_is_empty()
    tkEqual(homeEmpty([]).title, "No libraries yet")
    tkEqual(homeEmpty([{ id: 1, scanning: true }]).title, "Scanning your libraries…")
    tkEqual(homeEmpty([{ id: 1, scanning: false }]).title, "Nothing here yet")
    tkEqual(homeEmpty(invalid).title, "Nothing here yet", "the libraries not known yet")
end sub

sub test_home_menu_follows_the_other_tvs_rail()
    movies = { id: 1, name: "Movies", kind: "movies" }
    music = { id: 3, name: "Music", kind: "music" }
    requests = [{ title: "Discover", action: "discover", count: 2, danger: false }]
    actions = []
    titles = []
    for each entry in homeMenu([movies, music], true, requests, true)
        actions.Push(entry.action)
        titles.Push(entry.title)
    end for
    tkEqual(actions, ["search", "home", "discover", "live", "library", "library", "playlists", "collections", "nowPlaying", "settings"])
    tkEqual(titles[8], "Now Playing", "as the other TVs write it")
    actions = []
    for each entry in homeMenu([music], false, [], false)
        actions.Push(entry.action)
    end for
    tkEqual(actions, ["search", "home", "library", "playlists", "settings"], "no Collections without movies or shows")
end sub

sub test_home_menu_has_one_way_into_requests()
    summary = reqSummary({ enabled: true, access: "ask", manage: true, pending: 2, failed: 1, unseen: 1 })
    tkEqual(homeRequestsEntries(summary), [{ title: "Discover", action: "discover", count: 4, danger: true }], "Discover for those who may ask, with the requests' number")
    summary = reqSummary({ enabled: true, access: "off", manage: true, pending: 2, failed: 0, unseen: 0 })
    tkEqual(homeRequestsEntries(summary), [{ title: "Requests", action: "requests", count: 2, danger: false }], "a manager who may not ask: Requests")
    summary = reqSummary({ enabled: true, access: "off", own: 3, unseen: 1 })
    tkEqual(homeRequestsEntries(summary), [{ title: "Your requests", action: "requests", count: 1, danger: false }], "their own requests left to see")
    tkEqual(homeRequestsEntries(reqSummary({ enabled: true, access: "off" })), [], "nothing to ask or see")
    tkEqual(homeRequestsEntries(reqSummary({ enabled: false, access: "ask" })), [], "requests off")
    tkEqual(homeRequestsEntries(reqNoSummary(false)), [], "no summary yet")
end sub
