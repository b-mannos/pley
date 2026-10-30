' The lab's channels, as GET /api/live/browse lists them.
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

' A provider's groups, as GET /api/live/groups answers.
function liveSampleGroups() as object
    return {
        total: 56171, favorites: 2, recent: 9,
        groups: [
            { name: "UK| Sport", region: "UK", label: "Sport", count: 120 },
            { name: "UK| News", region: "UK", label: "News", count: 40 },
            { name: "News", region: invalid, label: "News", count: 2 },
            { name: "FR| Cinema", region: "FR", label: "Cinema", count: 30 },
            { label: "No name", count: 3 }
        ]
    }
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
    tkFalse(channels[2].DoesExist("now"), "left out, not invalid: a node's field doesn't keep invalid")
    tkEqual(liveTitle(news), "101  Pley News HD")
    tkEqual(liveTitle({ number: "", name: "Radio" }), "Radio")
    tkInvalid(liveProgram({ start: 10, stop: 10, title: "zero" }), "a programme that ends as it starts")
    page = livePage({ total: 120, offset: 50, channels: liveSample() }, "", "all")
    tkEqual(page.total, 120)
    tkEqual(page.offset, 50)
    tkEqual(page.channels.Count(), 3)
end sub

sub test_live_regions_and_short_names()
    tkEqual(liveSplitRegion("UK| Sky Sports 1"), ["UK", "Sky Sports 1"])
    tkEqual(liveSplitRegion("[FR] TF1 HD"), ["FR", "TF1 HD"])
    tkEqual(liveSplitRegion("DE: Das Erste"), ["DE", "Das Erste"])
    tkEqual(liveSplitRegion("||UK| BBC One"), ["UK", "BBC One"])
    tkInvalid(liveSplitRegion("BBC One"))
    tkInvalid(liveSplitRegion("Movies: The Best"), "a colon after more than a code")
    tkEqual(liveShortName("UK| Sky Sports 1", "UK| Sport", "group:UK| Sport"), "Sky Sports 1")
    tkEqual(liveShortName("UK| Sky Sports 1", "UK| Sport", "all"), "UK| Sky Sports 1", "every channel: the whole name")
    tkEqual(liveShortName("US| ESPN", "UK| Sport", "group:UK| Sport"), "US| ESPN", "another region stays")
    tkEqual(liveShortName("BBC One", "News", "group:News"), "BBC One")
    channel = liveChannel({ id: 9, name: "UK| BBC News", group: "UK| News" }, "", 240, "group:UK| News")
    tkEqual(channel.name, "BBC News")
    tkEqual(liveThousands(57176), "57,176")
    tkEqual(liveThousands(999), "999")
    tkEqual(liveThousands(1234567), "1,234,567")
end sub

sub test_live_categories()
    tkEqual(liveBrowseQuery("all", 0, 50), { category: "all", offset: 0, limit: 50 })
    tkEqual(liveBrowseQuery("favorites", 50, 50), { category: "favorites", offset: 50, limit: 50 })
    tkEqual(liveBrowseQuery("group:UK| Sport", 0, 50), { category: "group", group: "UK| Sport", offset: 0, limit: 50 })
    groups = liveGroups(liveSampleGroups())
    tkEqual(groups.Count(), 4, "one without a name is left out")
    tkEqual(groups[2].region, "")
    tkEqual(liveCategoryName("group:UK| Sport", groups), "UK · Sport")
    tkEqual(liveCategoryName("group:News", groups), "News")
    tkEqual(liveCategoryName("recent", groups), "Recently watched")
    tkEqual(liveCategoryName("all", groups), "All channels")
    summary = liveSummary(liveSampleGroups())
    tkEqual(summary, { total: 56171, favorites: 2, recent: 9 })
    tkEqual(liveCategoryValid("group:UK| News", summary, groups), "group:UK| News")
    tkEqual(liveCategoryValid("group:Kids", summary, groups), "all", "a group gone")
    tkEqual(liveCategoryValid("favorites", { total: 5, favorites: 0, recent: 0 }, groups), "all", "nothing marked any more")
    tkEqual(liveRegionOf("group:UK| News", groups), "UK")
    tkEqual(liveRegionOf("all", groups), "")
end sub

sub test_live_rail_gathers_regions()
    groups = liveGroups(liveSampleGroups())
    summary = liveSummary(liveSampleGroups())
    rail = liveRail(summary, groups, {})
    keys = []
    for each entry in rail
        keys.Push(entry.key)
    end for
    tkEqual(keys, ["favorites", "recent", "all", "UK", "group:News", "group:FR| Cinema"], "a region of one group stays a group")
    tkEqual(rail[3].kind, "region")
    tkEqual(rail[3].label, "UK  ·  2 groups")
    tkEqual(rail[3].count, 160)
    tkEqual(rail[2].count, 56171)
    opened = liveRail(summary, groups, { UK: true })
    tkEqual(opened.Count(), 8)
    tkEqual(opened[4], { kind: "category", key: "group:UK| Sport", label: "Sport", count: 120, inner: true })
    tkEqual(liveRail(summary, [groups[2]], {}).Count(), 3, "one group: no groups listed")
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

' A page of channels with ids from first, of a list of total.
function liveSamplePage(offset as integer, count as integer, total as integer) as object
    channels = []
    for i = 1 to count
        channels.Push({ id: offset + i, name: "c" })
    end for
    return { total: total, offset: offset, channels: channels }
end function

sub test_live_zapping_pages_round_the_ends()
    one = liveSamplePage(0, 3, 3)
    tkEqual(liveZap(one, 1, 1).channel.id, 2)
    tkEqual(liveZap(one, 3, 1).channel.id, 1, "after the last, the first")
    tkEqual(liveZap(one, 1, -1).channel.id, 3, "before the first, the last")
    tkEqual(liveZap(one, 99, 1).channel.id, 1, "a channel not in the page")
    tkInvalid(liveZap(liveSamplePage(0, 0, 0), 1, 1))
    size = livePageSize()
    middle = liveSamplePage(size, size, size * 3 + 7)
    tkEqual(liveZap(middle, size + 2, -1).channel.id, size + 1)
    tkEqual(liveZap(middle, size * 2, 1), { offset: size * 2, last: false }, "the next page's first")
    tkEqual(liveZap(middle, size + 1, -1), { offset: 0, last: true }, "the page before's last")
    first = liveSamplePage(0, size, size * 3 + 7)
    tkEqual(liveZap(first, 1, -1), { offset: size * 3, last: true }, "before the first page, the last")
    last = liveSamplePage(size * 3, 7, size * 3 + 7)
    tkEqual(liveZap(last, size * 3 + 7, 1), { offset: 0, last: false }, "after the last page, the first")
    tkEqual(liveRange(50, 50, 2345), "Channels 51–100 of 2,345")
    tkEqual(liveRange(0, 5, 5), "")
end sub

sub test_live_cards_and_list_items()
    channels = liveChannels(liveSample(), "")
    card = liveCard(channels[0], 1500, false)
    tkEqual(card.line, "Morning Briefing")
    tkEqual(card.progress, 0.5)
    tkTrue(Instr(1, card.nextLine, " · World at One") > 0, "what's next, with its time")
    tkEqual(liveCard(channels[2], 1500, false).line, "No guide")
    tkEqual(liveCard(channels[1], 2500, false).line, "Nothing in the guide now")
    item = liveListItem(channels[0], 2100, false)
    tkEqual(item.line, "World at One")
    tkEqual(liveListItem(channels[0], 900, false).progress, 0)
    tkTrue(Instr(1, liveListItem(channels[0], 900, false).line, " · Morning Briefing") > 0, "what comes on, with its time")
    tkEqual(liveListItem(channels[2], 1500, false).line, "")
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
    tkEqual(rows[0].programs[4].title, "No guide information")
    tkEqual(rows[0].programs[4].playDuration, 200)
    tkEqual(rows[2].programs.Count(), 1, "no guide: one gap over what is fetched")
    tkEqual(rows[2].programs[0].title, "Film Four", "a channel without a guide: its name across it")
    kept = {}
    liveGuideKeep(kept, { "1": [{ start: 1000, stop: 1030, title: "A" }, { start: 1100, stop: 1200, title: "B" }] })
    programs = liveGuideRows([{ id: 1, number: "", name: "x", logo: "", hasGuide: true }], kept, 900, 1200)[0].programs
    tkEqual(programs.Count(), 4, "gaps of a minute or more, before and between")
    tkEqual(programs[0].playDuration, 100)
    tkEqual(programs[2].playStart, 1030)
    tkEqual(liveIds(liveChannels(liveSample(), "")), "1,2,3")
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

sub test_live_badge_and_seeking()
    tkEqual(liveBadge(100, 108, false).text, "LIVE")
    tkTrue(liveBadge(100, 108, false).live)
    tkEqual(liveBadge(100, 183, false).text, "−1:23")
    tkEqual(liveBadge(100, 108, true).text, "−0:08", "paused at the edge")
    tkEqual(liveBadge(0, 0, false).text, "LIVE", "no window known")
    tkEqual(liveBadge(0, 0, true).text, "PAUSED")
    tkEqual(liveBehind(725), "12 min")
    tkEqual(liveBehind(59.6), "1:00")
    tkEqual(liveSeekTarget(100, 600, -10), 90)
    tkEqual(liveSeekTarget(5, 600, -10), 0, "not before the window")
    tkEqual(liveSeekTarget(590, 600, 10), 594, "not past what plays live")
end sub

sub test_live_view_kept()
    section = tkRegistrySection()
    key = liveViewKey("http://tv:7539", 2)
    tkEqual(liveLoadView(section, key), { tab: "guide", category: "all" })
    liveSaveView(section, key, { tab: "channels", category: "group:UK| Sport" })
    tkEqual(liveLoadView(section, key), { tab: "channels", category: "group:UK| Sport" })
    section.Write(key, "{""tab"":""elsewhere"",""category"":""group:""}")
    tkEqual(liveLoadView(section, key), { tab: "guide", category: "all" })
    section.Write(key, "garbage")
    tkEqual(liveLoadView(section, key), { tab: "guide", category: "all" })
end sub
