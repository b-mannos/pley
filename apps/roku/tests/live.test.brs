' The lab's channels, as GET /api/live/channels answers.
function liveSample() as object
    return [
        {
            id: 1, source_id: 1, name: "Pley News HD", number: "101", group: "News", logo: "/api/live/channels/1/logo", favorite: false, has_guide: true,
            now: { channel_id: 1, start: 1000, stop: 2000, title: "Morning Briefing", subtitle: "Part 7", description: "Everything you need to know.", categories: ["News"], image: invalid, season: 1, episode: 7, is_new: false, is_live: false, rating: invalid },
            next: { channel_id: 1, start: 2000, stop: 2600, title: "World at One", subtitle: invalid, description: invalid, categories: [], image: invalid, season: invalid, episode: invalid, is_new: true, is_live: false, rating: "TV-PG" }
        },
        {
            id: 2, source_id: 1, name: "Sport 1080i", number: "102", group: "Sport", logo: invalid, favorite: true, has_guide: true,
            now: { channel_id: 2, start: 1000, stop: 2000, title: "Match of the Day", is_live: true },
            next: invalid
        },
        { id: 3, source_id: 1, name: "Film Four", number: "103", group: "News", logo: invalid, favorite: false, has_guide: false, now: invalid, next: invalid },
        { id: 0, name: "No id" },
        "not a channel"
    ]
end function

sub test_live_channels_read()
    channels = liveChannels(liveSample(), "http://tv:7539")
    tkEqual(channels.Count(), 3, "those without an id are left out")
    news = channels[0]
    tkEqual(news.id, 1)
    tkEqual(news.number, "101")
    tkEqual(news.logo, "http://tv:7539/api/live/channels/1/logo?w=240")
    tkEqual(news.now.title, "Morning Briefing")
    tkEqual(news.now.line, "S1 E7 · Part 7")
    tkEqual(news.now.ends, 2000)
    tkEqual(news.after.badges, "New · TV-PG")
    tkEqual(channels[1].logo, "", "no logo")
    tkTrue(channels[1].now.live)
    tkInvalid(channels[2].now)
    tkEqual(liveTitle(news), "101  Pley News HD")
    tkEqual(liveTitle({ number: "", name: "Radio" }), "Radio")
    tkInvalid(liveProgram({ start: 10, stop: 10, title: "zero" }), "a programme that ends as it starts")
end sub

sub test_live_now_and_next()
    news = liveChannels(liveSample(), "")[0]
    tkEqual(liveOnNow(news, 1500).title, "Morning Briefing")
    tkEqual(liveComingUp(news, 1500).title, "World at One")
    tkEqual(liveOnNow(news, 2100).title, "World at One", "the next one, once it began")
    tkInvalid(liveComingUp(news, 2100))
    tkInvalid(liveOnNow(news, 900), "before the guide's now")
    tkEqual(liveComingUp(news, 900).title, "Morning Briefing")
    tkInvalid(liveOnNow(news, 3000), "after both")
    tkEqual(liveProgress(news.now, 1500), 0.5)
    tkEqual(liveProgress(news.now, 500), 0)
    tkEqual(liveProgress(news.now, 2500), 1)
    tkEqual(liveTimeLeft(news.now, 1500), "8 min left")
    tkEqual(liveTimeLeft({ start: 0, ends: 7200 }, 0), "2 h left")
    tkEqual(liveTimeLeft({ start: 0, ends: 4000 }, 0), "1 h 7 min left")
    tkEqual(liveTimeLeft({ start: 0, ends: 10 }, 0), "1 min left")
end sub

sub test_live_clock_text()
    tkEqual(liveClockText(20, 5, true), "8:05 PM")
    tkEqual(liveClockText(0, 30, true), "12:30 AM")
    tkEqual(liveClockText(12, 0, true), "12:00 PM")
    tkEqual(liveClockText(9, 0, false), "09:00")
    tkEqual(liveClockText(20, 5, false), "20:05")
end sub

sub test_live_filters()
    channels = liveChannels(liveSample(), "")
    choices = liveFilterChoices(channels)
    tkEqual(choices.Count(), 4, "all, favourites and two groups")
    tkEqual(choices[0], { value: "all", label: "All channels · 3" })
    tkEqual(choices[1], { value: "favorites", label: "Favourites · 1" })
    tkEqual(choices[2], { value: "group:News", label: "News · 2" })
    tkEqual(liveFiltered(channels, "group:News").Count(), 2)
    tkEqual(liveFiltered(channels, "favorites")[0].id, 2)
    tkEqual(liveFiltered(channels, "all").Count(), 3)
    tkEqual(liveFilterValid(channels, "group:Kids"), "all", "a group gone")
    tkEqual(liveFilterValid(channels, "group:Sport"), "group:Sport")
    channels[1].favorite = false
    tkEqual(liveFilterValid(channels, "favorites"), "all", "no favourites any more")
    tkEqual(liveFilterLabel("group:Sport"), "Sport")
    tkEqual(liveFilterLabel("favorites"), "Favourites")
    tkEqual(liveFilterLabel("all"), "All channels")
    one = liveFilterChoices([{ id: 1, group: "News", favorite: false }])
    tkEqual(one.Count(), 1, "one group: no group to choose")
end sub

' An IPTV lineup of 1,500 channels in three groups, a few favourites.
function liveBigLineup() as object
    channels = []
    groups = ["Sport", "News", "Films"]
    for i = 1 to 1500
        channels.Push({ id: i, name: "Channel " + i.ToStr(), number: i.ToStr(), group: groups[i mod 3], logo: "", favorite: i mod 500 = 0, hasGuide: false })
    end for
    return channels
end function

sub test_live_big_lineups_show_a_group_at_a_time()
    channels = liveBigLineup()
    choices = liveFilterChoices(channels)
    tkEqual(choices[0], { value: "favorites", label: "Favourites · 3" }, "no All channels past liveMany")
    tkEqual(choices.Count(), 4)
    tkEqual(liveFilterValid(channels, "all"), "favorites", "the first choice there is")
    tkEqual(liveFilterValid(channels, "group:News"), "group:News")
    for each channel in channels
        channel.favorite = false
    end for
    tkEqual(liveFilterValid(channels, "all"), "group:News", "the first group, without favourites")
    rows = liveRows(channels, [7, 8], 0, false)
    tkEqual(rows.Count(), 4, "Recently watched and the three groups")
    tkEqual(rows[1].cards.Count(), liveRowMax(), "a row of liveRowMax cards at most")
    tkEqual(liveRefreshMs(channels.Count()), 600000)
    tkEqual(liveRefreshMs(5), 55000)
    many = []
    for i = 1 to 30
        many.Push({ id: i, name: "c", number: "", group: "G" + i.ToStr(), logo: "", favorite: false, hasGuide: false })
    end for
    tkEqual(liveFilterValid(many, "all"), "all", "a small lineup shows every channel")
end sub

sub test_live_steps_round_the_ends()
    channels = liveChannels(liveSample(), "")
    tkEqual(liveStep(channels, 1, 1).id, 2)
    tkEqual(liveStep(channels, 3, 1).id, 1, "after the last, the first")
    tkEqual(liveStep(channels, 1, -1).id, 3, "before the first, the last")
    tkEqual(liveStep(channels, 99, 1).id, 1, "a channel not listed")
    tkInvalid(liveStep([], 1, 1))
end sub

sub test_live_cards_and_rows()
    channels = liveChannels(liveSample(), "")
    card = liveCard(channels[0], 1500, false)
    tkEqual(card.line, "Morning Briefing")
    tkEqual(card.progress, 0.5)
    tkTrue(Instr(1, card.nextLine, " · World at One") > 0, "what's next, with its time")
    tkEqual(liveCard(channels[2], 1500, false).line, "No guide")
    tkEqual(liveCard(channels[1], 2500, false).line, "Nothing in the guide now")
    rows = liveRows(channels, [3, 99, 1], 1500, false)
    keys = []
    for each row in rows
        keys.Push(row.key)
    end for
    tkEqual(keys, ["recent", "favorites", "group:News", "group:Sport"])
    tkEqual(rows[0].cards.Count(), 2, "a channel gone is left out")
    tkEqual(rows[0].cards[0].channelId, 3, "newest first")
    tkEqual(rows[2].cards.Count(), 2)
    item = liveListItem(channels[0], 2100, false)
    tkEqual(item.line, "World at One")
    tkEqual(liveListItem(channels[0], 900, false).progress, 0)
    tkTrue(Instr(1, liveListItem(channels[0], 900, false).line, " · Morning Briefing") > 0, "what comes on, with its time")
    tkEqual(liveListItem(channels[2], 1500, false).line, "")
    single = liveRows(liveFiltered(channels, "group:News"), [], 1500, false)
    tkEqual(single.Count(), 1)
    tkEqual(single[0].title, "All channels", "one group: one row")
end sub

sub test_live_guide_programmes()
    kept = {}
    liveGuideKeep(kept, { "1": [{ start: 0, stop: 100, title: "A" }, { start: 100, stop: 200, title: "B" }] })
    liveGuideKeep(kept, { "1": [{ start: 100, stop: 200, title: "B" }, { start: 150, stop: 300, title: "C overlaps" }, { start: 300, stop: 400, title: "D" }], "2": [] })
    programs = liveGuidePrograms(kept, 1)
    titles = []
    for each p in programs
        titles.Push(p.title)
    end for
    tkEqual(titles, ["A", "B", "C overlaps", "D"], "each once, by start")
    tkEqual(programs[2].start, 200, "an overlap starts where the one before stops")
    tkEqual(liveGuidePrograms(kept, 2), [])
    rows = liveGuideRows(liveChannels(liveSample(), "http://tv"), kept, 0, 600)
    tkEqual(rows.Count(), 3)
    tkEqual(rows[0].fields.title, "101  Pley News HD")
    tkEqual(rows[0].fields.id, "1")
    tkEqual(rows[0].programs.Count(), 5, "four, then the time the guide says nothing of")
    tkEqual(rows[0].programs[3].playStart, 300)
    tkEqual(rows[0].programs[3].playDuration, 100)
    tkEqual(rows[0].programs[4].id, "gap")
    tkEqual(rows[0].programs[4].playStart, 400)
    tkEqual(rows[0].programs[4].playDuration, 200)
    tkEqual(rows[1].programs.Count(), 1, "no guide: one gap over what is fetched")
    tkEqual(rows[1].programs[0].title, "No guide information")
    kept = {}
    liveGuideKeep(kept, { "1": [{ start: 1000, stop: 1030, title: "A" }, { start: 1100, stop: 1200, title: "B" }] })
    programs = liveGuideRows([{ id: 1, number: "", name: "x", logo: "" }], kept, 900, 1200)[0].programs
    tkEqual(programs.Count(), 4, "gaps of a minute or more, before and between")
    tkEqual(programs[0].playDuration, 100)
    tkEqual(programs[2].playStart, 1030)
end sub

sub test_live_now_and_next_guide()
    channels = liveChannels(liveSample(), "")
    rows = liveNowNextRows(channels, 900)
    tkEqual(rows.Count(), 3)
    tkEqual(rows[0].programs.Count(), 3, "the time before now, now, next")
    tkEqual(rows[0].programs[0].id, "gap")
    tkEqual(rows[0].programs[1].title, "Morning Briefing")
    tkEqual(rows[0].programs[2].title, "World at One")
    tkEqual(rows[1].programs.Count(), 2, "now only")
    tkEqual(rows[2].programs.Count(), 1, "no guide: one gap")
    tkEqual(rows[2].programs[0].playDuration, liveGuideChunk())
    tkEqual(liveNowNextRows(channels, 2100)[0].programs.Count(), 1, "what ended before the guide starts is left out")
end sub

sub test_live_guide_window()
    start = liveGuideStart(7300)
    tkEqual(start, 5400, "the half hour before the one now is in")
    tkInvalid(liveGuideMissing(0, 21600, 0, 7200), "the view and an hour more are fetched")
    tkEqual(liveGuideMissing(0, 21600, 12000, 7200), { from: 21600, to: 43200 })
    tkEqual(liveGuideMissing(0, 162000, 160000, 7200), { from: 162000, to: 172800 }, "not past the span")
    tkInvalid(liveGuideMissing(0, 172800, 170000, 7200), "the span fetched")
end sub

sub test_live_troubles()
    busy = liveTrouble({ status: 409, message: "The one connection of Test Provider is in use, watching Pley News HD (lab)." }, "Film Four")
    tkEqual(busy.kind, "busy")
    tkEqual(busy.title, "All connections are in use")
    tkEqual(busy.message, "The one connection of Test Provider is in use, watching Pley News HD (lab).")
    tkTrue(busy.retry)
    tkEqual(liveTrouble({ status: 404, message: "" }, "x").kind, "missing")
    tkFalse(liveTrouble({ status: 404, message: "" }, "x").retry)
    tkEqual(liveTrouble({ status: 503, message: "the channel didn't start" }, "Film Four").title, "Film Four isn't playing")
    tkEqual(liveTrouble({ status: 500, message: "boom" }, "Film Four").title, "Film Four didn't start")
    tkFalse(liveTrouble({ status: 410, message: "the channel was stopped" }, "x").retry, "stopped by an administrator")
end sub

sub test_live_badge()
    tkEqual(liveBadge(100, 108, false).text, "LIVE")
    tkTrue(liveBadge(100, 108, false).live)
    tkEqual(liveBadge(100, 183, false).text, "−1:23")
    tkEqual(liveBadge(100, 108, true).text, "−0:08", "paused at the edge")
    tkEqual(liveBadge(0, 0, false).text, "LIVE", "no window known")
    tkEqual(liveBadge(0, 0, true).text, "PAUSED")
    tkEqual(liveBehind(725), "12 min")
    tkEqual(liveBehind(59.6), "1:00")
end sub

sub test_live_view_kept()
    section = tkRegistrySection()
    key = liveViewKey("http://tv:7539", 2)
    tkEqual(liveLoadView(section, key), { tab: "guide", filter: "all" })
    liveSaveView(section, key, { tab: "channels", filter: "group:Sport" })
    tkEqual(liveLoadView(section, key), { tab: "channels", filter: "group:Sport" })
    section.Write(key, "{""tab"":""elsewhere"",""filter"":""group:""}")
    tkEqual(liveLoadView(section, key), { tab: "guide", filter: "all" })
    section.Write(key, "garbage")
    tkEqual(liveLoadView(section, key), { tab: "guide", filter: "all" })
end sub
