sub init()
    m.backdrop = m.top.findNode("backdrop")
    m.shade = m.top.findNode("shade")
    m.page = m.top.findNode("page")
    m.poster = m.top.findNode("poster")
    m.column = m.top.findNode("column")
    m.text = m.top.findNode("text")
    m.heading = m.top.findNode("heading")
    m.context = m.top.findNode("context")
    m.line = m.top.findNode("line")
    m.left = m.top.findNode("left")
    m.genres = m.top.findNode("genres")
    m.credits = m.top.findNode("credits")
    m.tagline = m.top.findNode("tagline")
    m.summary = m.top.findNode("summary")
    m.note = m.top.findNode("note")
    m.buttons = m.top.findNode("buttons")
    m.lower = m.top.findNode("lower")
    m.tabs = m.top.findNode("tabs")
    m.episodes = m.top.findNode("episodes")
    m.episodesNote = m.top.findNode("episodesNote")
    m.rows = m.top.findNode("rows")
    m.infoBlock = m.top.findNode("infoBlock")
    m.infoTitle = m.top.findNode("infoTitle")
    m.infoMeta = m.top.findNode("infoMeta")
    m.infoEpisode = m.top.findNode("infoEpisode")
    m.infoEpisodeLine = m.top.findNode("infoEpisodeLine")
    m.infoSummary = m.top.findNode("infoSummary")
    m.message = m.top.findNode("message")
    m.messageTitle = m.top.findNode("messageTitle")
    m.messageText = m.top.findNode("messageText")
    m.messageButton = m.top.findNode("messageButton")
    m.stage = m.top.findNode("stage")
    m.stageLine = m.top.findNode("stageLine")
    m.requestPoll = m.top.findNode("requestPoll")
    m.tabRest = m.top.findNode("tabRest")
    m.episodeRest = m.top.findNode("episodeRest")
    m.panel = m.top.findNode("panel")
    m.panelTitle = m.top.findNode("panelTitle")
    m.menu = m.top.findNode("menu")
    m.choices = m.top.findNode("choices")
    m.info = m.top.findNode("info")
    ' What the panel shows: "" (closed), a menu ("more", "tracks"), the
    ' choices of one of its entries ("rating", "version", "audio",
    ' "subtitles") or the media info ("info"); the menu Back goes back to
    ' ("" for none), and the values of the choices.
    m.panelKind = ""
    m.panelBack = ""
    m.panelValues = []
    ' Audio and subtitles' entries, in order; the version chosen (its file's
    ' id, invalid for the first), which isn't saved, as on the other apps;
    ' the audio picked on this page (-1 Automatic, -2 none), which subtitles
    ' picked after are saved with.
    m.trackKinds = []
    m.fileId = invalid
    m.audioPick = -2
    m.started = false
    ' What the server says of requests for the title (GET
    ' /api/items/{id}/request), invalid for nothing to show; the asks (an
    ' answer to an earlier one is left out); where Request more seasons
    ' goes (its tile's fields) while the person may ask for some.
    m.itemRequest = invalid
    m.requestAsks = 0
    m.askMore = invalid
    ' The item's JSON, which the player starts with, and what it shows; a
    ' show's or a season's episode to play next, once the server said
    ' (invalid for none); the titles of the collections it is part of.
    m.item = invalid
    m.detail = invalid
    m.onDeck = invalid
    m.onDeckKnown = false
    m.partOf = []
    ' The seasons of the tabs ([{ itemId, title, year }], the show's), once
    ' they came; the season whose episodes show (0 for none yet) and the one
    ' asked for last; the episode the row's focus goes to first.
    m.seasons = invalid
    m.shownSeason = 0
    m.askedSeason = 0
    m.episodeTarget = 0
    ' The rows under the episodes by key, as they come, and their keys as
    ' shown, in order.
    m.rowNodes = {}
    m.rowKeys = []
    ' Where the focus is: "hero" (the buttons), "tabs", "episodes" or
    ' "rows"; and the episode the info block says, once the focus rested on
    ' one.
    m.area = "hero"
    m.infoShown = invalid
    m.buttons.observeField("buttonSelected", "onButton")
    m.messageButton.observeField("buttonSelected", "onMessageButton")
    m.menu.observeField("itemSelected", "onMenu")
    m.choices.observeField("itemSelected", "onChoice")
    m.rows.observeField("rowItemSelected", "onTile")
    m.episodes.observeField("rowItemSelected", "onEpisode")
    m.episodes.observeField("rowItemFocused", "onEpisodeFocused")
    m.tabs.observeField("focusIndex", "onTabFocused")
    m.tabs.observeField("chosen", "onTabChosen")
    m.tabRest.observeField("fire", "onTabRested")
    m.episodeRest.observeField("fire", "onEpisodeRested")
    m.requestPoll.observeField("fire", "askItemRequest")
    m.top.observeField("active", "onActive")
    m.top.observeField("requestSummary", "onSummary")
    m.top.observeField("requestsChanged", "onRequestsChanged")
end sub

' Loaded each time it shows: back from the player, the place has changed,
' and so have the episodes' marks. Asking for it also has the server read
' the start of the file ahead. The focus comes here at once, where it was:
' the remote works whatever the answer, and a second Back goes back. The
' rows come with the first answer.
sub onActive()
    if not m.top.active
        m.requestPoll.control = "stop"
        return
    end if
    uiChangesSince()
    if m.panel.visible
        closePanel()
    else
        focusArea(m.area)
    end if
    if m.started
        uiGet("/api/items/" + urlValue(m.top.params.itemId), "onDetail")
        if m.shownSeason <> 0 then askEpisodes(m.shownSeason)
    else
        m.started = true
        askDetail()
    end if
end sub

sub askDetail()
    uiRequest({ method: "GET", path: "/api/items/" + urlValue(m.top.params.itemId), build: "detail" }, "onDetail")
end sub

' A title marked here or on another screen: its tiles show it at once,
' then as the server has it.
sub onTitleChanged(change as object)
    uiPatchTiles(m.episodes.content, change)
    if m.detail <> invalid and change.itemId = m.detail.itemId then uiGet("/api/items/" + urlValue(m.detail.itemId), "onDetail")
    if m.shownSeason <> 0 then askEpisodes(m.shownSeason)
end sub

' A reload that fails leaves the details as they were; the first says why,
' with Retry, as on Android TV (Back for a title gone).
sub onDetail(event as object)
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        if m.detail = invalid and not uiFailed(answer) then showFailure(answer)
        return
    end if
    first = m.item = invalid
    m.item = answer.data
    kind = httpString(m.item.kind)
    ' Asked again each time: back from the player, it has moved on. The
    ' buttons wait for it the first time only.
    if kind = "show" or kind = "season"
        if first then m.onDeckKnown = false
        uiGet("/api/items/" + urlValue(m.item.id) + "/on-deck", "onOnDeck")
    end if
    if first
        askRows(event.getRoSGNode(), answer, kind)
        askSeasons(kind)
    end if
    showDetail()
    askItemRequest()
end sub

sub showFailure(answer as object)
    gone = answer.status = 404
    m.messageTitle.text = "Couldn't load this item"
    m.messageText.text = answer.message
    m.messageButton.buttons = ["Retry"]
    if gone
        m.messageTitle.text = "Item not found"
        m.messageText.text = "It may have been removed, or this profile can't see it."
        m.messageButton.buttons = ["Back"]
    end if
    m.messageButton.visible = true
    m.message.visible = true
    if m.top.isInFocusChain() then m.messageButton.setFocus(true)
end sub

' Retry asks again, Loading… meanwhile; Back leaves.
sub onMessageButton()
    if m.messageButton.buttons[0] = "Back"
        uiNavigate({ back: true })
        return
    end if
    m.messageTitle.text = ""
    m.messageText.text = "Loading…"
    m.messageButton.visible = false
    m.top.setFocus(true)
    askDetail()
end sub

' A show's or a season's next episode: its button comes first, and a
' show's page opens on its season.
sub onOnDeck(event as object)
    answer = event.getData()
    m.onDeck = invalid
    if answer.ok and answer.data <> invalid then m.onDeck = httpObject(answer.data.item)
    m.onDeckKnown = true
    showDetail()
    pickSeason()
end sub

sub showDetail()
    if m.item = invalid then return
    years = []
    if m.seasons <> invalid
        for each season in m.seasons
            years.Push(season.year)
        end for
    end if
    beside = { fileId: m.fileId, askMore: m.askMore <> invalid, partOf: m.partOf, years: years }
    m.detail = contentDetail(m.item, m.global.session.server, m.onDeck, beside)
    m.message.visible = false
    m.heading.text = m.detail.heading
    m.context.text = m.detail.context
    m.line.text = m.detail.line
    m.left.text = m.detail.left
    m.genres.text = m.detail.genres
    m.credits.text = m.detail.credits
    fitSummary()
    m.note.text = m.detail.note
    layoutText()
    agent = m.top.getScene().getHttpAgent()
    if m.backdrop.uri <> m.detail.backdrop
        m.backdrop.setHttpAgent(agent)
        m.backdrop.uri = m.detail.backdrop
    end if
    if m.poster.uri <> m.detail.poster
        m.poster.setHttpAgent(agent)
        m.poster.uri = m.detail.poster
    end if
    ' A show's or a season's buttons wait for its next episode, which comes
    ' first.
    hasButtons = m.detail.buttons.Count() > 0 and ((m.detail.kind <> "show" and m.detail.kind <> "season") or m.onDeckKnown)
    if hasButtons
        focused = m.buttons.buttonFocused
        if not m.buttons.visible then focused = m.detail.focus
        if FormatJson(m.buttons.buttons) <> FormatJson(m.detail.buttons) then m.buttons.buttons = m.detail.buttons
        if focused > 0 and focused < m.detail.buttons.Count() then m.buttons.focusButton = focused
    end if
    m.buttons.visible = hasButtons
    m.column.visible = true
    placeLower()
    drawInfo()
    ' A menu of the panel says its entries as they are now.
    if m.panelKind = "more"
        openMore()
    else if m.panelKind = "tracks"
        openTracks()
    end if
    ' Only while this screen has the focus on its buttons: a dialog or the
    ' panel over it keeps it.
    if m.top.isInFocusChain() and m.area = "hero" and not m.panel.visible then focusArea("hero")
end sub

' The column ends above the top of what is under it: past six buttons each
' takes a line from the summary (four at most), and so do the context
' line, the time left and the credits; the tagline gives its room up once
' the summary would keep fewer than two lines. That is for a heading of
' two lines: one of a line gives one back. More's Details has the whole
' summary.
sub fitSummary()
    room = 4
    count = m.detail.buttons.Count()
    if count > 6 then room = room - (count - 6)
    for each part in [m.detail.context, m.detail.left, m.detail.credits]
        if part <> "" then room = room - 1
    end for
    if m.heading.boundingRect().height < 100 then room = room + 1
    tagline = m.detail.tagline
    if tagline <> "" and room < 2
        tagline = ""
        room = room + 1
    end if
    if room < 1 then room = 1
    if room > 4 then room = 4
    m.tagline.text = tagline
    m.summary.maxLines = room
    m.summary.text = m.detail.summary
end sub

' The labels with text, one under the other.
sub layoutText()
    shown = [m.heading]
    spacings = []
    for each part in [{ label: m.context, gap: 8 }, { label: m.line, gap: 16 }, { label: m.left, gap: 8 }, { label: m.genres, gap: 8 }, { label: m.credits, gap: 8 }, { label: m.tagline, gap: 28 }, { label: m.summary, gap: 20 }, { label: m.note, gap: 28 }]
        if part.label.text <> ""
            spacings.Push(part.gap)
            shown.Push(part.label)
        end if
    end for
    m.text.removeChildrenIndex(m.text.getChildCount(), 0)
    m.text.appendChildren(shown)
    m.text.itemSpacings = spacings
end sub

' --- The seasons and episodes ---

' A show's seasons are its tabs, a season's are its show's; an episode
' shows its own season, without tabs, its focus on itself, as on Android
' TV.
sub askSeasons(kind as string)
    showId = 0
    if kind = "show" then showId = httpNumber(m.item.id, 0)
    if kind = "season" then showId = httpNumber(m.item.parent_id, 0)
    if showId > 0
        uiRequest({ method: "GET", path: "/api/items/" + urlValue(showId) + "/children", build: "seasons" }, "onSeasons")
    else if kind = "season"
        m.seasons = []
        pickSeason()
    else if kind = "episode"
        m.seasons = []
        m.episodeTarget = httpNumber(m.item.id, 0)
        seasonId = httpNumber(m.item.parent_id, 0)
        if seasonId > 0 then askEpisodes(seasonId)
    end if
end sub

sub onSeasons(event as object)
    answer = event.getData()
    content = event.getRoSGNode().content
    m.seasons = []
    if answer.ok and content <> invalid
        for each node in content.getChildren(-1, 0)
            m.seasons.Push({ itemId: node.itemId, title: node.title, index: node.index, year: node.year })
        end for
    end if
    chips = []
    for each season in m.seasons
        chips.Push({ label: season.title, width: 0 })
    end for
    m.tabs.chips = chips
    showDetail()
    pickSeason()
end sub

' The season shown first, once the seasons (and a show's next episode)
' came: a season's own, else the next episode's (contentFirstSeason).
sub pickSeason()
    if m.item = invalid or m.seasons = invalid or m.shownSeason <> 0 or m.askedSeason <> 0 then return
    kind = httpString(m.item.kind)
    if kind = "season"
        if not m.onDeckKnown then return
        m.episodeTarget = 0
        if m.onDeck <> invalid and httpNumber(m.onDeck.parent_id, 0) = httpNumber(m.item.id, 0) then m.episodeTarget = httpNumber(m.onDeck.id, 0)
        askEpisodes(httpNumber(m.item.id, 0))
        return
    end if
    ' Waits for the next episode, so the page doesn't jump to its season.
    if kind <> "show" or not m.onDeckKnown then return
    seasonId = contentFirstSeason(m.seasons, m.onDeck)
    m.episodeTarget = 0
    if m.onDeck <> invalid then m.episodeTarget = httpNumber(m.onDeck.id, 0)
    if seasonId > 0 then askEpisodes(seasonId)
end sub

sub askEpisodes(seasonId as dynamic)
    m.askedSeason = seasonId
    ' The season rides along in the spec, which the API task ignores.
    uiRequest({ method: "GET", path: "/api/items/" + urlValue(seasonId) + "/children", build: "episodes", seasonId: seasonId }, "onEpisodes")
end sub

sub onEpisodes(event as object)
    request = event.getRoSGNode()
    seasonId = request.spec.seasonId
    ' An answer for a season left meanwhile.
    if seasonId <> m.askedSeason then return
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) and m.shownSeason = 0
            m.episodesNote.text = answer.message
            m.shownSeason = seasonId
            placeLower()
        end if
        return
    end if
    again = seasonId = m.shownSeason
    keep = 0
    position = m.episodes.rowItemFocused
    if again and position <> invalid and position.Count() = 2 then keep = position[1]
    m.shownSeason = seasonId
    row = CreateObject("roSGNode", "ContentNode")
    row.title = seasonTitleOf(seasonId)
    if request.content <> invalid then row.appendChildren(request.content.getChildren(-1, 0))
    root = CreateObject("roSGNode", "ContentNode")
    root.appendChild(row)
    count = row.getChildCount()
    ' Focus goes to the episode to watch (the page's own episode), else
    ' stays where it was.
    if not again
        keep = 0
        for i = 0 to count - 1
            if row.getChild(i).itemId = m.episodeTarget then keep = i
        end for
    end if
    m.episodes.showRowLabel = [not hasTabs()]
    m.episodes.content = root
    if keep > 0 and keep < count then m.episodes.jumpToRowItem = [0, keep]
    m.episodesNote.text = ""
    if count = 0 then m.episodesNote.text = "No episodes found."
    for i = 0 to m.seasons.Count() - 1
        if m.seasons[i].itemId = seasonId then m.tabs.current = i
    end for
    placeLower()
    if m.area = "episodes" and count = 0 then focusArea("tabs")
    drawInfo()
end sub

' The row's title without tabs: the season's ("Season 1"), as Android TV
' heads it.
function seasonTitleOf(seasonId as dynamic) as string
    if m.seasons <> invalid
        for each season in m.seasons
            if season.itemId = seasonId then return season.title
        end for
    end if
    if m.item <> invalid and httpString(m.item.kind) = "episode"
        title = contentSeasonTitle({ index: m.item.parent_index, title: "" })
        if httpNumber(m.item.parent_index, -1) < 0 and httpSentence(m.item.parent_title) <> "" then title = httpSentence(m.item.parent_title)
        return title
    end if
    if m.item <> invalid and httpString(m.item.kind) = "season" then return contentSeasonTitle(m.item)
    return "Episodes"
end function

' A tab the focus rests on shows its season; OK shows it at once.
sub onTabFocused()
    m.tabRest.control = "stop"
    if m.tabs.isInFocusChain() then m.tabRest.control = "start"
end sub

sub onTabRested()
    if m.tabs.isInFocusChain() then showTab(m.tabs.focusIndex)
end sub

sub onTabChosen()
    showTab(m.tabs.chosen)
end sub

sub showTab(at as integer)
    if m.seasons = invalid or at < 0 or at >= m.seasons.Count() then return
    seasonId = m.seasons[at].itemId
    if seasonId = m.askedSeason then return
    m.episodeTarget = 0
    if m.onDeck <> invalid then m.episodeTarget = httpNumber(m.onDeck.id, 0)
    askEpisodes(seasonId)
end sub

sub onEpisode()
    uiOpenTile(focusedEpisode())
end sub

' The episode in focus in the row, or invalid.
function focusedEpisode() as dynamic
    if m.episodes.content = invalid then return invalid
    row = m.episodes.content.getChild(0)
    position = m.episodes.rowItemFocused
    if row = invalid or position = invalid or position.Count() < 2 then return invalid
    return row.getChild(position[1])
end function

' Tabs for more than one season, as on Android TV.
function hasTabs() as boolean
    return m.seasons <> invalid and m.seasons.Count() > 1
end function

function hasEpisodes() as boolean
    if m.episodes.content = invalid then return false
    row = m.episodes.content.getChild(0)
    return row <> invalid and row.getChildCount() > 0
end function

' --- The rows ---

' The rows that came with the details (extras, cast and crew), and those
' asked for now: More Like This and the collections a movie or show is
' part of.
sub askRows(request as object, answer as object, kind as string)
    built = request.content
    if built <> invalid and answer.rows <> invalid
        children = built.getChildren(-1, 0)
        for i = 0 to answer.rows.Count() - 1
            if i < children.Count() then keepRow(answer.rows[i].key, children[i], answer.rows[i].wide)
        end for
    end if
    path = "/api/items/" + urlValue(m.item.id)
    if kind = "movie" or kind = "show"
        uiRequest({ method: "GET", path: path + "/related", query: { limit: 20 }, build: "tiles", row: "related", title: "More like this" }, "onRow")
        uiRequest({ method: "GET", path: path + "/collections", build: "collections", row: "collections", title: "Collections" }, "onRow")
    end if
    showRows()
end sub

sub onRow(event as object)
    request = event.getRoSGNode()
    answer = event.getData()
    if not answer.ok or request.content = invalid or request.content.getChildCount() = 0 then return
    row = CreateObject("roSGNode", "ContentNode")
    row.title = request.spec.title
    row.appendChildren(request.content.getChildren(-1, 0))
    keepRow(request.spec.row, row, false)
    ' The collections it is part of are on the genres' line too.
    if request.spec.row = "collections"
        m.partOf = []
        for each tile in row.getChildren(-1, 0)
            m.partOf.Push(tile.title)
        end for
        showDetail()
    end if
    showRows()
end sub

sub keepRow(key as string, row as object, wide as boolean)
    m.rowNodes[key] = { node: row, wide: wide }
end sub

' The rows there are, in Android TV's order; the focus stays on its row.
sub showRows()
    keys = []
    for each key in ["extras", "credits", "related", "collections"]
        if m.rowNodes.DoesExist(key) then keys.Push(key)
    end for
    if keys.Count() = 0 or FormatJson(keys) = FormatJson(m.rowKeys) then return
    focusedKey = ""
    position = m.rows.rowItemFocused
    if position <> invalid and position.Count() = 2 and position[0] >= 0 and position[0] < m.rowKeys.Count() then focusedKey = m.rowKeys[position[0]]
    content = CreateObject("roSGNode", "ContentNode")
    sizes = []
    heights = []
    for each key in keys
        row = m.rowNodes[key]
        content.appendChild(row.node)
        if row.wide
            sizes.Push([400, 300])
        else
            sizes.Push([220, 410])
        end if
        heights.Push(sizes.Peek()[1] + 60)
    end for
    m.rowKeys = keys
    m.rows.rowItemSize = sizes
    m.rows.rowHeights = heights
    m.rows.content = content
    for i = 0 to keys.Count() - 1
        if keys[i] = focusedKey then m.rows.jumpToRowItem = [i, position[1]]
    end for
    m.rows.visible = true
    placeLower()
end sub

sub onTile(event as object)
    position = event.getData()
    row = m.rows.content.getChild(position[0])
    if row <> invalid then uiOpenTile(row.getChild(position[1]))
end sub

' --- Where the focus is ---

' The parts under the buttons, one under the other: the tabs (more than
' one season), the season's episodes (or why there are none), the rows.
' With the focus on the buttons their top shows at y = 940; under them,
' the part with the focus comes up under the info block (y = 350), the
' parts above it put away, as on Android TV.
sub placeLower()
    y = 0
    tops = {}
    m.tabs.visible = hasTabs() and (m.area = "hero" or m.area = "tabs" or m.area = "episodes")
    if hasTabs()
        tops.tabs = y
        m.tabs.translation = [96, y]
        y = y + 72
    end if
    episodes = hasEpisodes()
    m.episodes.visible = episodes and m.area <> "rows"
    m.episodesNote.visible = not episodes and m.episodesNote.text <> "" and m.area <> "rows"
    if episodes or m.episodesNote.text <> ""
        tops.episodes = y
        m.episodes.translation = [96, y]
        m.episodesNote.translation = [96, y]
        if episodes
            y = y + 388
        else
            y = y + 72
        end if
    end if
    tops.rows = y
    m.rows.translation = [96, y]
    if m.area = "hero"
        m.lower.translation = [0, 940]
    else if m.area = "rows"
        m.lower.translation = [0, 350 - tops.rows]
    else if tops.DoesExist("tabs")
        m.lower.translation = [0, 350 - tops.tabs]
    else if tops.DoesExist("episodes")
        m.lower.translation = [0, 350 - tops.episodes]
    end if
    m.page.visible = m.area = "hero" and m.detail <> invalid
    m.infoBlock.visible = m.area <> "hero" and m.detail <> invalid
    if m.area = "hero"
        m.shade.width = 1260
    else
        m.shade.width = 1920
    end if
end sub

' The parts the focus can go to, top to bottom.
function areas() as object
    out = ["hero"]
    if hasTabs() then out.Push("tabs")
    if hasEpisodes() then out.Push("episodes")
    if m.rows.visible and m.rows.content <> invalid and m.rows.content.getChildCount() > 0 then out.Push("rows")
    return out
end function

' The part above or below the one with the focus (delta -1 or 1), "" for
' none.
function areaBeside(delta as integer) as string
    list = areas()
    for i = 0 to list.Count() - 1
        if list[i] = m.area
            if i + delta >= 0 and i + delta < list.Count() then return list[i + delta]
            return ""
        end if
    end for
    return ""
end function

sub focusArea(area as string)
    if area = ""
        area = "hero"
    else if area <> "hero"
        ' A part gone meanwhile (a season without episodes): the buttons.
        found = false
        for each other in areas()
            if other = area then found = true
        end for
        if not found then area = "hero"
    end if
    entering = area <> m.area
    m.area = area
    placeLower()
    if area = "tabs"
        ' On the shown season's tab, never the one nearest the focus.
        if entering and m.tabs.current >= 0 then m.tabs.focusIndex = m.tabs.current
        m.tabs.setFocus(true)
    else if area = "episodes"
        m.episodes.setFocus(true)
        if entering then showEpisodeInfo()
    else if area = "rows"
        m.rows.setFocus(true)
    else if m.message.visible and m.messageButton.visible
        m.messageButton.setFocus(true)
    else if m.buttons.visible
        m.buttons.setFocus(true)
    else
        m.top.setFocus(true)
    end if
    drawInfo()
end sub

' --- The info block ---

' Focus moved in the episodes: once it rests, the info block says the
' episode, as on the other TVs.
sub onEpisodeFocused()
    m.episodeRest.control = "stop"
    if m.episodes.isInFocusChain() then m.episodeRest.control = "start"
end sub

sub onEpisodeRested()
    if m.episodes.isInFocusChain() then showEpisodeInfo()
end sub

sub showEpisodeInfo()
    episode = focusedEpisode()
    if episode <> invalid then m.infoShown = { name: episode.name, line: episode.line, summary: episode.summary }
    drawInfo()
end sub

' The title and its line; in the tabs and the episodes, the episode the
' focus rested on, else the title's summary.
sub drawInfo()
    if m.detail = invalid then return
    m.infoTitle.text = m.detail.heading
    m.infoMeta.text = m.detail.line
    episode = invalid
    if m.area = "tabs" or m.area = "episodes" then episode = m.infoShown
    if episode <> invalid
        m.infoEpisode.text = episode.name
        m.infoEpisodeLine.text = episode.line
        m.infoSummary.text = episode.summary
    else
        m.infoEpisode.text = ""
        m.infoEpisodeLine.text = ""
        m.infoSummary.text = m.detail.summary
    end if
    shown = [m.infoTitle]
    spacings = []
    for each part in [{ label: m.infoMeta, gap: 8 }, { label: m.infoEpisode, gap: 14 }, { label: m.infoEpisodeLine, gap: 4 }, { label: m.infoSummary, gap: 8 }]
        if part.label.text <> ""
            spacings.Push(part.gap)
            shown.Push(part.label)
        end if
    end for
    m.infoBlock.removeChildrenIndex(m.infoBlock.getChildCount(), 0)
    m.infoBlock.appendChildren(shown)
    m.infoBlock.itemSpacings = spacings
end sub

' --- Requests ---

' Once the page has drawn, a movie's or show's requests, where the person
' may ask or has asked for something: how its request stands, and whether
' more seasons may be asked for.
sub askItemRequest()
    m.requestPoll.control = "stop"
    if m.item = invalid then return
    kind = httpString(m.item.kind)
    summary = reqSummaryOf(m.top.requestSummary)
    if (kind <> "movie" and kind <> "show") or not (reqMayAsk(summary) or summary.own > 0) then return
    m.requestAsks = m.requestAsks + 1
    uiRequest({ method: "GET", path: "/api/items/" + urlValue(m.item.id) + "/request", ask: m.requestAsks }, "onItemRequest")
end sub

' No match on TMDB (404), or requests off: nothing shows. The server not
' answering leaves it as it was. While the request downloads or goes into
' a library, it is asked again each 10 s.
sub onItemRequest(event as object)
    if event.getRoSGNode().spec.ask <> m.requestAsks then return
    answer = event.getData()
    if answer.ok
        m.itemRequest = httpObject(answer.data)
    else if answer.error <> "network"
        m.itemRequest = invalid
    end if
    showItemRequest()
    request = invalid
    if m.itemRequest <> invalid then request = httpObject(m.itemRequest.request)
    if request <> invalid and reqBusy(httpString(request.stage)) and m.top.active then m.requestPoll.control = "start"
end sub

sub showItemRequest()
    if m.item = invalid then return
    shown = reqItemRequest(m.itemRequest, reqSummaryOf(m.top.requestSummary), CreateObject("roDateTime").AsSeconds(), fmtZoneSeconds())
    stage = shown.stage
    if stage = invalid
        m.stage.visible = false
        m.stageLine.visible = false
    else
        m.stage.color = reqToneColor(stage.tone)
        m.stage.progress = stage.progress
        m.stage.text = stage.text
        m.stageLine.text = stage.line
        m.stage.visible = true
        m.stageLine.visible = stage.line <> ""
    end if
    ' Request more seasons is a button while more may be asked for.
    asked = m.askMore <> invalid
    m.askMore = invalid
    if shown.askMore and shown.kind = "show" and shown.tmdbId > 0 then m.askMore = reqAskMoreTile(shown.kind, shown.tmdbId, httpSentence(m.item.title))
    if asked <> (m.askMore <> invalid) then showDetail()
end sub

' The summary changed: the requests asked for, if they weren't yet;
' else whether more may be asked for.
sub onSummary()
    if m.item = invalid then return
    if m.requestAsks = 0
        askItemRequest()
    else
        showItemRequest()
    end if
end sub

' Requests changed: this title's (or many) are asked for again.
sub onRequestsChanged()
    change = m.top.requestsChanged
    tmdbId = httpNumber(change.tmdbId, 0)
    if tmdbId > 0 and m.itemRequest <> invalid and httpNumber(m.itemRequest.tmdb_id, 0) <> tmdbId then return
    askItemRequest()
end sub

' --- The buttons ---

sub onButton(event as object)
    if m.detail = invalid then return
    action = m.detail.actions[event.getData()]
    if action = "resume" or action = "start"
        startMs = 0
        if action = "resume" then startMs = m.detail.positionMs
        ' The player has the item already: one request less before it plays.
        params = { itemId: m.detail.itemId, startMs: startMs, item: m.item }
        if m.fileId <> invalid then params.fileId = m.fileId
        uiNavigate({ to: "player", params: params })
    else if action = "playNext" or action = "nextFromStart"
        if m.onDeck = invalid
            uiToast("Nothing to play")
            return
        end if
        tile = contentTile(m.onDeck, m.global.session.server, true)
        startMs = 0
        if action = "playNext" then startMs = playerStartOf(tile.positionMs, tile.durationMs)
        uiNavigate({ to: "player", params: { itemId: tile.itemId, startMs: startMs } })
    else if action = "unavailable"
        ' Nothing plays: the other TVs' button can't be pressed.
        return
    else if action = "askMore"
        ' The show's TMDB page, its season picker open.
        if m.askMore <> invalid then uiOpenTile(m.askMore)
    else if action = "trailer"
        ' From its start, as on the other TVs.
        uiNavigate({ to: "player", params: { itemId: m.detail.trailerId, startMs: 0 } })
    else if action = "tracks"
        openTracks()
    else if action = "more"
        openMore()
    else
        titleAction(action)
    end if
end sub

' What the title's tile would do with the Options key: mark it, My List,
' Add to playlist, Go to show or season.
sub titleAction(action as string)
    uiTitleAction(ownTile(), action)
end sub

' The title as its tile would be, for the Options key and titleAction.
function ownTile() as object
    return {
        itemId: m.detail.itemId,
        kind: m.detail.kind,
        title: m.detail.heading,
        watched: m.detail.watched,
        parentId: httpNumber(m.item.parent_id, 0),
        grandparentId: httpNumber(m.item.grandparent_id, 0),
        positionMs: m.detail.positionMs,
        durationMs: m.detail.durationMs
    }
end function

' --- The panel ---

' More: what is used less, in a menu over the right of the page, as on the
' other TVs: the profile's rating, Go to show and Go to season, Media info,
' Add to playlist.
sub openMore()
    labels = []
    for each entry in m.detail.more
        labels.Push(entry.label)
    end for
    showMenu("more", "More", labels)
end sub

' A menu; the entry in focus stays in focus as it shows again.
sub showMenu(kind as string, title as string, labels as object)
    at = 0
    if m.panelKind = kind or m.panelBack = kind then at = m.menu.itemFocused
    items = []
    for each label in labels
        items.Push({ title: label })
    end for
    m.panelKind = kind
    m.panelBack = ""
    m.panelTitle.text = title
    m.menu.content = nodesContent(items)
    if at > 0 and at < items.Count() then m.menu.jumpToItem = at
    m.choices.visible = false
    m.info.visible = false
    m.menu.visible = true
    m.panel.visible = true
    m.menu.setFocus(true)
end sub

' The choices of a menu's entry, the current one checked and focused; Back
' goes to the menu named back ("" closes the panel).
sub showChoices(kind as string, title as string, labels as object, values as object, current as integer, back as string)
    items = []
    for each label in labels
        items.Push({ title: label })
    end for
    m.panelKind = kind
    m.panelBack = back
    m.panelValues = values
    m.panelTitle.text = title
    ' A version's line is long: smaller letters keep it whole.
    size = "Medium"
    if kind = "version" then size = "Small"
    m.choices.font = "font:" + size + "SystemFont"
    m.choices.focusedFont = "font:" + size + "BoldSystemFont"
    m.choices.content = nodesContent(items)
    m.choices.checkedItem = current
    if current > 0 then m.choices.jumpToItem = current
    m.menu.visible = false
    m.info.visible = false
    m.choices.visible = true
    m.panel.visible = true
    m.choices.setFocus(true)
end sub

' Media info: each file and every stream in full (Media.brs), as on the
' other TVs, in text that Up and Down scroll; Back goes back to More.
sub showInfo()
    m.panelKind = "info"
    m.panelBack = "more"
    m.panelTitle.text = "Media info"
    m.info.text = mediaInfoText(m.item)
    m.menu.visible = false
    m.choices.visible = false
    m.info.visible = true
    m.panel.visible = true
    m.info.setFocus(true)
end sub

' Details: the whole summary, then who made the title, its original
' title, when it came out, its studio and how often it was played, as on
' Android TV, in text that Up and Down scroll; Back goes back to More.
sub showAbout()
    m.panelKind = "info"
    m.panelBack = "more"
    m.panelTitle.text = "Details"
    m.info.text = contentAbout(m.item, CreateObject("roDateTime").AsSeconds(), fmtZoneSeconds())
    m.menu.visible = false
    m.choices.visible = false
    m.info.visible = true
    m.panel.visible = true
    m.info.setFocus(true)
end sub

sub closePanel()
    m.panel.visible = false
    m.panelKind = ""
    m.panelBack = ""
    if m.buttons.visible
        m.buttons.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

sub onMenu(event as object)
    at = event.getData()
    if m.panelKind = "tracks"
        if at >= 0 and at < m.trackKinds.Count() then openTrackChoices(m.trackKinds[at], "tracks")
        return
    end if
    if m.panelKind <> "more" or at < 0 or at >= m.detail.more.Count() then return
    action = m.detail.more[at].action
    if action = "rate"
        labels = []
        values = []
        for stars = 0 to 5
            labels.Push(contentStarsLabel(stars))
            values.Push(stars)
        end for
        showChoices("rating", "Your rating", labels, values, m.detail.stars, "more")
        return
    end if
    if action = "info"
        showInfo()
        return
    end if
    if action = "about"
        showAbout()
        return
    end if
    closePanel()
    titleAction(action)
end sub

sub onChoice(event as object)
    at = event.getData()
    if at < 0 or at >= m.panelValues.Count() then return
    kind = m.panelKind
    if kind = "rating"
        rate(m.panelValues[at])
        openMore()
    else if kind = "version" or kind = "audio" or kind = "subtitles"
        if kind = "version"
            chooseVersion(m.panelValues[at])
        else
            pickTrack(kind, m.panelValues[at])
        end if
        if m.panelBack = "tracks"
            openTracks()
        else
            closePanel()
        end if
    end if
end sub

' Back from an entry's choices to its menu, else the panel closes.
sub goBackInPanel()
    if m.panelBack = "more"
        openMore()
    else if m.panelBack = "tracks"
        openTracks()
    else
        closePanel()
    end if
end sub

' --- Audio and subtitles ---

' The file Play plays: the version chosen, else the first.
function playingFile() as dynamic
    return contentFile(m.item, m.fileId)
end function

' The title's versions (several files), and the file's audio (two tracks
' or more) and subtitles (any), as on the other TVs: each an entry saying
' what plays; the tracks chosen are saved at once. With only one of them
' its choices open at once.
sub openTracks()
    versions = mediaVersions(m.item, m.fileId)
    tracks = mediaTracks(playingFile())
    labels = []
    m.trackKinds = []
    if versions.values.Count() > 1
        labels.Push("Version: " + versions.labels[versions.current])
        m.trackKinds.Push("version")
    end if
    if tracks.audio.labels.Count() > 0
        labels.Push("Audio: " + tracks.audio.labels[tracks.audio.current])
        m.trackKinds.Push("audio")
    end if
    if tracks.subtitles.labels.Count() > 0
        labels.Push("Subtitles: " + tracks.subtitles.labels[tracks.subtitles.current])
        m.trackKinds.Push("subtitles")
    end if
    if m.trackKinds.Count() > 1
        showMenu("tracks", "Audio and subtitles", labels)
    else if m.trackKinds.Count() = 1
        openTrackChoices(m.trackKinds[0], "")
    end if
end sub

sub openTrackChoices(kind as string, back as string)
    if kind = "version"
        versions = mediaVersions(m.item, m.fileId)
        showChoices("version", "Version", versions.labels, versions.values, versions.current, back)
        return
    end if
    tracks = mediaTracks(playingFile())
    if kind = "audio"
        showChoices("audio", "Audio", tracks.audio.labels, tracks.audio.values, tracks.audio.current, back)
    else
        showChoices("subtitles", "Subtitles", tracks.subtitles.labels, tracks.subtitles.values, tracks.subtitles.current, back)
    end if
end sub

' PUT /api/items/{id}/streams, as on the other TVs: audio picked here
' (Automatic as none) goes with the subtitles shown; subtitles picked go
' with the audio picked here, else leave the audio as it was picked. The
' page, and Play pressed at once, start with them; the page is asked for
' again for the server's word on them.
sub pickTrack(kind as string, index as integer)
    file = playingFile()
    if file = invalid then return
    subtitle = Int(httpNumber(file.selected_subtitle, -1))
    if kind = "audio"
        m.audioPick = index
    else
        subtitle = index
    end if
    mediaPick(m.item, file.id, m.audioPick, subtitle)
    body = tracksStreamsBody(file.id, m.audioPick, subtitle)
    uiRequest({ method: "PUT", path: "/api/items/" + urlValue(m.item.id) + "/streams", jsonText: FormatJson(body) }, "onTracksSaved")
end sub

' Play plays the version chosen (file_id in POST /api/playback), its
' tracks as the server picks them; it isn't saved, as on the other apps.
sub chooseVersion(fileId as dynamic)
    m.fileId = fileId
    m.audioPick = -2
    ' Its edition and quality on the line.
    showDetail()
end sub

sub onTracksSaved(event as object)
    answer = event.getData()
    if not answer.ok and not uiFailed(answer) then uiToast(answer.message, true)
    if m.item <> invalid then uiGet("/api/items/" + urlValue(m.item.id), "onDetail")
end sub

' --- Your rating ---

' PUT /api/items/{id}/rating: 2 to 10 (half stars), null for none. More
' says it at once, and again as it was if the server refuses.
sub rate(stars as integer)
    if stars < 0 or stars > 5 or m.item = invalid then return
    before = m.detail.stars
    rating = invalid
    if stars > 0 then rating = stars * 2
    showRating(rating)
    uiRequest({ method: "PUT", path: "/api/items/" + urlValue(m.item.id) + "/rating", json: { rating: rating }, before: before }, "onRated")
end sub

sub onRated(event as object)
    answer = event.getData()
    if answer.ok then return
    before = event.getRoSGNode().spec.before
    rating = invalid
    if before > 0 then rating = before * 2
    showRating(rating)
    if not uiFailed(answer) then uiToast(answer.message, true)
end sub

sub showRating(rating as dynamic)
    state = httpObject(m.item.user_state)
    if state = invalid
        state = {}
        m.item.user_state = state
    end if
    state.rating = rating
    showDetail()
end sub

' Down and Up go from the buttons to the tabs, the episodes and the rows,
' and back; Back leaves the page from anywhere on it, as on Android TV. The
' Options key on an episode or a title of the rows offers what can be done
' with it. Back or Left closes the panel (an entry's choices go back to its
' menu), and keys its lists leave go nowhere else.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.panel.visible
        if key = "back" or key = "left" then goBackInPanel()
        return true
    end if
    if m.detail = invalid then return false
    if key = "down" or key = "up"
        delta = 1
        if key = "up" then delta = -1
        other = areaBeside(delta)
        if other = "" then return m.area <> "hero"
        focusArea(other)
        return true
    end if
    if key = "options" and m.area = "episodes"
        uiTileOptions(focusedEpisode(), false)
        return true
    end if
    if key = "options" and m.area = "rows"
        position = m.rows.rowItemFocused
        row = m.rows.content.getChild(position[0])
        if row <> invalid then uiTileOptions(row.getChild(position[1]), false)
        return true
    end if
    ' The title's own, as its tile's.
    if key = "options"
        uiTileOptions(ownTile(), false)
        return true
    end if
    return false
end function
