' Live TV (docs/API.md, "Live TV"), as the web app's page: GET /api/live
' (Recently watched), the lineup (GET /api/live/channels, each channel
' with what's on now and next) and the guide (GET /api/live/guide),
' fetched six hours at a time as it is scrolled to, two days at most. The
' API task keeps the lineup and the guide and builds what shows: an IPTV
' lineup can run to tens of thousands of channels, so past a thousand the
' guide shows a group (or the favourites) at a time (Live.brs's
' liveMany). The lineup comes again each minute (each ten past a
' thousand channels). The tab and the guide's filter are kept on the Roku
' for each server and profile.

sub init()
    m.count = m.top.findNode("count")
    m.tabs = m.top.findNode("tabs")
    m.filterChip = m.top.findNode("filterChip")
    m.info = m.top.findNode("info")
    m.infoLogo = m.top.findNode("infoLogo")
    m.infoChannel = m.top.findNode("infoChannel")
    m.infoTitle = m.top.findNode("infoTitle")
    m.infoMeta = m.top.findNode("infoMeta")
    m.infoText = m.top.findNode("infoText")
    m.guide = m.top.findNode("guide")
    m.rows = m.top.findNode("rows")
    m.message = m.top.findNode("message")
    m.hint = m.top.findNode("hint")
    m.panel = m.top.findNode("panel")
    m.choices = m.top.findNode("choices")
    m.tick = m.top.findNode("tick")
    m.started = false
    m.twelve = CreateObject("roDeviceInfo").GetClockFormat() <> "24h"
    m.view = { tab: "guide", filter: "all" }
    m.viewKey = ""
    ' The lineup's size (-1 until it came), the guide's filters ([{ value,
    ' label }]), the channels watched last, the Channels tab's row keys.
    m.total = -1
    m.filters = []
    m.recent = []
    m.rowKeys = []
    m.ticks = 0
    ' The guide: where it starts, how far it is fetched, the channels of
    ' its rows, what they are (to build it again only when they change),
    ' and which asking the answer must be for.
    m.guideStart = 0
    m.loadedTo = 0
    m.guideChannels = []
    m.guideSignature = ""
    m.guideNote = ""
    m.guideAsked = 0
    m.guideLoading = false
    m.channelsAsked = 0
    ' The filter panel's values.
    m.values = []
    m.lastFocus = m.tabs
    ' The focus goes into the tab's content once, as it first shows.
    m.focusedOnce = false
    m.tabs.chips = [{ label: "Guide", width: 220 }, { label: "Channels", width: 220 }]
    m.tabs.observeField("chosen", "onTabChosen")
    m.filterChip.observeField("chosen", "openFilter")
    m.choices.observeField("itemSelected", "onFilterChosen")
    m.guide.observeField("programFocusedDetails", "onProgramFocused")
    m.guide.observeField("programSelected", "onProgramSelected")
    m.guide.observeField("leftEdgeTargetTime", "onLeftEdge")
    m.rows.observeField("rowItemSelected", "onCardSelected")
    m.tick.observeField("fire", "onTick")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        m.tick.control = "stop"
        m.lastFocus = m.tabs
        if m.filterChip.isInFocusChain() then m.lastFocus = m.filterChip
        if m.guide.isInFocusChain() then m.lastFocus = m.guide
        if m.rows.isInFocusChain() then m.lastFocus = m.rows
        return
    end if
    m.tick.control = "start"
    if not m.started
        m.started = true
        session = m.global.session
        m.viewKey = liveViewKey(session.server, session.profileId)
        m.view = liveLoadView(viewSection(), m.viewKey)
        m.tabs.setFocus(true)
        showTab(m.view.tab)
        load()
        return
    end if
    if m.panel.visible
        m.choices.setFocus(true)
    else if m.lastFocus.visible
        m.lastFocus.setFocus(true)
    else
        m.tabs.setFocus(true)
    end if
    ' Back from watching: Recently watched and the favourites moved on;
    ' an hour on, the guide starts later.
    load()
end sub

function viewSection() as object
    return CreateObject("roRegistrySection", "liveViews")
end function

function nowSeconds() as integer
    return CreateObject("roDateTime").AsSeconds()
end function

sub onTick()
    if not m.top.active then return
    m.ticks = m.ticks + 1
    if m.total <= liveMany() or m.ticks mod 10 = 0 then load()
end sub

' --- The lineup ---

' GET /api/live for the channels watched last, then the lineup.
sub load()
    uiGet("/api/live", "onSummary")
end sub

sub onSummary(event as object)
    answer = event.getData()
    if not answer.ok
        if m.total < 0 and not uiFailed(answer) then showMessage(answer.message)
        return
    end if
    summary = httpObject(answer.data)
    if summary = invalid then summary = {}
    if GetInterface(summary.allowed, "ifBoolean") <> invalid and not summary.allowed
        showMessage("Live TV isn't open to this account." + Chr(10) + "An administrator can open it on the web, in Settings → Live TV.")
        return
    end if
    m.recent = httpArray(summary.recent)
    m.channelsAsked = m.channelsAsked + 1
    uiRequest({ op: "liveChannels", refresh: true, recent: m.recent, filter: m.view.filter, asked: m.channelsAsked }, "onChannels")
end sub

sub onChannels(event as object)
    request = event.getRoSGNode()
    if request.spec.asked <> m.channelsAsked then return
    answer = event.getData()
    if not answer.ok
        if m.total < 0 and not uiFailed(answer) then showMessage(answer.message)
        return
    end if
    data = answer.data
    m.total = data.count
    m.count.text = channelCount(m.total)
    if m.total = 0
        showMessage("No channels yet." + Chr(10) + "Channels appear here once an administrator adds an IPTV provider on the web, in Settings → Live TV.")
        return
    end if
    m.filters = data.choices
    m.view.filter = data.filter
    showFilterChip()
    m.filterChip.visible = m.view.tab = "guide"
    placeRows(request.content, data.rowKeys)
    ' The guide again only when its channels changed.
    changed = data.guideSignature <> m.guideSignature
    m.guideSignature = data.guideSignature
    if m.view.tab = "guide"
        showGuide(changed)
    else
        showContent()
    end if
end sub

function channelCount(n as integer) as string
    if n = 1 then return "1 channel"
    return thousands(n) + " channels"
end function

' "57,176".
function thousands(n as integer) as string
    text = n.ToStr()
    out = ""
    while Len(text) > 3
        out = "," + Right(text, 3) + out
        text = Left(text, Len(text) - 3)
    end while
    return text + out
end function

' The Channels tab's rows, the focus kept on the same card of the same row.
sub placeRows(content as dynamic, keys as object)
    if content = invalid then return
    focus = invalid
    if m.rows.content <> invalid and m.rowKeys.Count() > 0
        position = m.rows.rowItemFocused
        if position <> invalid and position.Count() = 2 and position[0] >= 0 and position[0] < m.rowKeys.Count()
            row = m.rows.content.getChild(position[0])
            card = invalid
            if row <> invalid then card = row.getChild(position[1])
            channelId = 0
            if card <> invalid then channelId = card.channelId
            focus = { key: m.rowKeys[position[0]], channelId: channelId, column: position[1] }
        end if
    end if
    m.rowKeys = keys
    m.rows.content = content
    if focus = invalid then return
    for i = 0 to keys.Count() - 1
        if keys[i] = focus.key
            row = content.getChild(i)
            column = focus.column
            for j = 0 to row.getChildCount() - 1
                if row.getChild(j).channelId = focus.channelId then column = j
            end for
            if column >= row.getChildCount() then column = row.getChildCount() - 1
            if column < 0 then column = 0
            m.rows.jumpToRowItem = [i, column]
            return
        end if
    end for
end sub

' --- Tabs ---

sub onTabChosen(event as object)
    wanted = "guide"
    if event.getData() = 1 then wanted = "channels"
    if wanted <> m.view.tab then showTab(wanted)
end sub

sub showTab(wanted as string)
    m.view.tab = wanted
    liveSaveView(viewSection(), m.viewKey, m.view)
    m.tabs.current = 0
    if wanted = "channels" then m.tabs.current = 1
    m.tabs.focusIndex = m.tabs.current
    if m.guide.isInFocusChain() or m.rows.isInFocusChain() then m.tabs.setFocus(true)
    m.guide.visible = false
    m.info.visible = false
    m.rows.visible = false
    m.filterChip.visible = wanted = "guide" and m.total > 0
    if m.total < 0
        showMessage("Loading…")
        return
    end if
    if m.total = 0 then return
    if wanted = "guide"
        showGuide(false)
    else
        showContent()
    end if
end sub

sub showMessage(text as string)
    m.guide.visible = false
    m.info.visible = false
    m.rows.visible = false
    m.hint.text = ""
    m.message.text = text
    m.message.visible = true
    if not m.panel.visible and m.top.isInFocusChain() then m.tabs.setFocus(true)
end sub

' The tab's content shows; the focus goes into it the first time.
sub showContent()
    m.message.visible = false
    if m.view.tab = "guide"
        if m.guide.content = invalid
            m.message.text = "Loading the guide…"
            m.message.visible = true
            m.hint.text = ""
            return
        end if
        m.guide.visible = true
        m.info.visible = true
        hint = "OK: watch   ·   Options (*): favourite   ·   Rewind and fast forward: earlier and later   ·   Replay: now"
        if m.guideNote <> "" then hint = m.guideNote + "   ·   " + hint
        m.hint.text = hint
    else
        m.rows.visible = true
        m.hint.text = "OK: watch   ·   Options (*): favourite"
    end if
    if m.top.isInFocusChain() and m.tabs.isInFocusChain() and not m.focusedOnce
        m.focusedOnce = true
        focusContent()
    end if
end sub

sub focusContent()
    if m.view.tab = "guide" and m.guide.visible
        m.guide.setFocus(true)
    else if m.view.tab = "channels" and m.rows.visible
        m.rows.setFocus(true)
    end if
end sub

' --- The guide ---

' The guide for the filter's channels: built again from what the task
' kept when they changed, fetched afresh when it starts later (an hour
' on); else as it is.
sub showGuide(changed as boolean)
    if m.total <= 0 then return
    start = liveGuideStart(nowSeconds())
    if start = m.guideStart and not changed and m.guide.content <> invalid
        showContent()
        return
    end if
    spec = { op: "liveGuide", start: start, from: 0, to: 0, filter: m.view.filter }
    if start <> m.guideStart or m.guide.content = invalid
        m.guideStart = start
        m.loadedTo = start
        m.guide.content = invalid
        spec.from = start
        spec.to = start + liveGuideChunk()
        showContent()
    end if
    askGuide(spec)
end sub

sub askGuide(spec as object)
    m.guideAsked = m.guideAsked + 1
    spec.asked = m.guideAsked
    m.guideLoading = spec.to > spec.from
    uiRequest(spec, "onGuide")
end sub

' The guide's content in place: the focus stays on the same channel, the
' time shown the same.
sub onGuide(event as object)
    request = event.getRoSGNode()
    if request.spec.asked <> m.guideAsked then return
    m.guideLoading = false
    answer = event.getData()
    if not answer.ok
        if m.guide.content = invalid and not uiFailed(answer) and m.view.tab = "guide" then showMessage(answer.message)
        return
    end if
    data = answer.data
    content = request.content
    if content = invalid then return
    if data.upto > m.loadedTo then m.loadedTo = data.upto
    channelId = invalid
    left = m.guideStart + 1800
    if m.guide.content <> invalid
        focused = focusedGuide()
        if focused <> invalid then channelId = focused.channel.id
        if m.guide.leftEdgeTargetTime > 0 then left = m.guide.leftEdgeTargetTime
    end if
    m.guideChannels = data.channels
    notes = []
    if data.total > data.channels.Count() then notes.Push("The first " + thousands(data.channels.Count()) + " of " + thousands(data.total) + " channels")
    ' A lineup this big shows what's on now and next only: past it, the
    ' rows end rather than say they are loading.
    if data.nowNext then notes.Push("Now and next")
    m.guide.automaticLoadingDataFeedback = not data.nowNext
    m.guideNote = notes.Join("   ·   ")
    at = liveIndexOf(m.guideChannels, channelId)
    if at < 0 then at = 0
    m.guide.contentStartTime = m.guideStart
    m.guide.content = content
    m.guide.jumpToChannel = at
    m.guide.leftEdgeTargetTime = left
    if m.view.tab = "guide" then showContent()
    showProgram()
end sub

' Scrolled near what is fetched: the next six hours.
sub onLeftEdge()
    if m.guideLoading or m.guide.content = invalid then return
    missing = liveGuideMissing(m.guideStart, m.loadedTo, m.guide.leftEdgeTargetTime, Int(m.guide.duration))
    if missing = invalid then return
    askGuide({ op: "liveGuide", start: m.guideStart, from: missing.from, to: missing.to, filter: m.view.filter })
end sub

sub onProgramFocused()
    showProgram()
end sub

' The channel and programme in focus: { channel, program } (program
' invalid for a channel without one there).
function focusedGuide() as dynamic
    content = m.guide.content
    if content = invalid then return invalid
    details = m.guide.programFocusedDetails
    channelAt = 0
    programAt = 0
    if details <> invalid
        channelAt = Int(httpNumber(details.focusChannelIndex, 0))
        programAt = Int(httpNumber(details.focusIndex, 0))
    end if
    if channelAt < 0 or channelAt >= m.guideChannels.Count() then return invalid
    row = content.getChild(channelAt)
    program = invalid
    if row <> invalid then program = row.getChild(programAt)
    return { channel: m.guideChannels[channelAt], program: program }
end function

' Above the guide: the channel, the programme's title, when it is on
' (and how long it has left, on now), its episode and what it is.
sub showProgram()
    focused = focusedGuide()
    if focused = invalid then return
    channel = focused.channel
    program = focused.program
    m.infoChannel.text = liveTitle(channel)
    m.infoLogo.uri = channel.logo
    if program = invalid or program.id = "gap"
        m.infoTitle.text = channel.name
        m.infoMeta.text = "No guide information"
        if not channel.hasGuide then m.infoMeta.text = "This channel has no guide"
        m.infoText.text = ""
        return
    end if
    now = nowSeconds()
    p = programTimes(program)
    meta = []
    if p.start <= now and p.ends > now
        meta.Push("On now")
        meta.Push(liveSpan(p, m.twelve))
        meta.Push(liveTimeLeft(p, now))
    else
        meta.Push(daySpan(p, now))
    end if
    if program.shortDescriptionLine1 <> "" then meta.Push(program.shortDescriptionLine1)
    if program.shortDescriptionLine2 <> "" then meta.Push(program.shortDescriptionLine2)
    m.infoTitle.text = program.title
    m.infoMeta.text = meta.Join("   ·   ")
    m.infoText.text = program.description
end sub

function programTimes(program as object) as object
    start = Int(program.playStart)
    return { start: start, ends: start + Int(program.playDuration) }
end function

' "Tomorrow 8:00 PM – 9:00 PM", the day left out for today.
function daySpan(p as object, now as integer) as string
    span = liveSpan(p, m.twelve)
    day = dayLabel(p.start, now)
    if day <> "" then span = day + " " + span
    return span
end function

' "Tomorrow", "Yesterday", a weekday, or "" for today, in the Roku's
' time zone.
function dayLabel(seconds as integer, now as integer) as string
    at = localDay(seconds)
    today = localDay(now)
    if at = today then return ""
    if at = today + 1 then return "Tomorrow"
    if at = today - 1 then return "Yesterday"
    when = CreateObject("roDateTime")
    when.FromSeconds(seconds)
    when.ToLocalTime()
    return when.GetWeekday()
end function

function localDay(seconds as integer) as integer
    when = CreateObject("roDateTime")
    when.FromSeconds(seconds)
    when.ToLocalTime()
    return Int(when.AsSeconds() / 86400)
end function

' OK on a programme: one on now (or time without a guide) watches the
' channel; another says what it is, with Watch.
sub onProgramSelected()
    focused = focusedGuide()
    if focused = invalid then return
    program = focused.program
    now = nowSeconds()
    if program <> invalid and program.id <> "gap"
        p = programTimes(program)
        if p.start > now or p.ends <= now
            channelOptions(focused.channel, program)
            return
        end if
    end if
    watch(focused.channel)
end sub

' --- Cards ---

sub onCardSelected(event as object)
    channel = cardChannel(event.getData())
    if channel <> invalid then watch(channel)
end sub

' The channel of a card, as much of it as the card has.
function cardChannel(position as dynamic) as dynamic
    if m.rows.content = invalid or position = invalid then return invalid
    row = m.rows.content.getChild(position[0])
    if row = invalid then return invalid
    card = row.getChild(position[1])
    if card = invalid then return invalid
    return { id: card.channelId, name: card.title, number: card.number, logo: card.image, favorite: card.favorite, hasGuide: true, group: "" }
end function

' --- Watching and favourites ---

sub watch(channel as object)
    uiNavigate({ to: "livePlayer", params: { channel: channel } })
end sub

' The Options key on a channel (or OK on a programme not on now): its
' name, the programme, Watch, and Add to or Remove from favourites.
sub channelOptions(channel as object, program as dynamic)
    m.optionsChannel = channel
    buttons = ["Watch " + channel.name]
    if channel.favorite
        buttons.Push("Remove from favourites")
    else
        buttons.Push("Add to favourites")
    end if
    buttons.Push("Cancel")
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = liveTitle(channel)
    if program <> invalid and program.id <> "gap"
        lines = [program.title, daySpan(programTimes(program), nowSeconds())]
        if program.description <> "" then lines.Push(program.description)
        dialog.message = lines
    end if
    dialog.buttons = buttons
    dialog.observeField("buttonSelected", "onOptionChosen")
    m.top.getScene().dialog = dialog
end sub

sub onOptionChosen(event as object)
    chosen = event.getData()
    channel = m.optionsChannel
    m.optionsChannel = invalid
    uiCloseDialog()
    if channel = invalid then return
    if chosen = 0
        watch(channel)
    else if chosen = 1
        setFavorite(channel, not channel.favorite)
    end if
end sub

' PUT or DELETE /api/live/channels/{id}/favorite; the lineup the API task
' keeps follows, and the rows and the guide with it.
sub setFavorite(channel as object, favorite as boolean)
    method = "DELETE"
    if favorite then method = "PUT"
    uiRequest({ method: method, path: "/api/live/channels/" + urlValue(channel.id) + "/favorite", channelId: channel.id, favorite: favorite }, "onFavorite")
end sub

sub onFavorite(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then uiMessage("That didn't work", [answer.message])
        return
    end if
    spec = event.getRoSGNode().spec
    uiRequest({ op: "liveFavorite", channelId: spec.channelId, favorite: spec.favorite }, "")
    load()
end sub

' --- The filter ---

sub showFilterChip()
    label = liveFilterLabel(m.view.filter)
    width = 120 + Len(label) * 15
    if width > 420 then width = 420
    m.filterChip.chips = [{ label: label, width: width }]
    m.filterChip.current = -1
    if m.view.filter <> "all" then m.filterChip.current = 0
    m.filterChip.translation = [1824 - width, 132]
end sub

sub openFilter()
    if m.filters.Count() = 0 then return
    items = []
    m.values = []
    current = 0
    for each choice in m.filters
        if choice.value = m.view.filter then current = items.Count()
        items.Push({ title: choice.label })
        m.values.Push(choice.value)
    end for
    m.choices.content = nodesContent(items)
    m.choices.checkedItem = current
    m.choices.jumpToItem = current
    m.panel.visible = true
    m.choices.setFocus(true)
end sub

sub onFilterChosen(event as object)
    at = event.getData()
    closePanel()
    if at < 0 or at >= m.values.Count() then return
    if m.values[at] = m.view.filter then return
    m.view.filter = m.values[at]
    liveSaveView(viewSection(), m.viewKey, m.view)
    showFilterChip()
    showGuide(true)
end sub

sub closePanel()
    m.panel.visible = false
    m.filterChip.setFocus(true)
end sub

' Down from the chips goes to the guide or the cards, up and Back from
' them to the chips (the guide goes round at its ends); right from the
' tabs, the guide's filter. The Options key on a programme or a card: the
' channel's options.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.panel.visible
        if key = "back" or key = "left"
            closePanel()
            return true
        end if
        return false
    end if
    chips = m.tabs.isInFocusChain() or m.filterChip.isInFocusChain()
    if key = "down" and chips
        focusContent()
        return true
    end if
    if (key = "back" or key = "up") and (m.guide.isInFocusChain() or m.rows.isInFocusChain())
        m.tabs.setFocus(true)
        return true
    end if
    if key = "right" and m.tabs.isInFocusChain() and m.filterChip.visible
        m.filterChip.focusIndex = 0
        m.filterChip.setFocus(true)
        return true
    end if
    if key = "left" and m.filterChip.isInFocusChain()
        m.tabs.setFocus(true)
        return true
    end if
    if key = "options" and m.guide.isInFocusChain()
        focused = focusedGuide()
        if focused <> invalid then channelOptions(focused.channel, invalid)
        return true
    end if
    if key = "options" and m.rows.isInFocusChain()
        channel = cardChannel(m.rows.rowItemFocused)
        if channel <> invalid then channelOptions(channel, invalid)
        return true
    end if
    return false
end function
