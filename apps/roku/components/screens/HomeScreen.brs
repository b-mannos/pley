sub init()
    m.menu = m.top.findNode("menu")
    m.rows = m.top.findNode("rows")
    m.message = m.top.findNode("message")
    m.started = false
    m.libraries = []
    ' { title, wide, size } of each row shown.
    m.rowInfo = []
    m.lastFocus = m.menu
    m.menu.observeField("itemSelected", "onMenu")
    m.rows.observeField("rowItemSelected", "onTile")
    m.top.observeField("active", "onActive")
    buildMenu()
end sub

sub onActive()
    if not m.top.active
        ' Covered by another screen: focus comes back to the same place.
        m.lastFocus = m.menu
        if m.rows.isInFocusChain() then m.lastFocus = m.rows
        return
    end if
    if not m.started
        m.started = true
        m.menu.setFocus(true)
        load()
        return
    end if
    ' Back from a title: Continue Watching moved on.
    load()
    if m.rows.visible
        m.lastFocus.setFocus(true)
    else
        m.menu.setFocus(true)
    end if
end sub

' The libraries (the menu) and the rows, asked for together; the task
' makes the rows' ContentNodes.
sub load()
    if m.rowInfo.Count() = 0 then showMessage("Loading…")
    uiGet("/api/libraries", "onLibraries")
    uiRequest({ method: "GET", path: "/api/hubs/home", build: "rows" }, "onRows")
end sub

sub onLibraries(event as object)
    answer = event.getData()
    if not answer.ok then return
    m.libraries = contentLibraries(answer.data)
    buildMenu()
end sub

sub onRows(event as object)
    answer = event.getData()
    if not answer.ok
        ' A refresh that fails leaves the rows as they were.
        if m.rowInfo.Count() = 0 and not uiFailed(answer) then showMessage(answer.message)
        return
    end if
    m.rowInfo = answer.data.rows
    if m.rowInfo.Count() = 0
        showMessage("Nothing to watch yet. Add a library on the server, then come back.")
        return
    end if
    render(event.getRoSGNode().content)
end sub

sub buildMenu()
    items = [{ title: "Home" }]
    for each library in m.libraries
        items.Push({ title: library.name })
    end for
    items.Push({ title: "Search" })
    items.Push({ title: "Settings" })
    focused = m.menu.itemFocused
    m.menu.content = nodesContent(items)
    if focused > 0 and focused < items.Count() then m.menu.jumpToItem = focused
end sub

sub showMessage(text as string)
    m.rows.visible = false
    m.message.text = text
    m.message.visible = true
end sub

sub render(content as object)
    keep = m.rows.rowItemFocused
    sizes = []
    heights = []
    for each row in m.rowInfo
        ' A 16:9 picture or a poster, and two lines of text under it.
        if row.wide
            sizes.Push([400, 300])
        else
            sizes.Push([220, 410])
        end if
        heights.Push(sizes.Peek()[1] + 60)
    end for
    m.rows.rowItemSize = sizes
    m.rows.rowHeights = heights
    m.rows.content = content
    if keep <> invalid and keep.Count() = 2 and keep[0] < m.rowInfo.Count()
        if keep[1] < m.rowInfo[keep[0]].size then m.rows.jumpToRowItem = keep
    end if
    wasShown = m.rows.visible
    m.message.visible = false
    m.rows.visible = true
    if not wasShown and m.top.active then m.rows.setFocus(true)
end sub

sub onTile(event as object)
    position = event.getData()
    row = m.rows.content.getChild(position[0])
    if row <> invalid then uiOpenTile(row.getChild(position[1]))
end sub

sub onMenu(event as object)
    index = event.getData()
    if index = 0
        load()
        if m.rows.visible then m.rows.setFocus(true)
    else if index <= m.libraries.Count()
        uiNavigate({ to: "library", params: { library: m.libraries[index - 1] } })
    else if index = m.libraries.Count() + 1
        uiNavigate({ to: "search" })
    else
        uiNavigate({ to: "settings" })
    end if
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if (key = "left" or key = "back") and m.rows.isInFocusChain()
        m.menu.setFocus(true)
        return true
    end if
    if key = "right" and m.menu.isInFocusChain() and m.rows.visible
        m.rows.setFocus(true)
        return true
    end if
    return false
end function
