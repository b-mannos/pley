' Discover (GET /api/discover/{list}): a hero, TMDB's lists as rows of
' posters loaded as they come into view, and Your requests (GET
' /api/requests?mine=1). The hero's title is read whole (GET
' /api/discover/{kind}/{tmdbId}) for its main action, as its page has it;
' a movie's Request and Tell me too ask at once, a toast saying what came
' of it. In the rows, an info block and the backdrop follow the tile focus
' rests on. A request changed elsewhere changes the badge of its title
' where it shows (the scene's requestNote), never the lists.

sub init()
    ' Whether Your requests showed: a poll that fails then leaves them be.
    m.requestsShown = false
    m.backdrop = m.top.findNode("backdrop")
    m.shade = m.top.findNode("shade")
    m.page = m.top.findNode("page")
    m.chips = m.top.findNode("chips")
    m.credit = m.top.findNode("credit")
    m.hero = m.top.findNode("hero")
    m.heroName = m.top.findNode("heroName")
    m.heroLine = m.top.findNode("heroLine")
    m.heroOverview = m.top.findNode("heroOverview")
    m.heroStatus = m.top.findNode("heroStatus")
    m.heroStatusText = m.top.findNode("heroStatusText")
    m.heroStage = m.top.findNode("heroStage")
    m.heroButtons = m.top.findNode("heroButtons")
    m.heroNote = m.top.findNode("heroNote")
    m.rows = m.top.findNode("rows")
    m.info = m.top.findNode("info")
    m.infoTitle = m.top.findNode("infoTitle")
    m.infoMeta = m.top.findNode("infoMeta")
    m.infoStatus = m.top.findNode("infoStatus")
    m.infoStage = m.top.findNode("infoStage")
    m.infoSeasons = m.top.findNode("infoSeasons")
    m.infoSummary = m.top.findNode("infoSummary")
    m.infoWho = m.top.findNode("infoWho")
    m.infoLine = m.top.findNode("infoLine")
    m.problem = m.top.findNode("problem")
    m.problemTitle = m.top.findNode("problemTitle")
    m.problemText = m.top.findNode("problemText")
    m.problemButtons = m.top.findNode("problemButtons")
    m.busy = m.top.findNode("busy")
    m.rest = m.top.findNode("rest")
    m.top.findNode("heading").text = t("requests.names.discover")
    m.credit.text = t("requests.credit")
    m.started = false
    ' The rows in order: { key (a list's, or "requests"), title, state
    ' ("waiting", "asked", "shown"), page (the last one asked for), seen
    ' (the titles shown: { "movie:603": true }), first (its first title,
    ' a DiscoverTitle, or invalid), failed (why it failed, "" for not) }.
    ' A list with nothing for the profile leaves it.
    m.lists = []
    m.generation = 0
    ' The hero's title (DiscoverTitle), its page once read (DiscoverDetail,
    ' asked again with each load), how it shows (reqHero's), and its
    ' status and library title as last known (a change gives its tiles the
    ' new badge); whether it was found, or none can be.
    m.heroTitle = invalid
    m.heroDetail = invalid
    m.heroLoads = 0
    m.heroShown = invalid
    m.heroBadge = ""
    m.heroFound = false
    ' What the hero asks for while it's on its way: a second press waits.
    m.asked = invalid
    ' A row answered: a problem after that is only that row's.
    m.answered = false
    ' The problem shown in place of the page (reqProblem's), or invalid;
    ' whether the page shows only Sign in.
    m.problemShown = invalid
    m.signInOnly = false
    ' Whether the focus was put in the rows, the tile it rests on (its
    ' key, "" for none), and where it was when covered.
    m.inRows = false
    m.restingOn = ""
    m.lastFocus = ""
    m.heroButtons.observeField("buttonSelected", "onHeroButton")
    m.problemButtons.observeField("buttonSelected", "onProblemButton")
    m.chips.observeField("chosen", "onChip")
    m.rows.observeField("rowItemSelected", "onTile")
    m.rows.observeField("rowItemFocused", "onRowFocused")
    m.busy.observeField("fire", "reloadRequests")
    m.rest.observeField("fire", "onRested")
    m.top.observeField("active", "onActive")
    m.top.observeField("requestSummary", "onSummary")
    m.top.observeField("requestsChanged", "onRequestsChanged")
    m.top.observeField("requestNote", "onRequestNote")
end sub

sub onActive()
    if not m.top.active
        m.busy.control = "stop"
        m.rest.control = "stop"
        m.lastFocus = "hero"
        if m.rows.isInFocusChain() then m.lastFocus = "rows"
        if m.chips.isInFocusChain() then m.lastFocus = "chips"
        return
    end if
    if not m.started
        m.started = true
        showPage()
        return
    end if
    restoreFocus()
    ' Back from a title's page: what was asked for there shows in Your
    ' requests and in the hero.
    onRequestsChanged()
end sub

' The page for the summary: only Sign in for a sign-in from before
' apps could ask; else the hero and the rows, each row's tiles empty
' frames until it loads.
sub showPage()
    summary = reqSummaryOf(m.top.requestSummary)
    m.busy.control = "stop"
    m.rest.control = "stop"
    m.answered = false
    m.heroTitle = invalid
    m.heroDetail = invalid
    m.heroShown = invalid
    m.heroFound = false
    m.inRows = false
    m.hero.visible = false
    m.info.visible = false
    m.backdrop.uri = ""
    m.problem.visible = false
    m.problemShown = invalid
    m.page.translation = [0, 0]
    m.shade.translation = [0, 440]
    m.signInOnly = summary.signInAgain = "requests"
    if m.signInOnly
        showProblem(reqSignInAgain())
        return
    end if
    showChips(summary)
    m.credit.visible = true
    ' Answers to the page as it was before are left aside.
    m.generation = m.generation + 1
    m.lists = []
    for each list in reqLists()
        m.lists.Push(newList(list.key, list.title))
    end for
    if reqShowsRequests(summary) and summary.own > 0 then m.lists.Push(newList("requests", t("requests.names.yours")))
    content = CreateObject("roSGNode", "ContentNode")
    for each list in m.lists
        row = content.createChild("ContentNode")
        row.title = list.title
        nodesAppend(row, reqSkeletonTiles())
    end for
    m.rows.content = content
    m.rows.visible = true
    m.top.setFocus(true)
    ensureRows(1)
end sub

function newList(key as string, title as string) as object
    return { key: key, title: title, state: "waiting", page: 0, seen: {}, first: invalid, failed: "" }
end function

' The requests, where the summary has them, with the menu's number of
' what waits there. Without them the focus leaves the empty row.
sub showChips(summary as object)
    chips = []
    if reqShowsRequests(summary) then chips.Push(reqRequestsChip(summary))
    if FormatJson(m.chips.chips) <> FormatJson(chips) then m.chips.chips = chips
    m.chips.visible = chips.Count() > 0
    if not m.chips.visible and m.chips.isInFocusChain() then focusRows(false)
end sub

' --- The rows ---

' The rows up to upto load, if they haven't.
sub ensureRows(upto as integer)
    for i = 0 to upto
        if i < m.lists.Count() and m.lists[i].state = "waiting" then askRow(i)
    end for
end sub

' The row after the one focused comes into view: it loads; and the info
' block follows the focus once it rests.
sub onRowFocused()
    position = m.rows.rowItemFocused
    if position = invalid or position.Count() <> 2 then return
    ensureRows(position[0] + 1)
    onTileFocused()
end sub

sub askRow(index as integer)
    list = m.lists[index]
    list.state = "asked"
    if list.key = "requests"
        summary = reqSummaryOf(m.top.requestSummary)
        uiRequest({ method: "GET", path: "/api/requests", query: { mine: 1, limit: 20 }, build: "requestTiles", manage: summary.manage, list: list.key, generation: m.generation }, "onRequestsRow")
        return
    end if
    uiRequest({ method: "GET", path: "/api/discover/" + list.key, query: { page: list.page + 1 }, build: "discoverTiles", known: list.seen, list: list.key, generation: m.generation }, "onListRow")
end sub

' The row an answer is for, on the page now: its place, or -1 (an earlier
' page's answer, or a row that went).
function rowOf(spec as object) as integer
    if spec.generation <> m.generation then return -1
    for i = 0 to m.lists.Count() - 1
        if m.lists[i].key = spec.list then return i
    end for
    return -1
end function

' A page of a list: its titles after those shown, then See all. A page
' short of a screenful isn't the end: the next one comes, up to five,
' then See all goes on from there. A list with nothing for the profile
' goes.
sub onListRow(event as object)
    request = event.getRoSGNode()
    spec = request.spec
    answer = event.getData()
    index = rowOf(spec)
    if index < 0 then return
    list = m.lists[index]
    shown = []
    if list.page > 0
        for each node in m.rows.content.getChild(index).getChildren(-1, 0)
            if node.kind = "discover" then shown.Push(node)
        end for
    end if
    if not answer.ok
        list.state = "shown"
        ' The server away: the page in view gives way to the screen that
        ' says so, a covered one's row offers Try again.
        if uiFailed(answer) and (m.top.active or answer.error <> "network") then return
        problem = reqProblem(answer, reqSummaryOf(m.top.requestSummary).manage)
        if problem <> invalid and not m.answered
            showProblem(problem)
            return
        end if
        list.failed = answer.message
        ' What came before stays, See all going on from it.
        if shown.Count() > 0
            setRow(index, shown, reqMoreTile(t("common.actions.seeAll"), list.key, ""))
        else
            setRow(index, [], reqNoticeTile(t("requests.tiles.theseFailed"), "retry"))
        end if
        ' Without trending, the hero comes from the other lists: every one
        ' at once, as when trending has nothing to ask for.
        if list.key = "trending" and list.first = invalid then ensureRows(m.lists.Count() - 1)
        findHero()
        return
    end if
    m.answered = true
    list.failed = ""
    page = answer.data
    if list.first = invalid and httpNumber(page.first.tmdb_id, 0) > 0 then list.first = page.first
    ' A server that answers the same page again has no more.
    more = page.page < page.totalPages and page.page > list.page
    list.page = page.page
    if request.content <> invalid
        for each node in request.content.getChildren(-1, 0)
            if not list.seen.DoesExist(node.key)
                list.seen[node.key] = true
                shown.Push(node)
            end if
        end for
    end if
    if shown.Count() < 7 and more and page.page < 5
        if shown.Count() > 0 then setRow(index, shown, invalid)
        askRow(index)
        findHero()
        return
    end if
    list.state = "shown"
    if shown.Count() = 0 and not more
        ' Nothing here the profile may ask for: the row goes, as on the
        ' other TVs.
        dropRow(index)
    else
        extra = invalid
        if more then extra = reqMoreTile(t("common.actions.seeAll"), list.key, "")
        setRow(index, shown, extra)
    end if
    ' Trending has nothing the profile may ask for: every list comes at
    ' once, for a hero.
    if list.key = "trending" and list.first = invalid then ensureRows(m.lists.Count() - 1)
    findHero()
end sub

' Your requests: their posters and stages, then See all. None left (the
' last one withdrawn): the row goes.
sub onRequestsRow(event as object)
    request = event.getRoSGNode()
    spec = request.spec
    answer = event.getData()
    index = rowOf(spec)
    if index < 0 then return
    m.lists[index].state = "shown"
    if not answer.ok
        if uiFailed(answer) then return
        ' A poll that fails leaves the requests shown, and asks again in
        ' its time; with none shown yet, the row says so.
        if m.requestsShown
            restartPoll(false)
        else
            setRow(index, [], reqNoticeTile(t("requests.tiles.theseFailed"), "retry"))
        end if
        return
    end if
    tiles = []
    if request.content <> invalid then tiles = request.content.getChildren(-1, 0)
    if tiles.Count() = 0
        dropRow(index)
        return
    end if
    m.requestsShown = true
    setRow(index, tiles, reqMoreTile(t("common.actions.seeAll"), "requests", ""))
    restartPoll(answer.data.busy)
end sub

' Your requests asked again in 10 s while one of them downloads, in 30 s
' while the event stream is down (reqListPoll).
sub restartPoll(busy as boolean)
    m.busy.control = "stop"
    seconds = reqListPoll(busy, m.top.getScene().eventsLive)
    if seconds > 0 and m.top.active
        m.busy.duration = seconds
        m.busy.control = "start"
    end if
end sub

' Your requests asks again: one of them downloads, the event stream is
' down, or requests changed.
sub reloadRequests()
    for i = 0 to m.lists.Count() - 1
        if m.lists[i].key = "requests" and m.lists[i].state = "shown" then askRow(i)
    end for
end sub

' Requests changed, or the page shows again: Your requests and the hero's
' title as they are now.
sub onRequestsChanged()
    reloadRequests()
    askHeroDetail()
end sub

' Puts tiles (ContentNodes) in row index, then extra (a tile's fields, or
' invalid), in place of what it showed; the focus stays where it was.
sub setRow(index as integer, tiles as object, extra as dynamic)
    content = m.rows.content
    if content = invalid or index >= content.getChildCount() then return
    row = CreateObject("roSGNode", "ContentNode")
    row.title = m.lists[index].title
    row.appendChildren(tiles)
    if extra <> invalid then nodesAppend(row, [extra])
    focus = m.rows.rowItemFocused
    content.replaceChild(row, index)
    if focus <> invalid and focus.Count() = 2 and focus[0] = index and m.rows.isInFocusChain()
        column = focus[1]
        if column >= row.getChildCount() then column = row.getChildCount() - 1
        if column < 0 then column = 0
        m.rows.jumpToRowItem = [index, column]
    end if
end sub

' Takes row index away; the focus stays on the row it was on, else goes
' to the one in its place. The rows coming into view load.
sub dropRow(index as integer)
    content = m.rows.content
    if content = invalid or index >= content.getChildCount() then return
    focus = m.rows.rowItemFocused
    m.lists.Delete(index)
    content.removeChildIndex(index)
    count = content.getChildCount()
    at = 0
    if focus <> invalid and focus.Count() = 2 and count > 0
        at = focus[0]
        column = focus[1]
        if at > index then at = at - 1
        if at = index then column = 0
        if at >= count then at = count - 1
        if m.rows.isInFocusChain()
            last = content.getChild(at).getChildCount() - 1
            if column > last then column = last
            if column < 0 then column = 0
            m.rows.jumpToRowItem = [at, column]
        end if
    end if
    ensureRows(at + 1)
end sub

' A tile opens its title or its list; a row's Try again loads it again.
sub onTile(event as object)
    position = event.getData()
    row = m.rows.content.getChild(position[0])
    if row = invalid then return
    tile = row.getChild(position[1])
    if tile = invalid then return
    if tile.kind = "retry" and position[0] < m.lists.Count()
        list = m.lists[position[0]]
        list.page = 0
        list.seen = {}
        list.failed = ""
        setRow(position[0], nodesContent(reqSkeletonTiles()).getChildren(-1, 0), invalid)
        askRow(position[0])
        return
    end if
    uiOpenTile(tile)
end sub

' A title's request changed: its badge, in the rows and the hero. Told by
' another screen, the hero's page is read again for its main action.
sub onRequestNote()
    note = m.top.requestNote
    uiPatchRequestTiles(m.rows.content, note)
    if m.heroTitle <> invalid and reqKey(httpString(m.heroTitle.kind), m.heroTitle.tmdb_id) = note.key
        m.heroTitle.status = note.status
        m.heroTitle.item_id = note.libraryId
        badge = httpString(note.status) + ":" + urlValue(httpNumber(note.libraryId, 0))
        if badge <> m.heroBadge
            m.heroBadge = badge
            askHeroDetail()
        end if
        drawHero()
    end if
end sub

' --- The info block ---

' Focus moved in the rows: once it rests 0.4 s on a title or a request,
' the info block and the backdrop follow it, as on Home.
sub onTileFocused()
    tile = focusedTile()
    m.restingOn = ""
    if tile <> invalid then m.restingOn = tile.key
    m.rest.control = "stop"
    if m.restingOn <> "" and m.rows.isInFocusChain() then m.rest.control = "start"
end sub

sub onRested()
    if m.restingOn = "" or not m.rows.isInFocusChain() then return
    tile = focusedTile()
    if tile = invalid or tile.key <> m.restingOn then return
    drawInfo(tile)
end sub

' A title's name, facts and two lines of overview; a request's name, its
' stage with the seasons asked for, who asked and when, and its stage's
' line.
sub drawInfo(tile as object)
    request = tile.hasField("who")
    typeSet(m.infoTitle, tile.title)
    if request
        m.infoMeta.text = ""
        m.infoSummary.text = ""
        m.infoStage.color = tile.subtitleColor
        typeSet(m.infoStage, tile.subtitle)
        typeSet(m.infoSeasons, tile.seasonsText)
        typeSet(m.infoWho, tile.who)
        typeSet(m.infoLine, tile.stageLine)
    else
        typeSet(m.infoMeta, tile.facts)
        typeSet(m.infoSummary, tile.overview)
        m.infoStage.text = ""
        m.infoSeasons.text = ""
        m.infoWho.text = ""
        m.infoLine.text = ""
    end if
    layOut(m.info, [
        { node: m.infoTitle, gap: 0, filled: true },
        { node: m.infoMeta, gap: 8, filled: m.infoMeta.text <> "" },
        { node: m.infoStatus, gap: 10, filled: request and m.infoStage.text <> "" },
        { node: m.infoSummary, gap: 12, filled: m.infoSummary.text <> "" },
        { node: m.infoWho, gap: 10, filled: m.infoWho.text <> "" },
        { node: m.infoLine, gap: 4, filled: m.infoLine.text <> "" }
    ])
    m.info.visible = true
    if tile.backdrop <> "" then setBackdrop(tile.backdrop)
end sub

' The tile the rows' focus is on, a title or a request, or invalid (See
' all, a row's frames or its note).
function focusedTile() as dynamic
    if m.rows.content = invalid then return invalid
    position = m.rows.rowItemFocused
    if position = invalid or position.Count() < 2 then return invalid
    row = m.rows.content.getChild(position[0])
    if row = invalid then return invalid
    tile = row.getChild(position[1])
    if tile = invalid or not tile.hasField("backdrop") then return invalid
    return tile
end function

' Only the parts filled stay in a column, each after its gap; one that had
' the focus keeps it (taken out and put back, it would lose it).
sub layOut(column as object, parts as object)
    shown = []
    spacings = []
    focused = invalid
    for each part in parts
        if part.filled
            if shown.Count() > 0 then spacings.Push(part.gap)
            shown.Push(part.node)
            if part.node.isInFocusChain() then focused = part.node
        end if
    end for
    column.removeChildrenIndex(column.getChildCount(), 0)
    column.appendChildren(shown)
    column.itemSpacings = spacings
    if focused <> invalid then focused.setFocus(true)
end sub

' The scene's agent as each backdrop is asked for, as the other screens
' do: a new token makes the scene a new one.
sub setBackdrop(uri as string)
    if m.backdrop.uri = uri then return
    m.backdrop.setHttpAgent(m.top.getScene().getHttpAgent())
    m.backdrop.uri = uri
end sub

' --- The hero ---

' The hero, once the lists say which it is (reqHeroOf).
sub findHero()
    if m.heroFound then return
    title = reqHeroOf(m.lists)
    if title = invalid then return
    m.heroFound = true
    showHero(title)
end sub

' The hero's title, its page read for its main action; or none, and the
' page says why (reqNoHero).
sub showHero(title as object)
    if httpNumber(title.tmdb_id, 0) = 0
        showProblem(reqNoHero(m.lists))
        return
    end if
    m.heroTitle = title
    m.heroBadge = httpString(title.status) + ":" + urlValue(httpNumber(title.item_id, 0))
    drawHero()
    askHeroDetail()
end sub

' The hero's title on TMDB, whole: its main action as its page has it.
sub askHeroDetail()
    title = m.heroTitle
    if title = invalid or m.problemShown <> invalid then return
    m.heroLoads = m.heroLoads + 1
    kind = httpString(title.kind)
    uiRequest({ method: "GET", path: "/api/discover/" + kind + "/" + urlValue(httpNumber(title.tmdb_id, 0)), hero: reqKey(kind, title.tmdb_id), load: m.heroLoads, generation: m.generation }, "onHeroDetail")
end sub

' A load that fails leaves the hero as it was. A new status gives the
' title's tiles, here and elsewhere, their new badge.
sub onHeroDetail(event as object)
    spec = event.getRoSGNode().spec
    if spec.generation <> m.generation or spec.load <> m.heroLoads or m.heroTitle = invalid then return
    if spec.hero <> reqKey(httpString(m.heroTitle.kind), m.heroTitle.tmdb_id) then return
    answer = event.getData()
    detail = invalid
    if answer.ok then detail = httpObject(answer.data)
    if detail = invalid then return
    m.heroDetail = detail
    status = httpString(detail.status)
    libraryId = httpNumber(detail.item_id, 0)
    badge = status + ":" + urlValue(libraryId)
    if badge <> m.heroBadge
        m.heroBadge = badge
        uiNoteRequest({ key: spec.hero, status: status, libraryId: libraryId })
    end if
    drawHero()
end sub

sub drawHero()
    if m.heroTitle = invalid then return
    hero = reqHero(m.heroTitle, m.heroDetail, reqSummaryOf(m.top.requestSummary), m.global.session.server, CreateObject("roDateTime").AsSeconds(), fmtZoneSeconds())
    m.heroShown = hero
    typeSet(m.heroName, hero.title)
    typeSet(m.heroLine, hero.line)
    typeSet(m.heroOverview, hero.overview)
    stage = hero.stage
    if stage <> invalid
        ' Requested or Following, beside the request's stage; under them
        ' the stage's own line.
        typeSet(m.heroStatusText, hero.note)
        m.heroStage.color = reqToneColor(stage.tone)
        m.heroStage.progress = stage.progress
        typeSet(m.heroStage, stage.text)
        typeSet(m.heroNote, stage.line)
    else
        m.heroStatusText.text = ""
        m.heroStage.text = ""
        typeSet(m.heroNote, hero.note)
    end if
    focused = m.heroButtons.buttonFocused
    if FormatJson(m.heroButtons.buttons) <> FormatJson(hero.buttons)
        m.heroButtons.buttons = hero.buttons
        if focused > 0 and focused < hero.buttons.Count() then m.heroButtons.focusButton = focused
    end if
    noteGap = 28
    if stage <> invalid then noteGap = 12
    layOut(m.hero, [
        { node: m.heroName, gap: 0, filled: true },
        { node: m.heroLine, gap: 16, filled: m.heroLine.text <> "" },
        { node: m.heroOverview, gap: 20, filled: m.heroOverview.text <> "" },
        { node: m.heroStatus, gap: 28, filled: stage <> invalid },
        { node: m.heroNote, gap: noteGap, filled: m.heroNote.text <> "" },
        { node: m.heroButtons, gap: 28, filled: true }
    ])
    m.hero.visible = not m.inRows
    if not m.inRows then setBackdrop(hero.backdrop)
    if m.top.hasFocus() then m.heroButtons.setFocus(true)
end sub

' Open: the library's title; Request (a movie) and Tell me too ask at
' once; a show's Request opens its page with Which seasons?; Stop
' following, at once; Details: the title's page.
sub onHeroButton(event as object)
    hero = m.heroShown
    if hero = invalid then return
    at = event.getData()
    if at < 0 or at >= hero.actions.Count() then return
    action = hero.actions[at]
    if action = "open"
        uiNavigate({ to: "details", params: { itemId: hero.libraryId } })
    else if action = "request"
        heroAsk(invalid)
    else if action = "follow" and m.heroDetail <> invalid
        heroAsk(reqFollowSeasons(m.heroDetail.seasons))
    else if action = "pick"
        uiNavigate({ to: "requestTitle", params: { kind: hero.tmdbKind, tmdbId: hero.tmdbId, title: hero.title, pick: true } })
    else if action = "unwatch"
        uiRequestAct({ requestId: hero.requestId, title: hero.title, status: "", stage: "", kind: hero.tmdbKind, tmdbId: hero.tmdbId, seasons: [], libraryId: hero.libraryId, actions: ["unwatch"], mine: false }, "unwatch")
    else if action = "signIn"
        uiSignInAgain("requests")
    else
        uiNavigate({ to: "requestTitle", params: { kind: hero.tmdbKind, tmdbId: hero.tmdbId, title: hero.title } })
    end if
end sub

' Asks for the hero's title (POST /api/requests): a movie, or Tell me too
' (a show's seasons someone asked for already), which costs nothing.
sub heroAsk(seasons as dynamic)
    hero = m.heroShown
    if m.asked <> invalid or hero = invalid then return
    m.asked = { title: hero.title, kind: hero.tmdbKind, tmdbId: hero.tmdbId, allowance: hero.allowance }
    uiRequest({ method: "POST", path: "/api/requests", jsonText: reqAskBody(hero.tmdbKind, hero.tmdbId, seasons) }, "onHeroAsked")
end sub

' What came of asking, in a toast; the hero says how the title stands
' now, and what may still be asked for is asked again.
sub onHeroAsked(event as object)
    asked = m.asked
    m.asked = invalid
    answer = event.getData()
    if not answer.ok
        if uiFailed(answer) then return
        uiToast(reqAskProblem(answer, asked.allowance, asked.kind, CreateObject("roDateTime").AsSeconds()), true)
    else
        uiToast(reqAsked(answer.status, answer.data, asked.title))
    end if
    m.top.getScene().requestTouched = { requestId: 0, kind: asked.kind, tmdbId: asked.tmdbId }
    askHeroDetail()
end sub

' Stop following was done, or refused as the request had changed: the
' hero as it is now.
sub onRequestActed()
    hero = m.heroShown
    if hero <> invalid then m.top.getScene().requestTouched = { requestId: 0, kind: hero.tmdbKind, tmdbId: hero.tmdbId }
    askHeroDetail()
end sub

sub onChip()
    uiNavigate({ to: "requests" })
end sub

' --- What stops Discover ---

' In place of the page, why it can't be used: with Sign in, Try
' again, or nothing to do here.
sub showProblem(problem as object)
    m.problemShown = problem
    m.busy.control = "stop"
    m.rest.control = "stop"
    m.inRows = false
    m.hero.visible = false
    m.info.visible = false
    m.rows.visible = false
    m.chips.visible = false
    m.credit.visible = false
    m.backdrop.uri = ""
    m.page.translation = [0, 0]
    m.shade.translation = [0, 440]
    typeSet(m.problemTitle, problem.title)
    typeSet(m.problemText, problem.text)
    buttons = []
    if problem.signIn then buttons.Push(t("requests.buttons.signIn"))
    if problem.retry then buttons.Push(t("common.actions.retry"))
    m.problemButtons.buttons = buttons
    m.problemButtons.visible = buttons.Count() > 0
    m.problem.visible = true
    if m.top.isInFocusChain()
        if buttons.Count() > 0
            m.problemButtons.setFocus(true)
        else
            m.top.setFocus(true)
        end if
    end if
end sub

sub onProblemButton()
    if m.problemShown = invalid then return
    if m.problemShown.signIn
        uiSignInAgain("requests")
    else
        showPage()
    end if
end sub

' The summary changed: signing in again (or a new sign-in) changes the
' whole page; otherwise the chips and the hero's main action follow.
sub onSummary()
    if not m.started then return
    summary = reqSummaryOf(m.top.requestSummary)
    if (summary.signInAgain = "requests") <> m.signInOnly
        showPage()
        return
    end if
    if m.problemShown <> invalid then return
    showChips(summary)
    drawHero()
    addRequestsRow(summary)
end sub

' A first request, asked for since the page showed: Your requests comes
' after TMDB's lists, loaded as it comes into view like them.
sub addRequestsRow(summary as object)
    if not reqShowsRequests(summary) or summary.own <= 0 or m.rows.content = invalid then return
    for each list in m.lists
        if list.key = "requests" then return
    end for
    title = t("requests.names.yours")
    m.lists.Push(newList("requests", title))
    row = m.rows.content.createChild("ContentNode")
    row.title = title
    nodesAppend(row, reqSkeletonTiles())
    position = m.rows.rowItemFocused
    if m.rows.isInFocusChain() and position <> invalid and position.Count() = 2 then ensureRows(position[0] + 1)
end sub

' --- The remote ---

' The page slides up to the rows as they take the focus, the backdrop
' shaded under them all and the info block in the hero's place; and back
' down to the hero.
sub focusRows(onRows as boolean)
    m.inRows = onRows
    m.rest.control = "stop"
    if onRows
        m.page.translation = [0, -370]
        m.shade.translation = [0, -200]
        m.hero.visible = false
        m.rows.setFocus(true)
        onTileFocused()
        return
    end if
    m.page.translation = [0, 0]
    m.shade.translation = [0, 440]
    m.info.visible = false
    m.restingOn = ""
    m.hero.visible = m.heroShown <> invalid
    if m.hero.visible
        setBackdrop(m.heroShown.backdrop)
        m.heroButtons.setFocus(true)
    else if m.chips.visible
        m.chips.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

sub restoreFocus()
    if m.problem.visible
        if m.problemButtons.visible
            m.problemButtons.setFocus(true)
        else
            m.top.setFocus(true)
        end if
    else if m.lastFocus = "rows" and m.rows.visible
        m.rows.setFocus(true)
    else if m.lastFocus = "chips" and m.chips.visible
        m.chips.setFocus(true)
    else if m.hero.visible
        m.heroButtons.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

' Up and Down between the chips, the hero and the rows; Back from the
' rows goes to the hero first.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press or m.problem.visible then return false
    if key = "down"
        if m.chips.isInFocusChain()
            if m.hero.visible
                m.heroButtons.setFocus(true)
            else if m.rows.visible
                focusRows(true)
            end if
            return true
        end if
        if (m.heroButtons.isInFocusChain() or m.top.hasFocus()) and m.rows.visible
            focusRows(true)
            return true
        end if
    else if key = "up"
        if m.rows.isInFocusChain()
            focusRows(false)
            return true
        end if
        if (m.heroButtons.isInFocusChain() or m.top.hasFocus()) and m.chips.visible
            m.chips.setFocus(true)
            return true
        end if
    else if key = "back" and m.rows.isInFocusChain() and m.heroShown <> invalid
        focusRows(false)
        return true
    end if
    return false
end function
