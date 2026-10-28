' A library as the web app shows it: Recommended (GET
' /api/libraries/{id}/hubs, as rows), Library (GET …/items a page at a time,
' sorted and filtered as the chips at the right say) and Collections (GET
' …/collections). The tab, sort and filters it was left on are kept on the
' Roku for each server, profile and library (lib/Library.brs), and it
' opens on them again.

sub init()
    m.title = m.top.findNode("title")
    m.countLabel = m.top.findNode("count")
    m.tabs = m.top.findNode("tabs")
    m.viewChips = m.top.findNode("view")
    m.grid = m.top.findNode("grid")
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
    m.tabList = []
    ' The Library tab's titles: listed so far (the next page's offset), in
    ' all, and which listing (sort and filters) the answers must be for.
    m.content = CreateObject("roSGNode", "ContentNode")
    m.offset = 0
    m.total = -1
    m.loading = false
    m.listing = 0
    ' The other tabs, once loaded (invalid until then).
    m.collections = invalid
    m.recommended = invalid
    m.recommendedKeys = []
    ' Genres and years to filter by, once the server said.
    m.filters = invalid
    ' What the panel shows: "sort", "filters", or a filter's choices
    ' ("watched", "genre", "year"), and the values of its choices.
    m.panelKind = ""
    m.values = []
    ' Where the focus goes once the first answer comes: into the titles.
    m.focusContent = true
    m.lastFocus = m.grid
    m.tabs.observeField("chosen", "onTabChosen")
    m.viewChips.observeField("chosen", "onViewChosen")
    m.grid.observeField("itemSelected", "onSelected")
    m.grid.observeField("itemFocused", "onFocused")
    m.rows.observeField("rowItemSelected", "onRowTile")
    m.choices.observeField("itemSelected", "onChoice")
    m.menu.observeField("itemSelected", "onMenu")
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
        if m.rows.isInFocusChain() then m.lastFocus = m.rows
        return
    end if
    if not m.started
        m.started = true
        uiChangesSince()
        m.library = m.top.params.library
        m.title.text = m.library.name
        session = m.global.session
        m.viewKey = libViewKey(session.server, session.profileId, m.library.id)
        m.view = libLoadView(viewSection(), m.viewKey, m.library.kind)
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
        m.tabs.setFocus(true)
        showViewChips()
        showTab(m.view.tab)
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
    libSaveView(viewSection(), m.viewKey, m.view)
    for i = 0 to m.tabList.Count() - 1
        if m.tabList[i] = wanted
            m.tabs.current = i
            m.tabs.focusIndex = i
        end if
    end for
    m.viewChips.visible = wanted = "library"
    if m.grid.isInFocusChain() or m.rows.isInFocusChain() then m.tabs.setFocus(true)
    m.grid.visible = false
    m.rows.visible = false
    if wanted = "library"
        m.grid.content = m.content
        if m.content.getChildCount() > 0
            showContent()
        else
            loadMore()
        end if
    else if wanted = "collections"
        if m.collections = invalid
            showMessage("Loading…")
            uiRequest({ method: "GET", path: "/api/libraries/" + urlValue(m.library.id) + "/collections", build: "collections" }, "onCollections")
        else
            m.grid.content = m.collections
            showContent()
        end if
    else
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
        empty = "This library is empty."
        if libFiltered(m.view) then empty = "No titles match these filters."
    end if
    if empty <> ""
        showMessage(empty)
        return
    end if
    m.message.visible = false
    m.rows.visible = shownTab = "recommended"
    m.grid.visible = shownTab <> "recommended"
    if m.focusContent and m.top.isInFocusChain() and not m.panel.visible
        m.focusContent = false
        focusContent()
    end if
end sub

sub showMessage(text as string)
    m.grid.visible = false
    m.rows.visible = false
    m.message.text = text
    m.message.visible = true
    if not m.panel.visible and m.top.isInFocusChain() and not m.viewChips.isInFocusChain() then m.tabs.setFocus(true)
end sub

sub focusContent()
    if m.rows.visible
        m.rows.setFocus(true)
    else if m.grid.visible
        m.grid.setFocus(true)
    end if
end sub

sub showCount()
    if m.view.tab = "library" and m.total >= 0
        m.countLabel.text = fmtCount(m.total, m.library.kind)
    else if m.view.tab = "collections" and m.collections <> invalid
        n = m.collections.getChildCount()
        m.countLabel.text = urlValue(n) + " collections"
        if n = 1 then m.countLabel.text = "1 collection"
    else
        m.countLabel.text = fmtCount(m.library.itemCount, m.library.kind)
    end if
end sub

sub onCollections(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) and m.view.tab = "collections" then showMessage(answer.message)
        return
    end if
    m.collections = event.getRoSGNode().content
    if m.collections = invalid then m.collections = CreateObject("roSGNode", "ContentNode")
    if m.view.tab <> "collections" then return
    m.grid.content = m.collections
    showCount()
    showContent()
end sub

sub onRecommended(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) and m.view.tab = "recommended" then showMessage(answer.message)
        return
    end if
    content = event.getRoSGNode().content
    if content = invalid then content = CreateObject("roSGNode", "ContentNode")
    sizes = []
    heights = []
    m.recommendedKeys = []
    for each row in answer.data.rows
        m.recommendedKeys.Push(row.key)
        shape = contentRowSize(row.wide, row.square)
        sizes.Push(shape.size)
        heights.Push(shape.height)
    end for
    m.recommended = content
    if sizes.Count() > 0
        m.rows.rowItemSize = sizes
        m.rows.rowHeights = heights
        m.rows.content = content
    end if
    if m.view.tab = "recommended" then showContent()
end sub

' --- The Library tab ---

' A page of posters; the task makes their ContentNodes.
sub loadMore()
    if m.loading or (m.total >= 0 and m.offset >= m.total) then return
    m.loading = true
    if m.content.getChildCount() = 0 then showMessage("Loading…")
    query = libQuery(m.view, m.offset, pageSize())
    uiRequest({ method: "GET", path: urlWithQuery("/api/libraries/" + urlValue(m.library.id) + "/items", query), build: "tiles", listing: m.listing }, "onPage")
end sub

sub onPage(event as object)
    if event.getRoSGNode().spec.listing <> m.listing then return
    m.loading = false
    answer = event.getData()
    if not answer.ok
        if m.content.getChildCount() = 0 and not uiFailed(answer) and m.view.tab = "library" then showMessage(answer.message)
        return
    end if
    page = answer.data
    m.total = page.total
    m.offset = m.offset + page.listed
    ' An empty page ends the listing, whatever the total said.
    if page.listed = 0 then m.total = m.offset
    tiles = event.getRoSGNode().content
    if tiles <> invalid and tiles.getChildCount() > 0 then m.content.appendChildren(tiles.getChildren(-1, 0))
    if m.view.tab <> "library" then return
    showCount()
    showContent()
end sub

' Two rows before the end, the next page comes.
sub onFocused(event as object)
    if m.view.tab = "library" and event.getData() >= m.content.getChildCount() - 14 then loadMore()
end sub

sub onSelected(event as object)
    uiOpenTile(m.grid.content.getChild(event.getData()))
end sub

sub onRowTile(event as object)
    position = event.getData()
    row = m.rows.content.getChild(position[0])
    if row <> invalid then uiOpenTile(row.getChild(position[1]))
end sub

' The sort and the filters as chips, at the right of the tabs.
sub showViewChips()
    sortLabel = libSortLabel(m.view.sort, m.view.order)
    filterLabel = libFilterLabel(m.view)
    sortWidth = 120 + Len(sortLabel) * 15
    filterWidth = 120 + Len(filterLabel) * 15
    if filterWidth > 420 then filterWidth = 420
    m.viewChips.chips = [{ label: sortLabel, width: sortWidth }, { label: filterLabel, width: filterWidth }]
    m.viewChips.current = -1
    if libFiltered(m.view) then m.viewChips.current = 1
    m.viewChips.translation = [1824 - sortWidth - 16 - filterWidth, 132]
end sub

' A new sort or filters: the Roku keeps them, and the titles come again.
sub applyView()
    libSaveView(viewSection(), m.viewKey, m.view)
    showViewChips()
    m.listing = m.listing + 1
    m.content = CreateObject("roSGNode", "ContentNode")
    m.grid.content = m.content
    m.offset = 0
    m.total = -1
    m.loading = false
    loadMore()
end sub

' --- The sort's and the filters' panel ---

sub onViewChosen(event as object)
    if event.getData() = 0
        openSort()
    else
        openFilters()
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

' The filters, each with what it keeps; OK opens its choices.
sub openFilters()
    m.panelKind = "filters"
    m.panelTitle.text = "Filter"
    items = [{ title: "Show: " + watchedLabel(m.view.watched) }]
    genre = "Any"
    if m.view.genre <> "" then genre = m.view.genre
    items.Push({ title: "Genre: " + genre })
    year = "Any"
    if m.view.year > 0 then year = urlValue(m.view.year)
    items.Push({ title: "Year: " + year })
    if libFiltered(m.view) then items.Push({ title: "Clear filters" })
    at = m.menu.itemFocused
    m.menu.content = nodesContent(items)
    if at > 0 and at < items.Count() then m.menu.jumpToItem = at
    m.panelHint.text = ""
    m.choices.visible = false
    m.menu.visible = true
    m.panel.visible = true
    m.menu.setFocus(true)
end sub

function watchedLabel(watched as string) as string
    if watched = "false" then return "Unwatched"
    if watched = "true" then return "Watched"
    return "All titles"
end function

sub onMenu(event as object)
    at = event.getData()
    if at = 0
        current = 0
        if m.view.watched = "false" then current = 1
        if m.view.watched = "true" then current = 2
        showChoices("watched", "Show", ["All titles", "Unwatched", "Watched"], ["", "false", "true"], current, "")
    else if at = 1 or at = 2
        kind = "genre"
        if at = 2 then kind = "year"
        if m.filters = invalid
            m.panelHint.text = "Loading…"
            uiRequest({ method: "GET", path: "/api/libraries/" + urlValue(m.library.id) + "/filters", filter: kind }, "onFilters")
            return
        end if
        openFilter(kind)
    else
        m.view.watched = ""
        m.view.genre = ""
        m.view.year = 0
        applyView()
        openFilters()
    end if
end sub

sub onFilters(event as object)
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        if not uiFailed(answer) then m.panelHint.text = answer.message
        return
    end if
    m.filters = { genres: [], years: [] }
    for each genre in httpArray(answer.data.genres)
        name = httpSentence(genre)
        if name <> "" then m.filters.genres.Push(name)
    end for
    for each year in httpArray(answer.data.years)
        if httpNumber(year, 0) > 0 then m.filters.years.Push(Int(year))
    end for
    if m.panel.visible and m.panelKind = "filters" then openFilter(event.getRoSGNode().spec.filter)
end sub

' A genre or a year to keep, or Any.
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
    m.panelTitle.text = title
    items = []
    for each label in labels
        items.Push({ title: label })
    end for
    m.choices.content = nodesContent(items)
    m.choices.checkedItem = current
    m.choices.jumpToItem = current
    m.panelHint.text = hint
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
    applyView()
    openFilters()
end sub

sub closePanel()
    m.panel.visible = false
    m.panelKind = ""
    m.viewChips.setFocus(true)
end sub

' Down from the chips goes to the titles, up from their top row back;
' right from the last tab reaches the sort and filters. Back closes the
' panel (a filter's choices go back to the filters). The Options key on a
' title offers what can be done with it.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.panel.visible
        if key = "back" or key = "left"
            if m.panelKind = "watched" or m.panelKind = "genre" or m.panelKind = "year"
                openFilters()
            else
                closePanel()
            end if
            return true
        end if
        return false
    end if
    chips = m.tabs.isInFocusChain() or m.viewChips.isInFocusChain()
    if key = "down" and chips and (m.grid.visible or m.rows.visible)
        focusContent()
        return true
    end if
    if key = "up" and (m.grid.isInFocusChain() or m.rows.isInFocusChain())
        m.tabs.setFocus(true)
        return true
    end if
    if key = "right" and m.tabs.isInFocusChain() and m.viewChips.visible
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
        if row <> invalid then uiTileOptions(row.getChild(position[1]), m.recommendedKeys[position[0]] = "continue_watching")
        return true
    end if
    return false
end function
