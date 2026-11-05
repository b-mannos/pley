' A library as the web app shows it: Recommended (GET
' /api/libraries/{id}/hubs, as rows), Library (GET …/items a page at a time,
' sorted and filtered as the chips at the right say, with the A to Z beside
' it when sorted by title) and Collections (GET …/collections). The tab,
' sort and filters it was left on are kept on the Roku for each server,
' profile and library (lib/Library.brs), and it opens on them again.

sub init()
    m.title = m.top.findNode("title")
    m.countLabel = m.top.findNode("count")
    m.tabs = m.top.findNode("tabs")
    m.viewStrip = m.top.findNode("viewStrip")
    m.viewChips = m.top.findNode("view")
    m.grid = m.top.findNode("grid")
    m.letters = m.top.findNode("letters")
    m.jumpTimer = m.top.findNode("jumpTimer")
    m.rows = m.top.findNode("rows")
    m.message = m.top.findNode("message")
    m.panel = m.top.findNode("panel")
    m.panelTitle = m.top.findNode("panelTitle")
    m.choices = m.top.findNode("choices")
    m.menu = m.top.findNode("menu")
    m.panelHint = m.top.findNode("panelHint")
    m.started = false
    m.library = invalid
    m.view = libDefaultView()
    m.viewKey = ""
    ' Whether the view is a row's See all's, which isn't kept.
    m.seeAll = false
    m.tabList = []
    ' The Library tab's titles: their places so far (empty frames where a
    ' page hasn't come), how many in all, the pages asked for ({ "3": true
    ' }), and which listing (sort and filters) the answers must be for.
    m.content = CreateObject("roSGNode", "ContentNode")
    m.total = -1
    m.pages = {}
    m.listing = 0
    ' Its A to Z (lib/Library.brs): invalid until every title's letter
    ' came; the one being made; and the place a jump waits for.
    m.alpha = invalid
    m.alphaMaking = invalid
    m.jumpTo = -1
    ' The other tabs, once loaded (invalid until then), whether the
    ' collections are being asked, and Recommended's rows' keys and sizes,
    ' and whether they end with the collections yet.
    m.collections = invalid
    m.collectionsAsked = false
    m.recommended = invalid
    m.recommendedKeys = []
    m.rowSizes = []
    m.rowHeights = []
    m.collectionsRow = false
    ' What the titles can be filtered by, once the server said
    ' (lib/Library.brs's libFilters); whether it's being asked, and the
    ' filter whose choices wait for it.
    m.filters = invalid
    m.filtersAsked = false
    m.filterWanted = ""
    ' What the panel shows: "sort", "filters", or a filter's choices
    ' ("watched", "genre", "year", "resolution", "dynamic_range"), and the
    ' values of its choices; the Filter panel's rows' actions.
    m.panelKind = ""
    m.values = []
    m.menuActions = []
    ' What each chip is (lib/Library.brs's libChips), how far the chips
    ' slid to keep the focused one in view, and the room they have; whether
    ' the panel showing was opened from a filter's own chip, and goes back
    ' to the chips once its choice is made.
    m.chipKinds = []
    m.chipScroll = 0
    m.chipRoom = 0
    m.chipPanel = false
    ' What the message's button does (showMessage), "" without one.
    m.messageAction = ""
    ' Where the focus goes once the first answer comes: into the titles.
    m.focusContent = true
    m.lastFocus = m.grid
    m.tabs.observeField("chosen", "onTabChosen")
    m.viewChips.observeField("chosen", "onViewChosen")
    m.viewChips.observeField("focusIndex", "scrollChips")
    m.grid.observeField("itemSelected", "onSelected")
    m.grid.observeField("itemFocused", "onFocused")
    m.letters.observeField("chosen", "onLetter")
    m.jumpTimer.observeField("fire", "onJumpDue")
    m.rows.observeField("rowItemSelected", "onRowTile")
    m.choices.observeField("itemSelected", "onChoice")
    m.menu.observeField("itemSelected", "onMenu")
    m.message.observeField("chosen", "onMessageAction")
    m.top.observeField("active", "onActive")
end sub

function pageSize() as integer
    return 70
end function

sub onActive()
    if not m.top.active
        m.lastFocus = m.tabs
        if m.viewChips.isInFocusChain() then m.lastFocus = m.viewChips
        if m.grid.isInFocusChain() then m.lastFocus = m.grid
        if m.letters.isInFocusChain() then m.lastFocus = m.letters
        if m.rows.isInFocusChain() then m.lastFocus = m.rows
        if m.message.isInFocusChain() then m.lastFocus = m.message
        return
    end if
    if not m.started
        m.started = true
        uiChangesSince()
        m.library = m.top.params.library
        typeSet(m.title, m.library.name)
        session = m.global.session
        m.viewKey = libViewKey(session.server, session.profileId, m.library.id)
        m.view = libLoadView(viewSection(), m.viewKey, m.library.kind)
        ' A row's See all (params.sort): the Library tab in the row's order.
        sort = httpString(m.top.params.sort)
        if sort <> ""
            m.view = libSeeAllView(sort, m.library.kind)
            m.seeAll = true
        end if
        m.tabList = libTabs(m.library.kind)
        ' A music library's artists are square.
        if m.library.kind = "music"
            m.grid.itemSize = [220, 300]
            m.grid.numRows = 3
        end if
        chips = []
        for each shownTab in m.tabList
            chips.Push({ label: libTabLabel(shownTab), width: 270 })
        end for
        m.tabs.chips = chips
        m.letters.letters = libLetters()
        m.tabs.setFocus(true)
        showViewChips()
        showTab(m.view.tab)
        ' What the titles can be filtered by, for the Genre and Year chips.
        if m.library.kind <> "music" then askFilters()
        return
    end if
    ' What was played or marked meanwhile.
    for each change in uiChangesSince()
        onTitleChanged(change)
    end for
    if m.panel.visible
        if m.menu.visible
            m.menu.setFocus(true)
        else
            m.choices.setFocus(true)
        end if
    else if m.lastFocus.visible
        m.lastFocus.setFocus(true)
    else
        m.tabs.setFocus(true)
    end if
end sub

' Where each library's tab, sort and filters are kept on this Roku.
function viewSection() as object
    return CreateObject("roRegistrySection", "libraryViews")
end function

' Keeps the view on the Roku. In a See all's order only the tab is kept:
' the sort and filters kept before stay until the person changes them.
sub keepView()
    kept = m.view
    if m.seeAll
        kept = libLoadView(viewSection(), m.viewKey, m.library.kind)
        kept.tab = m.view.tab
    end if
    libSaveView(viewSection(), m.viewKey, kept)
end sub

sub onTitleChanged(change as object)
    uiPatchTiles(m.content, change)
    uiPatchTiles(m.recommended, change)
end sub

' --- Tabs ---

sub onTabChosen(event as object)
    shownTab = m.tabList[event.getData()]
    if shownTab <> m.view.tab then showTab(shownTab)
end sub

' Shows tab, loading it the first time; the Roku keeps it for next time.
sub showTab(wanted as string)
    m.view.tab = wanted
    keepView()
    for i = 0 to m.tabList.Count() - 1
        if m.tabList[i] = wanted
            m.tabs.current = i
            m.tabs.focusIndex = i
        end if
    end for
    m.viewStrip.visible = wanted = "library"
    if m.grid.isInFocusChain() or m.rows.isInFocusChain() or m.letters.isInFocusChain() or m.message.isInFocusChain() then m.tabs.setFocus(true)
    m.grid.visible = false
    m.rows.visible = false
    showLetters()
    if wanted = "library"
        m.grid.content = m.content
        if m.content.getChildCount() > 0 or m.total = 0
            showContent()
        else
            ' Its first page asked for, or on its way: the other tab's
            ' message goes.
            showMessage("Loading…")
            loadPage(0)
        end if
    else if wanted = "collections"
        if m.collections = invalid
            showMessage("Loading…")
            askCollections()
        else
            m.grid.content = m.collections
            showContent()
        end if
    else
        ' Its rows end with the collections (addCollectionsRow).
        askCollections()
        if m.recommended = invalid
            showMessage("Loading…")
            uiRequest({ method: "GET", path: "/api/libraries/" + urlValue(m.library.id) + "/hubs", build: "rows" }, "onRecommended")
        else
            showContent()
        end if
    end if
    showCount()
end sub

' The tab's titles, or what it says when it has none; the focus goes to
' them the first time they come.
sub showContent()
    shownTab = m.view.tab
    empty = ""
    if shownTab = "recommended"
        if m.recommended.getChildCount() = 0 then empty = "Nothing to recommend here yet."
    else if shownTab = "collections"
        if m.collections.getChildCount() = 0 then empty = "This library has no collections."
    else if m.content.getChildCount() = 0
        if libFiltered(m.view)
            ' Nothing matches: Clear filters, as on Android TV.
            showMessage("Nothing in this library matches the current filters.", "No matches", "clear")
        else
            showMessage("This library is empty.")
        end if
        return
    end if
    if empty <> ""
        showMessage(empty)
        return
    end if
    ' The message's button had the focus: the titles take it.
    if m.message.isInFocusChain() then m.focusContent = true
    m.message.visible = false
    m.rows.visible = shownTab = "recommended"
    m.grid.visible = shownTab <> "recommended"
    showLetters()
    if m.focusContent and m.top.isInFocusChain() and not m.panel.visible
        m.focusContent = false
        focusContent()
    end if
end sub

' What the tab says in place of its titles: text, under title, with the
' button of action ("clear": Clear filters, "retry": Retry) where it has
' one. That button takes the focus that was in the titles, or that waited
' for them to come; else the focus in the titles goes to the tabs. On the
' chips or the tabs, it stays.
sub showMessage(text as string, title = "" as string, action = "" as string)
    inContent = m.top.hasFocus() or m.grid.isInFocusChain() or m.rows.isInFocusChain() or m.letters.isInFocusChain() or m.message.isInFocusChain()
    m.grid.visible = false
    m.rows.visible = false
    showLetters()
    m.messageAction = action
    label = ""
    if action = "clear" then label = "Clear filters"
    if action = "retry" then label = "Retry"
    m.message.title = title
    typeSet(m.message, text)
    m.message.action = label
    m.message.visible = true
    if m.panel.visible or not m.top.isInFocusChain() then return
    if label <> "" and (inContent or m.focusContent)
        m.focusContent = false
        m.message.setFocus(true)
    else if inContent
        m.tabs.setFocus(true)
    end if
end sub

' The tab couldn't load, as on Android TV: what didn't, the server's
' sentence, and Retry.
sub showFailure(answer as object)
    titles = { library: "Couldn't load this library", recommended: "Couldn't load recommendations", collections: "Couldn't load collections" }
    showMessage(answer.message, titles[m.view.tab], "retry")
end sub

' The message's button: Clear filters lists every title again, Retry asks
' for the tab again, the focus going to the titles as they come.
sub onMessageAction()
    action = m.messageAction
    m.focusContent = true
    if action = "clear"
        libClearFilters(m.view)
        applyView()
    else if action = "retry"
        showTab(m.view.tab)
    end if
end sub

sub focusContent()
    if m.rows.visible
        m.rows.setFocus(true)
    else if m.grid.visible
        m.grid.setFocus(true)
    else if m.message.visible and m.message.action <> ""
        m.message.setFocus(true)
    end if
end sub

sub showCount()
    if m.view.tab = "library" and m.total >= 0
        typeSet(m.countLabel, libCountLine(m.view, m.library.kind, m.total, m.library.itemCount))
    else if m.view.tab = "collections" and m.collections <> invalid
        n = m.collections.getChildCount()
        typeSet(m.countLabel, urlValue(n) + " collections")
        if n = 1 then m.countLabel.text = "1 collection"
    else
        typeSet(m.countLabel, fmtCount(m.library.itemCount, m.library.kind))
    end if
end sub

' The library's collections, for the Collections tab and the end of
' Recommended, asked once at a time; a music library has none.
sub askCollections()
    if m.collections <> invalid or m.collectionsAsked or m.library.kind = "music" then return
    m.collectionsAsked = true
    uiRequest({ method: "GET", path: "/api/libraries/" + urlValue(m.library.id) + "/collections", build: "collections" }, "onCollections")
end sub

sub onCollections(event as object)
    m.collectionsAsked = false
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) and m.view.tab = "collections" then showFailure(answer)
        return
    end if
    m.collections = event.getRoSGNode().content
    if m.collections = invalid then m.collections = CreateObject("roSGNode", "ContentNode")
    addCollectionsRow()
    if m.view.tab = "recommended" and m.recommended <> invalid then showContent()
    if m.view.tab <> "collections" then return
    m.grid.content = m.collections
    showCount()
    showContent()
end sub

sub onRecommended(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) and m.view.tab = "recommended" then showFailure(answer)
        return
    end if
    content = event.getRoSGNode().content
    if content = invalid then content = CreateObject("roSGNode", "ContentNode")
    m.rowSizes = []
    m.rowHeights = []
    m.recommendedKeys = []
    for each row in answer.data.rows
        m.recommendedKeys.Push(row.key)
        shape = contentRowSize(row.wide, row.square)
        m.rowSizes.Push(shape.size)
        m.rowHeights.Push(shape.height)
    end for
    m.recommended = content
    m.collectionsRow = false
    if m.rowSizes.Count() > 0
        m.rows.rowItemSize = m.rowSizes
        m.rows.rowHeights = m.rowHeights
        m.rows.content = content
        typeRowLabels(m.rows)
    end if
    addCollectionsRow()
    if m.view.tab = "recommended" then showContent()
end sub

' The collections end the Recommended tab once both came, as on Android
' TV (libCollectionsRow): copies of the Collections tab's tiles, whose
' nodes are that grid's. With no other rows, they are its only one.
sub addCollectionsRow()
    if m.recommended = invalid or m.collections = invalid or m.collectionsRow then return
    tiles = []
    for each node in m.collections.getChildren(20, 0)
        tiles.Push({ title: node.title, subtitle: node.subtitle, image: node.image, wide: false, progress: 0.0, watched: false, itemId: node.itemId, kind: node.kind })
    end for
    row = libCollectionsRow(tiles)
    if row = invalid then return
    m.collectionsRow = true
    shape = contentRowSize(false, false)
    m.rowSizes.Push(shape.size)
    m.rowHeights.Push(shape.height)
    m.recommendedKeys.Push(row.key)
    m.rows.rowItemSize = m.rowSizes
    m.rows.rowHeights = m.rowHeights
    rowNode = m.recommended.createChild("ContentNode")
    rowNode.title = row.title
    nodesAppend(rowNode, row.tiles)
    if m.rows.content = invalid or not m.rows.content.isSameNode(m.recommended) then m.rows.content = m.recommended
end sub

' --- The Library tab ---

' A page of posters, by its number; the task makes their ContentNodes.
sub loadPage(page as integer)
    at = page * pageSize()
    if page < 0 or m.pages.DoesExist(page.ToStr()) or (m.total >= 0 and at >= m.total) then return
    m.pages[page.ToStr()] = true
    if m.content.getChildCount() = 0 then showMessage("Loading…")
    query = libQuery(m.view, at, pageSize())
    path = urlWithQuery("/api/libraries/" + urlValue(m.library.id) + "/items", query)
    uiRequest({ method: "GET", path: path, build: "tiles", badges: libShowsBadges(m.view), bitrate: m.view.sort = "bitrate", listing: m.listing, page: page }, "onPage")
end sub

sub onPage(event as object)
    request = event.getRoSGNode()
    spec = request.spec
    if spec.listing <> m.listing then return
    answer = event.getData()
    if not answer.ok
        ' Asked again when the focus comes near it.
        m.pages.Delete(spec.page.ToStr())
        if m.content.getChildCount() = 0 and not uiFailed(answer) and m.view.tab = "library" then showFailure(answer)
        return
    end if
    page = answer.data
    at = spec.page * pageSize()
    m.total = page.total
    nodes = []
    if request.content <> invalid then nodes = request.content.getChildren(-1, 0)
    place(at, nodes)
    ' An empty page ends the listing, whatever the total said.
    if page.listed = 0 then m.total = at
    count = m.content.getChildCount()
    if m.total >= 0 and count > m.total then m.content.removeChildrenIndex(count - m.total, m.total)
    askLetters()
    ' A jump waited for this page.
    if m.jumpTo >= 0 and m.jumpTo < m.content.getChildCount() then m.jumpTimer.control = "start"
    if m.view.tab <> "library" then return
    showCount()
    showContent()
end sub

' A page's tiles at at. The places between the last title here and the
' page (a jump far down) are empty frames until their titles come: plain
' ContentNodes, which Tile draws as frames, made in one native call however
' many (a library of thousands). The tiles take the frames' places, or
' come after the rest.
sub place(at as integer, nodes as object)
    count = m.content.getChildCount()
    if at > count
        m.content.createChildren(at - count, "ContentNode")
        count = at
    end if
    over = []
    after = []
    for i = 0 to nodes.Count() - 1
        if at + i < count
            over.Push(nodes[i])
        else
            after.Push(nodes[i])
        end if
    end for
    if over.Count() > 0 then m.content.replaceChildren(over, at)
    if after.Count() > 0 then m.content.appendChildren(after)
end sub

' The pages near the focus come, and the A to Z follows it.
sub onFocused(event as object)
    if m.view.tab <> "library" then return
    near(event.getData())
    if m.alpha <> invalid then m.letters.current = libAlphaLetterAt(m.alpha, event.getData())
end sub

' Two rows before a page's end, the next page comes; two rows into a page
' (after a jump), the one before.
sub near(index as integer)
    page = Int(index / pageSize())
    loadPage(page)
    if index - page * pageSize() < 14 then loadPage(page - 1)
    if (page + 1) * pageSize() - index <= 14 then loadPage(page + 1)
end sub

sub onSelected(event as object)
    uiOpenTile(m.grid.content.getChild(event.getData()))
end sub

' A title opens; See all shows the Library tab in its row's order, which
' isn't kept, the focus in the titles once they come; the collections'
' See all, the Collections tab.
sub onRowTile(event as object)
    position = event.getData()
    row = m.rows.content.getChild(position[0])
    if row = invalid then return
    tile = row.getChild(position[1])
    if tile <> invalid and tile.kind = "more" and tile.hasField("opens")
        m.focusContent = true
        showTab("collections")
        return
    end if
    if tile <> invalid and tile.kind = "more" and tile.hasField("sort")
        m.view = libSeeAllView(tile.sort, m.library.kind)
        m.seeAll = true
        m.focusContent = true
        relist()
        showTab("library")
        return
    end if
    uiOpenTile(tile)
end sub

' The sort and the filters as chips (libChips), each as wide as its
' words, at the right of the tabs; the focus stays on the chip of the same
' kind, as a filter chosen changes its words or a chip comes or goes.
sub showViewChips()
    focusedKind = ""
    at = m.viewChips.focusIndex
    if at >= 0 and at < m.chipKinds.Count() then focusedKind = m.chipKinds[at]
    m.chipKinds = []
    chips = []
    for each chip in libChips(m.view, m.library.kind, m.filters)
        m.chipKinds.Push(chip.kind)
        chips.Push({ label: chip.label, width: 0, selected: chip.selected })
    end for
    m.viewChips.chips = chips
    m.viewChips.current = -1
    for i = 0 to m.chipKinds.Count() - 1
        if m.chipKinds[i] = focusedKind then at = i
    end for
    if at >= m.chipKinds.Count() then at = m.chipKinds.Count() - 1
    if at < 0 then at = 0
    m.viewChips.focusIndex = at
    ' From the tabs' end to the right edge; past it, they are cut off at
    ' the tabs and slide (scrollChips).
    m.chipRoom = 1824 - (m.tabs.translation[0] + m.tabs.rowWidth + 32)
    width = m.viewChips.rowWidth
    if width <= m.chipRoom
        m.viewStrip.translation = [1824 - width, 132]
        m.viewStrip.clippingRect = [0, 0, 0, 0]
    else
        m.viewStrip.translation = [1824 - m.chipRoom, 132]
        m.viewStrip.clippingRect = [0, -8, m.chipRoom, 72]
    end if
    scrollChips()
end sub

' The chip in focus in view: the row slides no further than it needs, and
' not at all when it fits.
sub scrollChips()
    places = m.viewChips.places
    width = m.viewChips.rowWidth
    scroll = m.chipScroll
    at = m.viewChips.focusIndex
    if width <= m.chipRoom or places = invalid
        scroll = 0
    else if at >= 0 and at < places.Count()
        chip = places[at]
        if chip[0] + chip[1] - scroll > m.chipRoom then scroll = chip[0] + chip[1] - m.chipRoom
        if chip[0] < scroll then scroll = chip[0]
    end if
    if scroll > width - m.chipRoom then scroll = width - m.chipRoom
    if scroll < 0 then scroll = 0
    m.chipScroll = scroll
    m.viewChips.translation = [-scroll, 0]
end sub

' A new sort or filters: the Roku keeps them, and the titles come again.
sub applyView()
    m.seeAll = false
    keepView()
    relist()
end sub

' The Library tab's titles of the view, from the top, with their A to Z.
sub relist()
    showViewChips()
    m.listing = m.listing + 1
    m.content = CreateObject("roSGNode", "ContentNode")
    m.grid.content = m.content
    m.total = -1
    m.pages = {}
    m.alpha = invalid
    m.alphaMaking = invalid
    m.jumpTo = -1
    showLetters()
    loadPage(0)
end sub

' --- The A to Z ---

' Sorted by title with more than 30 titles, the listing's letters come in
' pages of a thousand, once the first page said how many there are.
sub askLetters()
    if m.alpha <> invalid or m.alphaMaking <> invalid or not libShowsJump(m.view, m.total) then return
    m.alphaMaking = libAlphaNew()
    loadLetters()
end sub

sub loadLetters()
    query = libQuery(m.view, m.alphaMaking.count, 1000)
    uiRequest({ method: "GET", path: urlWithQuery("/api/libraries/" + urlValue(m.library.id) + "/items", query), build: "letters", listing: m.listing }, "onLetters")
end sub

' Without an answer, this listing has no A to Z.
sub onLetters(event as object)
    if event.getRoSGNode().spec.listing <> m.listing or m.alphaMaking = invalid then return
    answer = event.getData()
    if not answer.ok then return
    page = answer.data
    libAlphaAdd(m.alphaMaking, page.runs, page.listed)
    if page.listed > 0 and m.alphaMaking.count < page.total
        loadLetters()
        return
    end if
    m.alpha = m.alphaMaking
    has = []
    for i = 0 to 26
        has.Push(libAlphaHas(m.alpha, i))
    end for
    m.letters.has = has
    m.letters.current = libAlphaLetterAt(m.alpha, m.grid.itemFocused)
    showLetters()
end sub

' Beside the Library tab's grid while it shows, once its letters came.
sub showLetters()
    shown = m.alpha <> invalid and m.view.tab = "library" and m.grid.visible
    if not shown and m.letters.isInFocusChain()
        if m.grid.visible
            m.grid.setFocus(true)
        else
            m.tabs.setFocus(true)
        end if
    end if
    m.letters.visible = shown
end sub

' OK on a letter: the grid at its first title (lib/Library.brs's
' libAlphaJump), once that title's page is here.
sub onLetter(event as object)
    if m.alpha = invalid then return
    target = libAlphaJump(m.alpha, event.getData())
    if target < m.content.getChildCount()
        jumpTo(target)
    else
        m.jumpTo = target
        loadPage(Int(target / pageSize()))
    end if
end sub

' The page a jump waited for came: the grid goes there, unless the focus
' left the letters meanwhile.
sub onJumpDue()
    if m.jumpTo >= 0 and m.letters.isInFocusChain() then jumpTo(m.jumpTo)
    m.jumpTo = -1
end sub

sub jumpTo(position as integer)
    m.grid.jumpToItem = position
    m.grid.setFocus(true)
    near(position)
end sub

' --- The sort's and the filters' panel ---

' Sort and Filter open their panels; Unwatched turns on or off at once; a
' filter's own chip (Genre, Year, a resolution, a dynamic range) opens its
' choices, and the pick goes back to the chips, as on Android TV.
sub onViewChosen(event as object)
    at = event.getData()
    if at < 0 or at >= m.chipKinds.Count() then return
    kind = m.chipKinds[at]
    m.chipPanel = false
    if kind = "sort"
        openSort()
    else if kind = "filter"
        openFilters()
    else if kind = "unwatched"
        m.view.watched = libToggleUnwatched(m.view.watched)
        applyView()
    else
        m.chipPanel = true
        if m.filters = invalid
            ' Its choices once the server says what they are.
            openFilters()
            m.filterWanted = kind
            m.panelHint.text = "Loading…"
        else
            openFilter(kind)
        end if
    end if
end sub

sub openSort()
    labels = []
    values = []
    current = 0
    for each sort in libSorts(m.library.kind)
        if sort.id = m.view.sort then current = values.Count()
        labels.Push(sort.label)
        values.Push(sort.id)
    end for
    showChoices("sort", "Sort by", labels, values, current, "OK on the one checked turns the order around.")
end sub

' The filters, each with what it keeps; OK opens its choices. Once some
' are set, Clear filters and Save as…. What the titles can be filtered by
' is asked the first time: Resolution and Dynamic range join the rows when
' it comes, the focus staying on its row.
sub openFilters()
    m.panelKind = "filters"
    m.panelTitle.text = "Filter"
    rows = libFilterRows(m.view, m.library.kind, m.filters)
    at = m.menu.itemFocused
    focused = ""
    if at >= 0 and at < m.menuActions.Count() then focused = m.menuActions[at]
    m.menuActions = []
    for i = 0 to rows.Count() - 1
        m.menuActions.Push(rows[i].action)
        if rows[i].action = focused then at = i
    end for
    m.menu.content = nodesContent(rows)
    if at > 0 and at < rows.Count() then m.menu.jumpToItem = at
    m.panelHint.text = ""
    m.choices.visible = false
    m.menu.visible = true
    m.panel.visible = true
    m.menu.setFocus(true)
    if m.filters = invalid then askFilters()
end sub

sub onMenu(event as object)
    at = event.getData()
    if at < 0 or at >= m.menuActions.Count() then return
    action = m.menuActions[at]
    if action = "watched"
        current = 0
        if m.view.watched = "false" then current = 1
        if m.view.watched = "true" then current = 2
        showChoices("watched", "Show", ["All titles", "Unwatched", "Watched"], ["", "false", "true"], current, "")
    else if action = "clear"
        libClearFilters(m.view)
        applyView()
        openFilters()
    else if action = "save"
        saveAs()
    else if m.filters = invalid
        ' Its choices once the server says what they are.
        m.filterWanted = action
        m.panelHint.text = "Loading…"
        askFilters()
    else
        openFilter(action)
    end if
end sub

sub askFilters()
    if m.filtersAsked then return
    m.filtersAsked = true
    uiRequest({ method: "GET", path: "/api/libraries/" + urlValue(m.library.id) + "/filters" }, "onFilters")
end sub

' --- Save as… ---

' These filters, and the order, as a smart playlist (POST /api/playlists
' with library_id and smart_filter): its name typed on the keyboard, what
' it finds said above. The panel closes, back to the Filter chip.
sub saveAs()
    closePanel()
    m.smartFilter = libSmartFilter(m.view)
    dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
    dialog.title = "Save these filters"
    dialog.message = [libSmartFilterWords(m.view).Join(" · ") + ". It keeps up with the library by itself."]
    dialog.buttons = ["Save", "Cancel"]
    dialog.observeField("buttonSelected", "onSaveNamed")
    m.top.getScene().dialog = dialog
end sub

sub onSaveNamed(event as object)
    dialog = m.top.getScene().dialog
    name = ""
    if dialog <> invalid then name = dialog.text.Trim()
    uiCloseDialog()
    if event.getData() <> 0 or name = "" then return
    uiRequest({ method: "POST", path: "/api/playlists", json: { title: name, library_id: m.library.id, smart_filter: m.smartFilter } }, "onSaved")
end sub

' Saved: the new playlist's page.
sub onSaved(event as object)
    answer = event.getData()
    if answer.ok and httpObject(answer.data) <> invalid
        uiNavigate({ to: "playlist", params: { id: answer.data.id, title: httpSentence(answer.data.title) } })
    else if not uiFailed(answer)
        uiMessage("That didn't work", [pagePlaylistRefusal(answer)])
    end if
end sub

' Asked again the next time the panel opens when it didn't come. Once it
' does, the filter chosen meanwhile opens; else the Filter panel, showing,
' takes the rows it adds.
sub onFilters(event as object)
    m.filtersAsked = false
    wanted = m.filterWanted
    m.filterWanted = ""
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        ' Only the Filter panel waits for it: another panel's hint stays its own.
        if not uiFailed(answer) and m.panel.visible and m.panelKind = "filters" then typeSet(m.panelHint, answer.message)
        return
    end if
    m.filters = libFilters(answer.data)
    ' Genre and Year join the chips.
    showViewChips()
    if not m.panel.visible or m.panelKind <> "filters" then return
    if wanted <> ""
        openFilter(wanted)
    else if libFilterRows(m.view, m.library.kind, m.filters).Count() <> m.menuActions.Count()
        openFilters()
    end if
end sub

' A genre, a year, a resolution or a dynamic range to keep, or Any.
sub openFilter(kind as string)
    labels = ["Any"]
    values = [""]
    current = 0
    if kind = "genre"
        for each genre in m.filters.genres
            if genre = m.view.genre then current = values.Count()
            labels.Push(genre)
            values.Push(genre)
        end for
        showChoices("genre", "Genre", labels, values, current, "")
    else if kind = "resolution"
        choices = libQualityChoices(fmtResolutions(), m.filters.resolutions, m.view.resolution, "All resolutions")
        showChoices("resolution", "Resolution", choices.labels, choices.values, choices.current, "")
    else if kind = "dynamic_range"
        choices = libQualityChoices(fmtDynamicRanges(), m.filters.dynamicRanges, m.view.dynamic_range, "All dynamic ranges")
        showChoices("dynamic_range", "Dynamic range", choices.labels, choices.values, choices.current, "")
    else
        values = [0]
        for each year in m.filters.years
            if year = m.view.year then current = values.Count()
            labels.Push(urlValue(year))
            values.Push(year)
        end for
        showChoices("year", "Year", labels, values, current, "")
    end if
end sub

sub showChoices(kind as string, title as string, labels as object, values as object, current as integer, hint as string)
    m.panelKind = kind
    m.values = values
    typeSet(m.panelTitle, title)
    items = []
    for each label in labels
        items.Push({ title: label })
    end for
    m.choices.content = nodesContent(items)
    m.choices.checkedItem = current
    m.choices.jumpToItem = current
    typeSet(m.panelHint, hint)
    m.menu.visible = false
    m.choices.visible = true
    m.panel.visible = true
    m.choices.setFocus(true)
end sub

sub onChoice(event as object)
    at = event.getData()
    if at < 0 or at >= m.values.Count() then return
    value = m.values[at]
    kind = m.panelKind
    if kind = "sort"
        if value = m.view.sort
            if m.view.order = "asc"
                m.view.order = "desc"
            else
                m.view.order = "asc"
            end if
        else
            m.view.sort = value
            m.view.order = libDefaultOrder(value)
        end if
        closePanel()
        applyView()
        return
    end if
    if kind = "watched" then m.view.watched = value
    if kind = "genre" then m.view.genre = value
    if kind = "year" then m.view.year = value
    if kind = "resolution" then m.view.resolution = value
    if kind = "dynamic_range" then m.view.dynamic_range = value
    if m.chipPanel
        closePanel()
        applyView()
        return
    end if
    applyView()
    openFilters()
end sub

sub closePanel()
    m.panel.visible = false
    m.panelKind = ""
    m.chipPanel = false
    ' A filter picked while its choices were loading is forgotten with the panel.
    m.filterWanted = ""
    m.viewChips.setFocus(true)
end sub

' Down from the chips goes to the titles, up from their top row (or the
' A to Z's first letter) back; right from the last tab reaches the sort
' and filters, and from the grid's last column the A to Z, at the letter
' of the title it leaves, which Left goes back to. Back closes the
' panel (a filter's choices go back to the filters, unless its own chip
' opened them). The Options key on a title offers what can be done with
' it.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.panel.visible
        if key = "back" or key = "left"
            if m.panelKind <> "sort" and m.panelKind <> "filters" and not m.chipPanel
                openFilters()
            else
                closePanel()
            end if
            return true
        end if
        return false
    end if
    chips = m.tabs.isInFocusChain() or m.viewChips.isInFocusChain()
    if key = "down" and chips and (m.grid.visible or m.rows.visible or (m.message.visible and m.message.action <> ""))
        focusContent()
        return true
    end if
    if key = "up" and (m.grid.isInFocusChain() or m.rows.isInFocusChain() or m.letters.isInFocusChain() or m.message.isInFocusChain())
        m.tabs.setFocus(true)
        return true
    end if
    if key = "right" and m.grid.isInFocusChain() and m.letters.visible
        m.letters.focusIndex = m.letters.current
        m.letters.setFocus(true)
        return true
    end if
    if key = "left" and m.letters.isInFocusChain()
        m.grid.setFocus(true)
        return true
    end if
    if key = "right" and m.tabs.isInFocusChain() and m.viewStrip.visible
        m.viewChips.focusIndex = 0
        m.viewChips.setFocus(true)
        return true
    end if
    if key = "left" and m.viewChips.isInFocusChain()
        m.tabs.setFocus(true)
        return true
    end if
    if key = "options" and m.grid.isInFocusChain() and m.grid.content <> invalid
        uiTileOptions(m.grid.content.getChild(m.grid.itemFocused), false)
        return true
    end if
    if key = "options" and m.rows.isInFocusChain()
        position = m.rows.rowItemFocused
        row = m.rows.content.getChild(position[0])
        if row <> invalid then uiTileOptions(row.getChild(position[1]), contentContinueWatching(m.recommendedKeys[position[0]]))
        return true
    end if
    return false
end function
