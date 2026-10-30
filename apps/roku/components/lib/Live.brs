' Live TV (docs/API.md, "Live TV"): the channels and their guide as the
' screens show them, what the player says, and where it goes on up and
' down. Times are unix seconds, as the server sends them.

' Logos are asked at about the size they're drawn.
function liveLogoWidth() as integer
    return 240
end function

' The guide is fetched this many seconds at a time as it is scrolled to,
' this far ahead at most (the web app's chunks and span).
function liveGuideChunk() as integer
    return 6 * 3600
end function

function liveGuideSpan() as integer
    return 48 * 3600
end function

' Where the guide starts: the half hour before the one now is in, so the
' programme on now shows whole.
function liveGuideStart(now as integer) as integer
    return Int(now / 1800) * 1800 - 1800
end function

' A programme as the screens keep it: { start, ends, title, line,
' description, badges, live, isNew }, or invalid for none (or one that
' ends before it starts).
function liveProgram(raw as dynamic) as dynamic
    p = httpObject(raw)
    if p = invalid then return invalid
    start = Int(httpNumber(p.start, 0))
    ends = Int(httpNumber(p["stop"], 0))
    if ends <= start then return invalid
    title = httpSentence(p.title)
    if title = "" then title = "Untitled"
    line = []
    episode = fmtEpisode(p.season, p.episode)
    if episode <> "" then line.Push(episode)
    subtitle = httpSentence(p.subtitle)
    if subtitle <> "" and subtitle <> title then line.Push(subtitle)
    badges = []
    if httpTrue(p.is_live) then badges.Push("Live")
    if httpTrue(p.is_new) then badges.Push("New")
    rating = httpSentence(p.rating)
    if rating <> "" then badges.Push(rating)
    return {
        start: start,
        ends: ends,
        title: title,
        line: line.Join(" · "),
        description: httpClean(p.description, 600, false),
        badges: badges.Join(" · "),
        live: httpTrue(p.is_live),
        isNew: httpTrue(p.is_new)
    }
end function

' A channel as the screens keep it: { id, name, number, group, logo,
' favorite, hasGuide, now, after }, its logo's address on server at
' width (now and after left out for none); invalid without an id.
function liveChannel(raw as dynamic, server as string, width = 240 as integer) as dynamic
    c = httpObject(raw)
    if c = invalid then return invalid
    id = Int(httpNumber(c.id, 0))
    if id <= 0 then return invalid
    name = httpSentence(c.name)
    if name = "" then name = "Channel " + id.ToStr()
    channel = {
        id: id,
        name: name,
        number: httpSentence(c.number),
        group: httpSentence(c.group),
        logo: urlImage(server, c.logo, width),
        favorite: httpTrue(c.favorite),
        hasGuide: httpTrue(c.has_guide)
    }
    ' Left out when there are none: a node's field doesn't keep invalid,
    ' and channels go from the API task to the screens in one.
    on = liveProgram(c.now)
    if on <> invalid then channel.now = on
    coming = liveProgram(c["next"])
    if coming <> invalid then channel.after = coming
    return channel
end function

' The channels of GET /api/live/channels, in its order (by number).
function liveChannels(list as dynamic, server as string) as object
    out = []
    for each raw in httpArray(list)
        channel = liveChannel(raw, server)
        if channel <> invalid then out.Push(channel)
    end for
    return out
end function

' "101  Pley News HD", or the name alone without a number.
function liveTitle(channel as object) as string
    if channel.number = "" then return channel.name
    return channel.number + "  " + channel.name
end function

' What's on a channel at now: its now, or the one after once that has begun;
' invalid when the guide has nothing on now.
function liveOnNow(channel as object, now as integer) as dynamic
    for each p in [channel.now, channel.after]
        if p <> invalid and p.start <= now and p.ends > now then return p
    end for
    return invalid
end function

' What comes on next: the first of now and after that hasn't begun.
function liveComingUp(channel as object, now as integer) as dynamic
    for each p in [channel.now, channel.after]
        if p <> invalid and p.start > now then return p
    end for
    return invalid
end function

' How far into p now is, from 0 to 1.
function liveProgress(p as dynamic, now as integer) as float
    if p = invalid then return 0
    length = p.ends - p.start
    if length <= 0 then return 0
    done = (now - p.start) / length
    if done < 0 then return 0
    if done > 1 then return 1
    return done
end function

' "12 min left", "1 h 5 min left", "2 h left".
function liveTimeLeft(p as object, now as integer) as string
    minutes = Int((p.ends - now + 30) / 60)
    if minutes < 1 then minutes = 1
    if minutes < 60 then return minutes.ToStr() + " min left"
    hours = Int(minutes / 60)
    minutes = minutes mod 60
    if minutes = 0 then return hours.ToStr() + " h left"
    return hours.ToStr() + " h " + minutes.ToStr() + " min left"
end function

' "8:05 PM" on a Roku set to a 12-hour clock, "20:05" on a 24-hour one.
function liveClockText(hours as integer, minutes as integer, twelve as boolean) as string
    if not twelve then return fmtTwo(hours) + ":" + fmtTwo(minutes)
    suffix = " AM"
    if hours >= 12 then suffix = " PM"
    shown = hours mod 12
    if shown = 0 then shown = 12
    return shown.ToStr() + ":" + fmtTwo(minutes) + suffix
end function

' A time in the Roku's own time zone.
function liveClock(seconds as integer, twelve as boolean) as string
    at = CreateObject("roDateTime")
    at.FromSeconds(seconds)
    at.ToLocalTime()
    return liveClockText(at.GetHours(), at.GetMinutes(), twelve)
end function

' "8:00 PM – 9:30 PM".
function liveSpan(p as object, twelve as boolean) as string
    return liveClock(p.start, twelve) + " – " + liveClock(p.ends, twelve)
end function

' The groups channels are in, in their order: [{ name, count }].
function liveGroups(channels as object) as object
    counts = {}
    order = []
    for each channel in channels
        if channel.group <> ""
            key = "g" + channel.group
            if not counts.DoesExist(key)
                counts[key] = 0
                order.Push(channel.group)
            end if
            counts[key] = counts[key] + 1
        end if
    end for
    out = []
    for each name in order
        out.Push({ name: name, count: counts["g" + name] })
    end for
    return out
end function

' How many channels a lineup can have for a Roku to show them all at
' once: past it, the guide and the player's list show a group (or the
' favourites) at a time. IPTV lineups run to tens of thousands.
function liveMany() as integer
    return 1000
end function

' How old a kept lineup may be before it is fetched again: a minute
' (what's on moves on), ten past liveMany channels, whose lineup is
' megabytes.
function liveRefreshMs(count as integer) as integer
    if count > liveMany() then return 600000
    return 55000
end function

' The most channels the guide, the player's list or a row of cards shows,
' and the most cards the Channels tab makes in all.
function liveShownMax() as integer
    return 1000
end function

function liveRowMax() as integer
    return 100
end function

function liveCardBudget() as integer
    return 2000
end function

' The guide's filters: "all", "favorites" or "group:<name>", with labels
' that count their channels: [{ value, label }]. All channels only for a
' lineup of liveMany or fewer, favourites only when some are, groups only
' when there are more than one.
function liveFilterChoices(channels as object) as object
    out = []
    if channels.Count() <= liveMany() then out.Push({ value: "all", label: "All channels · " + channels.Count().ToStr() })
    favorites = 0
    for each channel in channels
        if channel.favorite then favorites = favorites + 1
    end for
    if favorites > 0 then out.Push({ value: "favorites", label: "Favourites · " + favorites.ToStr() })
    groups = liveGroups(channels)
    if groups.Count() > 1 or out.Count() = 0
        for each group in groups
            out.Push({ value: "group:" + group.name, label: group.name + " · " + group.count.ToStr() })
        end for
    end if
    if out.Count() = 0 then out.Push({ value: "all", label: "All channels · " + channels.Count().ToStr() })
    return out
end function

' The filter as it can be: a group gone, or favourites none any more,
' shows the first there is (every channel, where they aren't too many).
function liveFilterValid(channels as object, filter as string) as string
    choices = liveFilterChoices(channels)
    for each choice in choices
        if choice.value = filter then return filter
    end for
    return choices[0].value
end function

' The chip's label: "All channels", "Favourites", or the group's name.
function liveFilterLabel(filter as string) as string
    if filter = "favorites" then return "Favourites"
    if Left(filter, 6) = "group:" then return Mid(filter, 7)
    return "All channels"
end function

' The channels the filter shows, in order.
function liveFiltered(channels as object, filter as string) as object
    if filter = "all" then return channels
    out = []
    group = ""
    if Left(filter, 6) = "group:" then group = Mid(filter, 7)
    for each channel in channels
        if filter = "favorites"
            if channel.favorite then out.Push(channel)
        else if channel.group = group
            out.Push(channel)
        end if
    end for
    return out
end function

' Where a channel is in channels, or -1.
function liveIndexOf(channels as object, id as dynamic) as integer
    for i = 0 to channels.Count() - 1
        if channels[i].id = id then return i
    end for
    return -1
end function

' The channel by steps from the one with id (1: the next number), round
' the ends; invalid without channels. A channel not in the list steps
' from the first.
function liveStep(channels as object, id as dynamic, by as integer) as dynamic
    count = channels.Count()
    if count = 0 then return invalid
    at = liveIndexOf(channels, id)
    if at < 0 then return channels[0]
    return channels[((at + by) mod count + count) mod count]
end function

' A channel's card (components/items/LiveCard): its logo and number, the
' name, what's on with how far along, and what's next.
function liveCard(channel as object, now as integer, twelve as boolean) as object
    on = liveOnNow(channel, now)
    coming = liveComingUp(channel, now)
    line = "No guide"
    if channel.hasGuide then line = "Nothing in the guide now"
    progress = 0.0
    if on <> invalid
        line = on.title
        progress = liveProgress(on, now)
    end if
    nextLine = ""
    if coming <> invalid then nextLine = liveClock(coming.start, twelve) + " · " + coming.title
    return {
        channelId: channel.id,
        title: channel.name,
        number: channel.number,
        image: channel.logo,
        favorite: channel.favorite,
        line: line,
        progress: progress,
        nextLine: nextLine
    }
end function

' A channel in the player's channel list (components/items/LiveChannelRow):
' what's on with how far along, else what comes on next.
function liveListItem(channel as object, now as integer, twelve as boolean) as object
    on = liveOnNow(channel, now)
    line = ""
    progress = 0.0
    if on <> invalid
        line = on.title
        progress = liveProgress(on, now)
    else
        coming = liveComingUp(channel, now)
        if coming <> invalid then line = liveClock(coming.start, twelve) + " · " + coming.title
    end if
    return { channelId: channel.id, number: channel.number, title: channel.name, image: channel.logo, line: line, progress: progress, favorite: channel.favorite, current: false }
end function

' The Channels tab's rows: Recently watched (the last 12, newest first),
' Favourites, then a row for each group (one row, "All channels", when
' they are in one group or none): [{ key, title, cards }]. A row has
' liveRowMax cards at most, and rows stop at liveCardBudget cards: the
' guide's filter reaches the rest.
function liveRows(channels as object, recent as dynamic, now as integer, twelve as boolean) as object
    rows = []
    byId = {}
    favorites = []
    byGroup = {}
    groupOrder = []
    others = []
    for each channel in channels
        byId["c" + channel.id.ToStr()] = channel
        if channel.favorite and favorites.Count() < liveRowMax() then favorites.Push(channel)
        if channel.group = ""
            if others.Count() < liveRowMax() then others.Push(channel)
        else
            key = "g" + channel.group
            if not byGroup.DoesExist(key)
                byGroup[key] = []
                groupOrder.Push(channel.group)
            end if
            if byGroup[key].Count() < liveRowMax() then byGroup[key].Push(channel)
        end if
    end for
    picked = []
    for each id in httpArray(recent)
        key = "c" + urlValue(id)
        if byId.DoesExist(key) and picked.Count() < 12 then picked.Push(byId[key])
    end for
    lists = []
    if picked.Count() > 0 then lists.Push({ key: "recent", title: "Recently watched", channels: picked })
    if favorites.Count() > 0 then lists.Push({ key: "favorites", title: "Favourites", channels: favorites })
    if groupOrder.Count() > 1
        for each name in groupOrder
            lists.Push({ key: "group:" + name, title: name, channels: byGroup["g" + name] })
        end for
        if others.Count() > 0 then lists.Push({ key: "other", title: "Other channels", channels: others })
    else if channels.Count() > 0
        all = []
        for each channel in channels
            if all.Count() < liveRowMax() then all.Push(channel)
        end for
        lists.Push({ key: "all", title: "All channels", channels: all })
    end if
    made = 0
    for each list in lists
        if made >= liveCardBudget() then exit for
        cards = []
        for each channel in list.channels
            cards.Push(liveCard(channel, now, twelve))
        end for
        made = made + cards.Count()
        rows.Push({ key: list.key, title: list.title, cards: cards })
    end for
    return rows
end function

' --- The guide ---

' Programmes of GET /api/live/guide added to kept ({ "<channel id>": {
' "<start>": programme } }), each once.
sub liveGuideKeep(kept as object, programs as dynamic)
    byChannel = httpObject(programs)
    if byChannel = invalid then return
    for each id in byChannel
        list = httpArray(byChannel[id])
        if list.Count() > 0
            key = "c" + id
            if not kept.DoesExist(key) then kept[key] = {}
            known = kept[key]
            for each raw in list
                p = liveProgram(raw)
                if p <> invalid then known["s" + p.start.ToStr()] = p
            end for
        end if
    end for
end sub

' A channel's programmes kept, by start, none overlapping the one before
' (the TimeGrid draws nothing of programmes that overlap).
function liveGuidePrograms(kept as object, channelId as integer) as object
    known = kept["c" + channelId.ToStr()]
    if known = invalid then return []
    list = []
    for each key in known
        list.Push(known[key])
    end for
    list.SortBy("start")
    out = []
    lastStop = 0
    for each p in list
        if p.start >= lastStop
            out.Push(p)
            lastStop = p.ends
        else if p.ends > lastStop
            ' Overlapping the one before: from where that one stops.
            cut = {}
            cut.Append(p)
            cut.start = lastStop
            out.Push(cut)
            lastStop = p.ends
        end if
    end for
    return out
end function

' The TimeGrid's channels for channels: [{ fields, programs }], fields
' the channel's ContentNode's (its name as the guide writes it when the
' logo doesn't show, the logo, its id) and programs its children's
' (TITLE, PLAYSTART, PLAYDURATION; the details as the screen above the
' grid shows them). Between from and upto (what is fetched), time the
' guide says nothing of is a programme too, "No guide information" (id
' "gap"): the TimeGrid would say it is still loading.
function liveGuideRows(channels as object, kept as object, from as integer, upto as integer) as object
    out = []
    for each channel in channels
        programs = []
        at = from
        for each p in liveGuidePrograms(kept, channel.id)
            if p.start - at >= 60 then programs.Push(liveGuideGap(at, p.start))
            programs.Push({
                title: p.title,
                playStart: p.start,
                playDuration: p.ends - p.start,
                description: p.description,
                shortDescriptionLine1: p.line,
                shortDescriptionLine2: p.badges
            })
            if p.ends > at then at = p.ends
        end for
        if upto - at >= 60 then programs.Push(liveGuideGap(at, upto))
        out.Push({ fields: { id: channel.id.ToStr(), title: liveTitle(channel), hdSmallIconUrl: channel.logo }, programs: programs })
    end for
    return out
end function

' The guide of a lineup past liveMany channels: what's on now and next of
' each, from the lineup. GET /api/live/guide gives every channel's guide
' at once, tens of megabytes for such a lineup. A channel without either
' says so for six hours.
function liveNowNextRows(channels as object, from as integer) as object
    kept = {}
    for each channel in channels
        known = {}
        for each p in [channel.now, channel.after]
            if p <> invalid and p.ends > from then known["s" + p.start.ToStr()] = p
        end for
        kept["c" + channel.id.ToStr()] = known
    end for
    rows = liveGuideRows(channels, kept, from, from)
    for each row in rows
        if row.programs.Count() = 0 then row.programs.Push(liveGuideGap(from, from + liveGuideChunk()))
    end for
    return rows
end function

function liveGuideGap(from as integer, upto as integer) as object
    return { id: "gap", title: "No guide information", playStart: from, playDuration: upto - from, description: "", shortDescriptionLine1: "", shortDescriptionLine2: "" }
end function

' The part of the guide to fetch for the time shown (left edge and the
' width shown, in seconds), with loadedTo fetched so far from start:
' { from, to }, or invalid when what is fetched covers it, with an hour to
' spare, or the guide's span is reached.
function liveGuideMissing(start as integer, loadedTo as integer, leftEdge as integer, shown as integer) as dynamic
    last = start + liveGuideSpan()
    if loadedTo >= last then return invalid
    if leftEdge + shown + 3600 <= loadedTo then return invalid
    upto = loadedTo + liveGuideChunk()
    if upto > last then upto = last
    return { from: loadedTo, to: upto }
end function

' --- The player ---

' A watching that didn't start, as the player says it: { kind, title,
' message, retry }. kind "busy" (every connection of the source in use,
' 409, the server's message naming what is watched), "missing" (404),
' "closed" (403), "stopped" (410), "unavailable" (503) or "other".
function liveTrouble(answer as object, channelName as string) as object
    status = answer.status
    message = httpSentence(answer.message)
    if status = 409 then return { kind: "busy", title: "All connections are in use", message: message, retry: true }
    if status = 404 then return { kind: "missing", title: "This channel isn't here any more", message: "It may have been hidden, or its source removed.", retry: false }
    if status = 403 then return { kind: "closed", title: "Live TV isn't open to this account", message: "An administrator can open it in Settings → Live TV, on the web.", retry: false }
    if status = 410 then return { kind: "stopped", title: "Watching stopped", message: message, retry: false }
    if status = 503 then return { kind: "unavailable", title: channelName + " isn't playing", message: message, retry: true }
    return { kind: "other", title: channelName + " didn't start", message: message, retry: true }
end function

' Seconds behind the live edge still counted as live: the Video node
' plays a few segments back from it.
function liveWithin() as integer
    return 15
end function

' "1:23" behind live, "12 min" from ten minutes.
function liveBehind(seconds as dynamic) as string
    s = Int(httpNumber(seconds, 0) + 0.5)
    if s < 0 then s = 0
    if s >= 600 then return Int((s + 30) / 60).ToStr() + " min"
    return Int(s / 60).ToStr() + ":" + fmtTwo(s mod 60)
end function

' The Live badge: "LIVE" within liveWithin of the edge, else how far
' behind ("−1:23"). duration and position are the Video node's for the
' stream's window; without a duration, live unless paused.
function liveBadge(position as dynamic, duration as dynamic, paused as boolean) as object
    behind = 0
    length = httpNumber(duration, 0)
    if length > 0 then behind = length - httpNumber(position, 0)
    if behind < 0 then behind = 0
    if not paused and behind < liveWithin() then return { live: true, text: "LIVE", behind: behind }
    if length <= 0 then return { live: false, text: "PAUSED", behind: 0 }
    return { live: false, text: "−" + liveBehind(behind), behind: behind }
end function

' The registry key of the Live TV screen's tab and filter for a server
' and profile.
function liveViewKey(server as string, profileId as dynamic) as string
    return server + "|" + urlValue(profileId)
end function

' { tab: "guide" | "channels", filter } kept for key, or the defaults.
function liveLoadView(section as object, key as string) as object
    view = { tab: "guide", filter: "all" }
    saved = httpObject(httpJson(section.Read(key)))
    if saved = invalid then return view
    if httpString(saved.tab) = "channels" then view.tab = "channels"
    filter = httpString(saved.filter)
    if filter = "favorites" or (Left(filter, 6) = "group:" and Len(filter) > 6) then view.filter = filter
    return view
end function

sub liveSaveView(section as object, key as string, view as object)
    section.Write(key, FormatJson({ tab: view.tab, filter: view.filter }))
    section.Flush()
end sub
