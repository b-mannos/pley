sub init()
    m.keyboard = m.top.findNode("keyboard")
    m.grid = m.top.findNode("grid")
    m.message = m.top.findNode("message")
    m.debounce = m.top.findNode("debounce")
    m.started = false
    ' The search asked last: answers for earlier ones are dropped.
    m.query = ""
    m.path = ""
    m.lastFocus = m.keyboard
    m.keyboard.observeField("text", "onText")
    m.debounce.observeField("fire", "search")
    m.grid.observeField("itemSelected", "onSelected")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        m.lastFocus = m.keyboard
        if m.grid.isInFocusChain() then m.lastFocus = m.grid
        return
    end if
    m.lastFocus.setFocus(true)
    for each change in uiChangesSince()
        onTitleChanged(change)
    end for
end sub

sub onTitleChanged(change as object)
    if m.grid.visible then uiPatchTiles(m.grid.content, change)
end sub

' Searches once typing rests for half a second.
sub onText()
    m.debounce.control = "stop"
    m.debounce.control = "start"
end sub

sub search()
    query = m.keyboard.text.Trim()
    if query = m.query then return
    m.query = query
    if query = ""
        m.path = ""
        hideGrid()
        showMessage("Type to search your libraries.")
        return
    end if
    m.path = urlWithQuery("/api/search", { q: query, limit: 40 })
    uiRequest({ method: "GET", path: m.path, build: "tiles" }, "onResults")
end sub

sub onResults(event as object)
    request = event.getRoSGNode()
    if request.spec.path <> m.path then return
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then showMessage(answer.message)
        return
    end if
    if answer.data.shown = 0
        hideGrid()
        showMessage("Nothing found for “" + m.query + "”.")
        return
    end if
    m.message.visible = false
    m.grid.content = request.content
    m.grid.visible = true
end sub

' The keyboard takes the focus before the results it was on go.
sub hideGrid()
    if m.grid.isInFocusChain() then m.keyboard.setFocus(true)
    m.grid.visible = false
end sub

sub showMessage(text as string)
    m.message.text = text
    m.message.visible = true
end sub

sub onSelected(event as object)
    uiOpenTile(m.grid.content.getChild(event.getData()))
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if key = "right" and m.keyboard.isInFocusChain() and m.grid.visible
        m.grid.setFocus(true)
        return true
    end if
    if (key = "left" or key = "back") and m.grid.isInFocusChain()
        m.keyboard.setFocus(true)
        return true
    end if
    if key = "options" and m.grid.isInFocusChain()
        uiTileOptions(m.grid.content.getChild(m.grid.itemFocused), false)
        return true
    end if
    return false
end function
