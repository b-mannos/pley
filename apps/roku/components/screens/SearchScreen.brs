sub init()
    m.keyboard = m.top.findNode("keyboard")
    m.grid = m.top.findNode("grid")
    m.live = m.top.findNode("live")
    ' The Live TV channels found, as the cards show them.
    m.channels = []
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
    m.live.observeField("rowItemSelected", "onChannelSelected")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        m.lastFocus = m.keyboard
        if m.grid.isInFocusChain() then m.lastFocus = m.grid
        if m.live.isInFocusChain() then m.lastFocus = m.live
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
        showChannels([])
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
    showChannels(answer.data.channels)
    if answer.data.shown = 0 and m.channels.Count() = 0
        hideGrid()
        showMessage("Nothing found for “" + m.query + "”.")
        return
    end if
    m.message.visible = false
    m.grid.content = request.content
    m.grid.visible = answer.data.shown > 0
    if not m.grid.visible and m.grid.isInFocusChain() then focusResults()
end sub

' The Live TV channels found (GET /api/search's channels, for those Live
' TV is open to) as a row of cards above the titles, which move down.
sub showChannels(channels as dynamic)
    m.channels = httpArray(channels)
    shown = m.channels.Count() > 0
    if not shown and m.live.isInFocusChain() then focusResults()
    m.live.visible = shown
    if shown
        now = CreateObject("roDateTime").AsSeconds()
        twelve = CreateObject("roDeviceInfo").GetClockFormat() <> "24h"
        cards = []
        for each channel in m.channels
            cards.Push(liveCard(channel, now, twelve))
        end for
        m.live.content = nodesRows([{ title: "Live TV", tiles: cards }])
        m.grid.translation = [780, 470]
        m.grid.numRows = 1
    else
        m.grid.translation = [780, 170]
        m.grid.numRows = 2
    end if
end sub

' The focus on the results: the channels first, else the titles, else the
' keyboard.
sub focusResults()
    if m.live.visible
        m.live.setFocus(true)
    else if m.grid.visible
        m.grid.setFocus(true)
    else
        m.keyboard.setFocus(true)
    end if
end sub

' OK on a channel watches it; up and down in the player go through what
' the search found.
sub onChannelSelected(event as object)
    position = event.getData()
    at = position[1]
    if at < 0 or at >= m.channels.Count() then return
    uiNavigate({ to: "livePlayer", params: { channel: m.channels[at], category: "search:" + m.query, categoryName: "Results for “" + m.query + "”" } })
end sub

' The keyboard takes the focus before the results it was on go.
sub hideGrid()
    if m.grid.isInFocusChain() or m.live.isInFocusChain() then m.keyboard.setFocus(true)
    m.grid.visible = false
    m.live.visible = false
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
    if key = "right" and m.keyboard.isInFocusChain() and (m.grid.visible or m.live.visible)
        focusResults()
        return true
    end if
    if (key = "left" or key = "back") and (m.grid.isInFocusChain() or m.live.isInFocusChain())
        m.keyboard.setFocus(true)
        return true
    end if
    if key = "down" and m.live.isInFocusChain() and m.grid.visible
        m.grid.setFocus(true)
        return true
    end if
    if key = "up" and m.grid.isInFocusChain() and m.live.visible
        m.live.setFocus(true)
        return true
    end if
    if key = "options" and m.grid.isInFocusChain()
        uiTileOptions(m.grid.content.getChild(m.grid.itemFocused), false)
        return true
    end if
    return false
end function
