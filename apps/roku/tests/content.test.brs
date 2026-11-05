' Items as docs/API.md's types describe them.

function contentTestServer() as string
    return "http://nas:7539"
end function

function contentTestMovie() as object
    return ParseJson("{""id"":12,""library_id"":1,""kind"":""movie"",""title"":""Arrival"",""sort_title"":""Arrival"",""year"":2016,""duration_ms"":6960000,""added_at"":1700000000,""poster_url"":""/api/items/12/images/poster?v=1729600000"",""backdrop_url"":""/api/items/12/images/backdrop?v=1729600000"",""user_state"":{""position_ms"":600000,""watched"":false,""play_count"":0}}")
end function

function contentTestEpisode() as object
    return ParseJson("{""id"":101,""library_id"":2,""kind"":""episode"",""title"":""Good News About Hell"",""sort_title"":""Good News About Hell"",""index"":1,""parent_id"":100,""parent_title"":""Season 1"",""parent_index"":1,""grandparent_id"":99,""grandparent_title"":""Severance"",""duration_ms"":3300000,""added_at"":1700000000,""thumb_url"":""/api/items/101/images/thumb?v=2"",""summary"":""Mark leads a team."",""user_state"":{""position_ms"":0,""watched"":true,""play_count"":1}}")
end function

function contentTestDetail() as object
    detail = contentTestMovie()
    detail.original_title = invalid
    detail.tagline = "Why are they here?"
    detail.genres = ["Drama", "Science Fiction", "Mystery", "Thriller"]
    detail.content_rating = "PG-13"
    detail.summary = "A linguist works with the military."
    detail.files = ParseJson("[{""id"":7,""path"":""Arrival (2016).mkv"",""size"":1,""container"":""mkv"",""duration_ms"":6960000,""bitrate"":null,""width"":3840,""height"":2160,""video_codec"":""hevc"",""audio_codec"":""eac3"",""streams"":[],""edition"":null,""selected_audio"":1,""selected_subtitle"":3,""chapters"":[]}]")
    detail.markers = []
    return detail
end function

sub test_content_movie_tiles_show_poster_year_and_progress()
    tile = contentTile(contentTestMovie(), contentTestServer(), false)
    tkEqual(tile.title, "Arrival")
    tkEqual(tile.subtitle, "2016")
    tkEqual(tile.image, "http://nas:7539/api/items/12/images/poster?v=1729600000&w=220")
    tkEqual(tile.itemId, 12)
    tkEqual(tile.kind, "movie")
    tkFalse(tile.watched)
    tkTrue(tile.progress > 0.086 and tile.progress < 0.087, "600000 of 6960000")
    tkEqual(tile.positionMs, 600000)
end sub

sub test_content_library_posters_show_their_files_only_when_asked()
    movie = contentTestMovie()
    movie.media = { bitrate: 42000000, size: 25125558681&, width: 3840, height: 1600, video_codec: "hevc", dynamic_range: "dolby_vision" }
    plain = contentTiles([movie, "not an item"], contentTestServer())
    tkEqual(plain.Count(), 1)
    tkFalse(plain[0].DoesExist("qualityBadges"), "no badges unless asked")
    tkEqual(plain[0].subtitle, "1 h 46 min left", "in progress: the time left")
    badged = contentTiles([movie], contentTestServer(), true)
    tkEqual(badged[0].qualityBadges, ["4K", "DV"])
    tkEqual(badged[0].subtitle, "1 h 46 min left", "the caption, unless sorted by bitrate")
    sorted = contentTiles([movie], contentTestServer(), true, true)
    tkEqual(sorted[0].qualityBadges, ["4K", "DV"])
    tkEqual(sorted[0].subtitle, "42 Mbps · 23.4 GB")
    movie.Delete("media")
    tkEqual(contentTiles([movie], contentTestServer(), true, true)[0].qualityBadges, [], "an older server's listing")
    tkEqual(contentTiles([movie], contentTestServer(), true, true)[0].subtitle, "Bitrate unknown")
end sub

sub test_content_quality_badges_say_4k_and_the_dynamic_range()
    tkEqual(contentQualityBadges({ width: 3840, height: 2160, dynamic_range: "hdr10" }), ["4K", "HDR"])
    tkEqual(contentQualityBadges({ width: 3996, height: 1716, dynamic_range: "hlg" }), ["4K", "HLG"], "a wide 4K film")
    tkEqual(contentQualityBadges({ width: 1920, height: 1080, dynamic_range: "hdr10plus" }), ["HDR+"], "no other resolution")
    tkEqual(contentQualityBadges({ width: 1920, height: 1080, dynamic_range: "sdr", video_codec: "h264" }), [], "1080p, SDR and a codec say nothing worth the room")
    tkEqual(contentQualityBadges({ width: invalid, height: invalid, dynamic_range: invalid }), [])
    tkEqual(contentQualityBadges(invalid), [])
    tkEqual(contentMediaLine({ bitrate: 8200000, size: 6979321856& }), "8.2 Mbps · 6.5 GB")
    tkEqual(contentMediaLine({ bitrate: invalid, size: 1048576 }), "1.0 MB")
    tkEqual(contentMediaLine({ bitrate: invalid, size: invalid }), "Bitrate unknown")
    tkEqual(contentMediaLine(invalid), "Bitrate unknown")
end sub

sub test_content_tiles_are_captioned_as_on_the_other_tvs()
    server = contentTestServer()
    tkEqual(contentTile(contentTestMovie(), server, false).subtitle, "2016", "without a caption: what places it, for lists")
    tkEqual(contentTile(contentTestMovie(), server, false, true).subtitle, "1 h 46 min left")
    fresh = contentTestMovie()
    fresh.user_state.position_ms = 0
    tkEqual(contentTile(fresh, server, false, true).subtitle, "2016")
    started = contentTestEpisode()
    started.user_state = { position_ms: 1740000, watched: false, play_count: 0 }
    tkEqual(contentTile(started, server, true, true).subtitle, "S1 · E1 · 26 min left")
    tkEqual(contentTile(contentTestEpisode(), server, true, true).subtitle, "S1 · E1 · Good News About Hell")
    show = { id: 99, kind: "show", title: "Severance", year: 2022, child_count: 3, leaf_count: 27, viewed_leaf_count: 27, user_state: { watched: true } }
    tkEqual(contentTile(show, server, false, true).subtitle, "3 seasons")
    show.child_count = invalid
    tkEqual(contentTile(show, server, false, true).subtitle, "27 episodes", "no seasons counted: its episodes")
    show.leaf_count = invalid
    tkEqual(contentTile(show, server, false, true).subtitle, "2022", "neither: its year")
    season = { id: 100, kind: "season", title: "Season 1", parent_title: "Severance", leaf_count: 10, viewed_leaf_count: 0 }
    tkEqual(contentTile(season, server, false, true).subtitle, "10 episodes")
    tkEqual(contentTile(season, server, false).subtitle, "Severance")
    track = { id: 5201, kind: "track", title: "Song 1", parent_title: "Blue", grandparent_title: "The Fixtures" }
    tkEqual(contentTile(track, server, false, true).subtitle, "The Fixtures · Blue")
    album = { id: 5100, kind: "album", title: "Blue", parent_title: "The Fixtures", year: 2001 }
    tkEqual(contentTile(album, server, false, true).subtitle, "The Fixtures · 2001")
    tkEqual(contentTile({ id: 5000, kind: "artist", title: "The Fixtures", child_count: 2 }, server, false, true).subtitle, "2 albums")
end sub

sub test_content_shows_and_seasons_count_their_episodes_left()
    server = contentTestServer()
    show = { id: 99, kind: "show", title: "Severance", leaf_count: 18, viewed_leaf_count: 2, user_state: { watched: false } }
    tile = contentTile(show, server, false)
    tkEqual(tile.unwatched, 16)
    tkFalse(tile.watched)
    tkEqual(tile.leafCount, 18)
    show.viewed_leaf_count = 18
    show.user_state.watched = true
    tile = contentTile(show, server, false)
    tkEqual(tile.unwatched, 0)
    tkTrue(tile.watched, "every episode watched: the mark")
    show.viewed_leaf_count = 17
    tkFalse(contentTile(show, server, false).watched, "one left: its count, not the mark")
    show.viewed_leaf_count = 30
    tkEqual(contentTile(show, server, false).unwatched, 0, "never below none")
    tkEqual(contentTile(contentTestMovie(), server, false).unwatched, 0, "a movie has no count")
    tkFalse(contentTile(contentTestMovie(), server, false).DoesExist("leafCount"))
end sub

sub test_content_episode_tiles_name_the_show()
    tile = contentTile(contentTestEpisode(), contentTestServer(), true)
    tkEqual(tile.title, "Severance")
    tkEqual(tile.subtitle, "S1 · E1 · Good News About Hell")
    tkEqual(tile.image, "http://nas:7539/api/items/101/images/thumb?v=2&w=400")
    tkTrue(tile.watched)
    tkEqual(tile.progress, 0, "watched shows no progress")
    tkEqual(tile.grandparentId, 99)
    poster = contentTile(contentTestEpisode(), contentTestServer(), false)
    tkEqual(poster.image, "http://nas:7539/api/items/101/images/poster?w=220", "the server falls back to the show's")
end sub

sub test_content_home_rows_of_every_kind()
    libraries = ParseJson("[{""id"":1,""name"":""Movies"",""kind"":""movies"",""paths"":[],""item_count"":245},{""id"":2,""name"":""TV Shows"",""kind"":""shows"",""paths"":[],""item_count"":31},{""id"":3,""name"":""Music"",""kind"":""music"",""paths"":[],""item_count"":120}]")
    hubs = [
        { key: "continue_watching", title: "Continue Watching", library_id: invalid, items: [contentTestEpisode(), contentTestMovie()] },
        { key: "my_list", title: "My List", library_id: invalid, items: [] },
        { key: "recently_added:1", title: "Recently Added in Movies", library_id: 1, items: [contentTestMovie()] },
        { key: "recently_added:3", title: "Recently Added in Music", library_id: 3, items: [{ id: 5, kind: "album", title: "Blue" }] },
        { key: "odd", title: "Odd", library_id: invalid, items: [{ id: 6, kind: "person", title: "Someone" }] }
    ]
    rows = contentHomeRows(hubs, contentTestServer())
    tkEqual(rows.Count(), 3, "an empty hub, and one of kinds this version doesn't show, are left out")
    tkEqual(rows[0].title, "Continue Watching")
    tkTrue(rows[0].wide)
    tkEqual(rows[0].tiles.Count(), 2)
    tkEqual(rows[0].tiles[1].image, "http://nas:7539/api/items/12/images/backdrop?v=1729600000&w=400", "a movie's backdrop in a wide tile")
    tkEqual(rows[0].tiles[1].subtitle, "1 h 46 min left", "captioned")
    tkEqual(rows[1].title, "Recently Added in Movies")
    tkFalse(rows[1].wide)
    tkFalse(rows[1].square)
    tkEqual(rows[0].tiles.Count(), 2, "Continue Watching has no See all")
    tkEqual(rows[1].tiles.Count(), 2, "the movie, then See all")
    tkEqual(rows[1].tiles[1], contentSeeAllTile(1, "added", false, false))
    tkTrue(rows[2].square, "a music library's albums")
    tkTrue(rows[2].tiles[0].square)
    tkTrue(rows[2].tiles[1].square, "See all in the row's shape")
    tkEqual(rows[2].tiles[0].image, "http://nas:7539/api/items/5/images/poster?w=260", "an album's cover, asked for square")
    tkEqual(contentRowSize(false, true), { size: [294, 360], height: 436 })
    tkEqual(contentRowSize(false, false).size, [252, 430], "a poster's cell, 220 wide inside it")
    tkEqual(contentRowSize(true, false).size, [441, 320], "a wide tile's, 400 inside")
    tkEqual(contentLibraries(libraries), [{ id: 1, name: "Movies", kind: "movies", itemCount: 245 }, { id: 2, name: "TV Shows", kind: "shows", itemCount: 31 }, { id: 3, name: "Music", kind: "music", itemCount: 120 }])
end sub

sub test_content_see_all_by_row()
    tkEqual(contentSeeAllSort("recently_added:3"), "added")
    tkEqual(contentSeeAllSort("recently_added"), "added", "a library's own rows")
    tkEqual(contentSeeAllSort("recently_released:1"), "release")
    tkEqual(contentSeeAllSort("top_rated:1"), "rating")
    tkEqual(contentSeeAllSort("continue_watching:1"), "")
    tkEqual(contentSeeAllSort("my_list"), "")
    hubs = [
        { key: "top_rated:1", title: "Top Rated", library_id: 1, items: [contentTestMovie()] },
        { key: "recently_added", title: "Recently Added", library_id: invalid, items: [contentTestMovie()] }
    ]
    rows = contentHomeRows(hubs, contentTestServer())
    tkEqual(rows[0].tiles[1].sort, "rating")
    tkEqual(rows[0].tiles[1].libraryId, 1)
    tkEqual(rows[1].tiles.Count(), 1, "without its library: no See all")
end sub

sub test_content_continue_watching_on_home_and_in_a_library()
    tkTrue(contentContinueWatching("continue_watching"))
    tkTrue(contentContinueWatching("continue_watching:5"), "a library's")
    tkFalse(contentContinueWatching("continue_watching_more"))
    tkFalse(contentContinueWatching("recently_added:5"))
    rows = contentHomeRows([{ key: "continue_watching:5", title: "Continue Watching", library_id: 5, items: [contentTestEpisode()] }], contentTestServer())
    tkTrue(rows[0].wide, "a library's Continue Watching is wide, as Home's")
    rows = contentHomeRows([{ key: "recently_added:2", title: "New Episodes", library_id: 2, items: [contentTestEpisode(), contentTestMovie()] }, { key: "recently_added:1", title: "Recently Added", library_id: 1, items: [contentTestMovie(), contentTestEpisode()] }], contentTestServer())
    tkTrue(rows[0].wide, "a row whose first title is an episode: its stills")
    tkEqual(rows[0].tiles[0].image, "http://nas:7539/api/items/101/images/thumb?v=2&w=400")
    tkFalse(rows[1].wide, "by its first title only")
    changes = contentRowChanges([{ key: "recently_added:2", title: "New Episodes", library_id: 2, items: [contentTestEpisode()] }], contentTestServer(), invalid)
    tkTrue(changes.summary[0].wide)
    known = { "recently_added:2": changes.summary[0].sig }
    tkTrue(contentRowChanges([{ key: "recently_added:2", title: "New Episodes", library_id: 2, items: [contentTestEpisode()] }], contentTestServer(), known).summary[0].wide, "as it was, wide too")
end sub

sub test_content_seasons_and_episodes()
    seasons = contentSeasons([
        { id: 98, kind: "season", title: "", index: 0, leaf_count: 2 },
        { id: 100, kind: "season", title: "Season 1", index: 1, leaf_count: 9, viewed_leaf_count: 3 },
        { id: 1, kind: "episode", title: "stray" }
    ])
    tkEqual(seasons, [{ itemId: 98, title: "Specials", index: 0, episodes: 2, watchedEpisodes: 0, year: 0 }, { itemId: 100, title: "Season 1", index: 1, episodes: 9, watchedEpisodes: 3, year: 0 }])
    tkEqual(contentSeasons([{ id: 7, kind: "season", index: invalid, year: 2024 }])[0].title, "Season", "no number")
    tkEqual(contentSeasons([{ id: 7, kind: "season", index: 2, year: 2024 }])[0].year, 2024)

    started = contentTestEpisode()
    started.user_state = { position_ms: 900000, watched: false, play_count: 0 }
    started.release_date = "2022-02-18"
    episodes = contentEpisodes([contentTestEpisode(), started], contentTestServer())
    tkEqual(episodes[0].title, "1. Good News About Hell")
    tkEqual(episodes[0].image, "http://nas:7539/api/items/101/images/thumb?v=2&w=400", "its still, as wide as the row's tiles")
    tkTrue(episodes[0].wide)
    tkEqual(episodes[0].subtitle, "55 min", "its watched mark says the rest")
    tkTrue(episodes[0].watched)
    tkEqual(episodes[0].summary, "Mark leads a team.")
    tkEqual(episodes[0].name, "Good News About Hell")
    tkEqual(episodes[0].line, "Season 1 · Episode 1 · 55 min")
    tkEqual(episodes[1].subtitle, "40 min left")
    tkEqual(episodes[1].line, "Season 1 · Episode 1 · 55 min · Aired Feb 18, 2022")
end sub

sub test_content_a_show_opens_on_the_next_episodes_season()
    seasons = contentSeasons([
        { id: 98, kind: "season", index: 0 },
        { id: 100, kind: "season", index: 1 },
        { id: 200, kind: "season", index: 2 }
    ])
    tkEqual(contentFirstSeason(seasons, { id: 203, kind: "episode", parent_id: 200 }), 200, "the next episode's")
    tkEqual(contentFirstSeason(seasons, invalid), 100, "nothing next: the first that isn't Specials")
    tkEqual(contentFirstSeason(seasons, { id: 5, kind: "episode", parent_id: 77 }), 100, "a season not listed")
    tkEqual(contentFirstSeason([seasons[0]], invalid), 98, "only Specials")
    tkEqual(contentFirstSeason([], invalid), 0)
end sub

sub test_content_details_offer_resume_and_start()
    detail = contentDetail(contentTestDetail(), contentTestServer())
    tkEqual(detail.heading, "Arrival")
    tkEqual(detail.context, "")
    tkEqual(detail.line, "2016 · 1 h 56 min · PG-13 · 4K")
    tkEqual(detail.left, "1 h 46 min left", "resumed: how much is left")
    tkEqual(detail.genres, "Drama · Science Fiction · Mystery · Thriller")
    tkEqual(detail.buttons, ["Resume", "From start", "Mark as watched", "Add to My List", "Add to playlist", "More…"])
    tkEqual(detail.actions, ["resume", "start", "watched", "listAdd", "playlist", "more"])
    tkEqual(detail.focus, 0)
    tkEqual(detail.more, [{ label: "Rate", action: "rate" }, { label: "Media info", action: "info" }])
    partOf = contentDetail(contentTestDetail(), contentTestServer(), invalid, { partOf: ["Arrival Collection"] })
    tkEqual(partOf.genres, "Drama · Science Fiction · Mystery · Thriller · Part of Arrival Collection")
    tkEqual(detail.backdrop, "http://nas:7539/api/items/12/images/backdrop?v=1729600000&w=640", "faint behind text: small")
    tkEqual(detail.poster, "http://nas:7539/api/items/12/images/poster?v=1729600000&w=420")

    fresh = contentTestDetail()
    fresh.user_state.position_ms = 0
    fresh.user_state.watched = true
    fresh.in_watchlist = true
    fresh.user_state.rating = 7
    tkEqual(contentDetail(fresh, contentTestServer()).buttons, ["Play", "Mark as unwatched", "Remove from My List", "Add to playlist", "More…"])
    tkEqual(contentDetail(fresh, contentTestServer()).left, "")
    tkEqual(contentDetail(fresh, contentTestServer()).more[0].label, "Your rating: 4 stars", "3.5 stars shown as 4, as the other apps do")

    long = contentTestDetail()
    long.summary = String(60, "A long summary. ")
    tkEqual(Len(contentDetail(long, contentTestServer()).summary), 959, "the whole summary, trimmed")

    empty = contentTestDetail()
    empty.files = []
    unavailable = contentDetail(empty, contentTestServer())
    tkEqual(unavailable.note, "")
    tkEqual(unavailable.buttons, ["Unavailable", "Mark as watched", "Add to My List", "Add to playlist", "More…"], "nothing to play, as on the other TVs")
    tkEqual(unavailable.actions[0], "unavailable")
    tkEqual(unavailable.focus, 1, "the focus starts on what can be pressed")
    tkEqual(unavailable.more.Count(), 1, "no media info without a file")
end sub

sub test_content_details_offer_audio_and_subtitles()
    detail = contentTestDetail()
    tkFalse(contentHasTracks(detail), "no streams")
    detail.files[0].streams = [{ index: 1, kind: "audio" }, { index: 2, kind: "audio" }]
    tkTrue(contentHasTracks(detail), "two audio tracks")
    tkEqual(contentDetail(detail, contentTestServer()).buttons, ["Resume", "From start", "Mark as watched", "Audio and subtitles", "Add to My List", "Add to playlist", "More…"])
    tkEqual(contentDetail(detail, contentTestServer()).actions[3], "tracks")
    detail.files[0].streams = [{ index: 1, kind: "audio" }]
    tkFalse(contentHasTracks(detail), "one audio track: nothing to choose")
    detail.files[0].streams.Push({ index: 3, kind: "subtitle" })
    tkTrue(contentHasTracks(detail), "subtitles")
    detail.files = []
    tkFalse(contentHasTracks(detail), "no file")
    detail.files = [{ id: 7, streams: [] }, { id: 8, streams: [] }]
    tkTrue(contentHasTracks(detail), "versions to choose from")
end sub

sub test_content_details_offer_the_trailer()
    detail = contentTestDetail()
    detail.extras = [{ id: 40, kind: "extra", extra_type: "behind_the_scenes" }, { id: 41, kind: "extra", extra_type: "Trailer" }]
    shown = contentDetail(detail, contentTestServer())
    tkEqual(shown.buttons, ["Resume", "From start", "Mark as watched", "Add to My List", "Trailer", "Add to playlist", "More…"])
    tkEqual(shown.actions[4], "trailer")
    tkEqual(shown.trailerId, 41)
    detail.extras = [{ id: 40, kind: "extra", extra_type: "featurette" }]
    tkEqual(contentDetail(detail, contentTestServer()).trailerId, 0, "no trailer among the extras")
    tkEqual(contentTrailer({ extras: invalid }), 0)
end sub

sub test_content_show_details_offer_its_next_episode()
    show = ParseJson("{""id"":99,""kind"":""show"",""title"":""Severance"",""year"":2022,""child_count"":2,""leaf_count"":18,""viewed_leaf_count"":2,""content_rating"":""TV-MA"",""user_state"":{""position_ms"":0,""watched"":false,""play_count"":0},""files"":[],""in_watchlist"":true}")
    detail = contentDetail(show, contentTestServer())
    tkEqual(detail.line, "2 seasons · 2022 · TV-MA", "Android TV's order")
    tkEqual(detail.buttons, ["Play", "Mark as watched", "Remove from My List", "Add to playlist", "More…"], "nothing next: Play says so")
    tkEqual(detail.note, "")
    tkEqual(contentDetail(show, contentTestServer(), invalid, { years: [2025, 2022, 2023] }).line, "2 seasons · 2022-2025 · TV-MA", "its seasons' years")
    onDeck = contentTestEpisode()
    onDeck.user_state = { position_ms: 600000, watched: false, play_count: 0 }
    detail = contentDetail(show, contentTestServer(), onDeck)
    tkEqual(detail.buttons, ["Resume S1 · E1", "From start", "Mark as watched", "Remove from My List", "Add to playlist", "More…"])
    tkEqual(detail.actions, ["playNext", "nextFromStart", "watched", "listRemove", "playlist", "more"])
    tkEqual(detail.left, "45 min left", "the next episode's")
    tkEqual(detail.more, [{ label: "Rate", action: "rate" }])
    onDeck.user_state.position_ms = 0
    tkEqual(contentDetail(show, contentTestServer(), onDeck).buttons[0], "Play S1 · E1")
    asking = contentDetail(show, contentTestServer(), onDeck, { askMore: true })
    tkEqual(asking.buttons[1], "Request more seasons")
    tkEqual(asking.actions[1], "askMore")
    show.viewed_leaf_count = 18
    tkEqual(contentDetail(show, contentTestServer(), onDeck).buttons[1], "Mark as unwatched", "every episode watched")
    show.leaf_count = 0
    tkEqual(contentDetail(show, contentTestServer()).buttons[0], "Unavailable", "no episodes")
end sub

sub test_content_season_details_name_the_show()
    season = { id: 100, kind: "season", title: "", index: 1, parent_id: 99, parent_title: "Severance", year: 2022, leaf_count: 9, viewed_leaf_count: 9, content_rating: "TV-MA", user_state: { position_ms: 0, watched: false } }
    detail = contentDetail(season, contentTestServer())
    tkEqual(detail.heading, "Severance")
    tkEqual(detail.context, "Season 1")
    tkEqual(detail.line, "2022 · 9 episodes · TV-MA")
    tkEqual(detail.buttons, ["Play", "Mark as unwatched", "Add to playlist", "More…"])
    tkEqual(detail.more, [{ label: "Go to show", action: "show" }])
    tkEqual(detail.showId, 99)
    tkEqual(contentSeasonTitle({ index: 0 }), "Specials")
    tkEqual(contentSeasonTitle({ title: "The Final Season", index: 4 }), "The Final Season")
end sub

sub test_content_file_quality_as_the_title_page_says_it()
    dv = { width: 3840, height: 2160, streams: [{ kind: "video", dynamic_range: "dolby_vision", dv_profile: 8, dv_bl_compat: 1 }, { kind: "audio", channels: 2 }, { kind: "audio", channels: 8, profile: "Dolby TrueHD + Dolby Atmos", default: true }] }
    tkEqual(contentFileBadges(dv), ["4K", "DV P8.1", "Atmos"], "the default audio")
    dv.streams[0] = { kind: "video", dynamic_range: "dolby_vision", dv_profile: 7, dv_bl_compat: 6 }
    dv.streams[2].profile = invalid
    tkEqual(contentFileBadges(dv), ["4K", "DV P7", "7.1"], "profile 7's base layer is never named")
    tkEqual(contentFileBadges({ width: 1920, height: 1080, streams: [{ kind: "video", dynamic_range: "sdr" }, { kind: "audio", channels: 6 }] }), ["5.1"])
    tkEqual(contentFileBadges(invalid), [])
    movie = contentTestDetail()
    movie.files.Push({ id: 9, edition: "Director's Cut", width: 1920, height: 1080, streams: [] })
    tkEqual(contentDetail(movie, contentTestServer(), invalid, { fileId: 9 }).line, "2016 · Director's Cut · 1 h 56 min · PG-13", "the version chosen")
end sub

sub test_content_episode_details_name_show_and_episode()
    episode = contentTestEpisode()
    episode.files = [{ id: 8, container: "mp4" }]
    detail = contentDetail(episode, contentTestServer())
    tkEqual(detail.buttons, ["Play", "Mark as unwatched", "Add to playlist", "More…"], "watched; no My List for an episode")
    tkEqual(detail.more, [{ label: "Rate", action: "rate" }, { label: "Go to show", action: "show" }, { label: "Go to season", action: "season" }, { label: "Media info", action: "info" }, { label: "Details", action: "about" }], "Details: played once")
    episode.parent_id = invalid
    tkEqual(contentDetail(episode, contentTestServer()).more[2].action, "info", "no season to go to")
    tkEqual(detail.showId, 99)
    tkEqual(detail.seasonId, 100)
    tkEqual(detail.heading, "Severance")
    tkEqual(detail.context, "S1 · E1 · Good News About Hell")
    tkEqual(detail.line, "55 min")
    episode.release_date = "2022-02-18"
    tkEqual(contentDetail(episode, contentTestServer()).line, "Feb 18, 2022 · 55 min", "its air day first")
    tkEqual(contentPlayerTitle(episode), "Severance · S1 · E1 · Good News About Hell")
    tkEqual(contentPlayerTitle(contentTestMovie()), "Arrival")
end sub

sub test_content_stars_round_up_as_the_other_apps()
    tkEqual(contentStars(invalid), 0)
    tkEqual(contentStars(0), 0)
    tkEqual(contentStars(1), 1, "half a star")
    tkEqual(contentStars(8), 4)
    tkEqual(contentStars(7.3), 4)
    tkEqual(contentStars(10), 5)
    tkEqual(contentStars(12), 5, "never more than 5")
    tkEqual(contentStarsLabel(0), "No rating")
    tkEqual(contentStarsLabel(1), "1 star")
    tkEqual(contentRatingLabel(0), "Rate")
    tkEqual(contentRatingLabel(5), "Your rating: 5 stars")
end sub

sub test_content_stream_formats()
    detail = contentTestDetail()
    tkEqual(contentStreamFormat({ hls: true, file_id: 7 }, detail), "hls")
    tkEqual(contentStreamFormat({ hls: false, file_id: 7 }, detail), "mkv")
    detail.files[0].container = "mov"
    tkEqual(contentStreamFormat({ hls: false, file_id: 7 }, detail), "mp4")
    tkEqual(contentStreamFormat({ hls: false, file_id: 99 }, detail), "mp4", "the first file when the id is unknown")
end sub

sub test_content_stream_carries_text_subtitles()
    info = ParseJson("{""session_id"":""s1"",""method"":""remux"",""url"":""/api/playback/s1/master.m3u8"",""hls"":true,""file_id"":7,""duration_ms"":6960000,""audio_index"":1,""subtitle_index"":null,""burn_subtitles"":false,""audio_converted"":false,""subtitles"":[{""index"":3,""codec"":""subrip"",""language"":""eng"",""title"":""English"",""default"":false,""forced"":false,""external"":false,""text_based"":true,""url"":""/api/files/7/subtitles/3.vtt""},{""index"":4,""codec"":""subrip"",""language"":null,""title"":null,""default"":false,""forced"":true,""external"":false,""text_based"":true,""url"":""/api/files/7/subtitles/4.vtt""},{""index"":5,""codec"":""hdmv_pgs_subtitle"",""language"":""fre"",""title"":null,""default"":false,""forced"":false,""external"":false,""text_based"":false,""url"":null}],""reasons"":[],""reason"":null,""auto_quality"":null}")
    stream = contentStream(info, contentTestDetail(), contentTestServer(), 600000)
    tkEqual(stream.url, "http://nas:7539/api/playback/s1/master.m3u8")
    tkEqual(stream.streamFormat, "hls")
    tkEqual(stream.title, "Arrival")
    tkEqual(stream.playStart, 600)
    tkEqual(stream.subtitleTracks, [
        { Language: "eng", TrackName: "http://nas:7539/api/files/7/subtitles/3.vtt", Description: "English" },
        { Language: "und", TrackName: "http://nas:7539/api/files/7/subtitles/4.vtt", Description: "Forced" }
    ])
    tkEqual(stream.subtitle, "http://nas:7539/api/files/7/subtitles/3.vtt", "the file's selected_subtitle")
    tkEqual(stream.previews, "", "no key, no previews: the Video node sends no token for them")
end sub

sub test_content_stream_names_a_keyed_sessions_previews()
    key = "pley_mk_" + String(43, "a")
    stream = contentStream({ url: "/api/media/" + key + "/master.m3u8", hls: true, file_id: 7 }, contentTestDetail(), contentTestServer(), 0)
    tkEqual(stream.url, "http://nas:7539/api/media/" + key + "/master.m3u8")
    tkEqual(stream.previews, "http://nas:7539/api/media/" + key + "/previews.bif")
    direct = contentStream({ url: "/api/media/" + key + "/file.mkv", hls: false, file_id: 7 }, contentTestDetail(), contentTestServer(), 0)
    tkEqual(direct.previews, "http://nas:7539/api/media/" + key + "/previews.bif", "a direct play's too")
end sub

sub test_content_names_subtitles()
    tkEqual(contentSubtitleName({ title: "SDH" }, "eng"), "SDH")
    tkEqual(contentSubtitleName({ title: invalid }, "fre"), "French", "a file without titles: the language's name, not its code")
    tkEqual(contentSubtitleName({ title: invalid }, "fra"), "French", "the ISO 639-2/T code")
    tkEqual(contentSubtitleName({ title: invalid }, "kur"), "kur", "a language not among the 34: its code")
    tkEqual(contentSubtitleName({ title: invalid, forced: true }, "fre"), "French (Forced)")
    tkEqual(contentSubtitleName({ title: "English (Forced)", forced: true }, "eng"), "English (Forced)", "said once")
    tkEqual(contentSubtitleName({ title: "FORCED", forced: true }, "eng"), "FORCED")
    tkEqual(contentSubtitleName({ title: "Forcedly", forced: true }, "eng"), "Forcedly (Forced)")
    tkEqual(contentSubtitleName({ title: "" }, "und"), "Unknown")
    tkEqual(contentSubtitleName({ forced: true }, "und"), "Forced")
end sub

sub test_content_rebuilds_only_the_home_rows_that_changed()
    hubs = [
        { key: "continue_watching", title: "Continue Watching", library_id: invalid, items: [contentTestEpisode(), contentTestMovie()] },
        { key: "recently_added:1", title: "Recently Added in Movies", library_id: 1, items: [contentTestMovie()] }
    ]
    server = contentTestServer()
    first = contentRowChanges(hubs, server, invalid)
    tkEqual(first.changed.Count(), 2, "nothing shown yet: every row")
    tkEqual(first.changed, contentHomeRows(hubs, server), "the same rows as built in one go")
    tkEqual(first.summary[0].key, "continue_watching")
    tkTrue(first.summary[0].wide)
    tkTrue(first.summary[1].built)
    known = {}
    for each row in first.summary
        known[row.key] = row.sig
    end for
    tkEqual(contentRowChanges(hubs, server, known).changed.Count(), 0, "the same answer: nothing to build")

    hubs[0].items[1].user_state.position_ms = 1200000
    hubs.Push({ key: "my_list", title: "My List", library_id: invalid, items: [contentTestMovie()] })
    hubs.Push({ key: "people", title: "People", library_id: invalid, items: [{ id: 5, kind: "person", title: "Someone" }] })
    again = contentRowChanges(hubs, server, known)
    tkEqual(again.summary.Count(), 3, "a hub with nothing this version shows is left out")
    tkEqual(again.changed.Count(), 2, "a title further along, and a new row")
    tkTrue(again.summary[0].built)
    tkFalse(again.summary[1].built)
    tkTrue(again.summary[1].sig = known["recently_added:1"])
    tkEqual(again.changed[1].key, "my_list")
end sub

sub test_content_home_asks_again_after_playing_or_a_minute()
    tkFalse(contentHomeStale(3, 3, 0), "back from a title at once")
    tkFalse(contentHomeStale(3, 3, 59))
    tkTrue(contentHomeStale(4, 3, 5), "something played")
    tkTrue(contentHomeStale(3, 3, 60), "other devices may have changed it")
end sub

sub test_content_up_next_card()
    nextEpisode = contentTestEpisode()
    nextEpisode.index = 2
    nextEpisode.title = "Half Loop"
    nextEpisode.user_state = { position_ms: 0, watched: false, play_count: 0 }
    card = contentUpNext(nextEpisode, contentTestServer())
    tkEqual(card.title, "Severance")
    tkEqual(card.line, "S1 · E2 · Half Loop")
    tkEqual(card.startMs, 0)
    nextEpisode.user_state.position_ms = 120000
    tkEqual(contentUpNext(nextEpisode, contentTestServer()).startMs, 120000, "its own place")
end sub

sub test_content_credits_name_the_makers_and_the_cast()
    movie = { kind: "movie", title: "Dune", crew: [{ id: 9, name: "Grace Hopper", role: "director" }, { id: 9, name: "Grace Hopper", role: "director" }, { id: 10, name: "Ada", role: "writer" }], cast: [{ id: 1, name: "A" }, { id: 2, name: "B" }, { id: 3, name: "C" }, { id: 4, name: "D" }] }
    tkEqual(contentCredits(movie), "Directed by Grace Hopper · Starring A, B, C")
    show = { kind: "show", crew: [{ id: 5, name: "Vince", role: "creator" }], cast: [] }
    tkEqual(contentCredits(show), "Created by Vince")
    tkEqual(contentCredits({ kind: "movie" }), "")
end sub

sub test_content_about_has_the_whole_summary_and_the_facts()
    now = 1767225600 ' 2026-01-01
    movie = { kind: "movie", title: "Dune", original_title: "Dune: Part One", release_date: "2021-10-22", studio: "Legendary", summary: "Paul goes to Arrakis.", user_state: { play_count: 2 }, crew: [{ id: 1, name: "A", role: "writer" }, { id: 2, name: "B", role: "writer" }, { id: 3, name: "C", role: "writer" }, { id: 4, name: "D", role: "writer" }, { id: 5, name: "E", role: "writer" }, { id: 6, name: "F", role: "director" }] }
    lines = contentAbout(movie, now, 0).Split(Chr(10))
    tkEqual(lines[0], "Paul goes to Arrakis.")
    tkEqual(lines[1], "")
    tkEqual(lines[2], "Director: F")
    tkEqual(lines[3], "Writers: A, B, C, D and 1 more")
    tkEqual(lines[4], "Original title: Dune: Part One")
    tkEqual(lines[5], "Released: Oct 22, 2021")
    tkEqual(lines[6], "Studio: Legendary")
    tkEqual(lines[7], "Plays: 2")
    tkEqual(contentAbout({ kind: "show", studio: "HBO", summary: "" }, now, 0), "Network: HBO")
    tkEqual(contentAbout({ kind: "movie", title: "Dune", original_title: "Dune", summary: "Short." }, now, 0), "", "nothing beyond the page")
end sub

sub test_content_details_offers_details_in_more()
    movie = { id: 5, kind: "movie", title: "Dune", studio: "Legendary", files: [{ id: 1 }], user_state: {} }
    actions = []
    for each entry in contentDetail(movie, contentTestServer()).more
        actions.Push(entry.action)
    end for
    tkEqual(actions, ["rate", "info", "about"], "Add to playlist is a button")
end sub
