' Watch history (GET /api/history), 50 plays at a time, the next as the
' focus nears the end. The Options key takes the play in focus out at
' once (DELETE /api/history/{id}), as Android TV's cross does: it goes
' from the list, a toast says so, and the focus is on the play after it.

sub init()
    m.count = m.top.findNode("count")
    m.entries = m.top.findNode("entries")
    m.message = m.top.findNode("message")
    m.started = false
    m.content = CreateObject("roSGNode", "ContentNode")
    m.entries.content = m.content
    ' Plays listed so far (the next page's offset) and in all.
    m.offset = 0
    m.total = -1
    m.loading = false
    ' The plays being taken out, by their id, each asked once.
    m.removing = {}
    m.entries.observeField("itemSelected", "onEntry")
    m.entries.observeField("itemFocused", "onFocused")
    m.message.observeField("chosen", "onRetry")
    m.top.observeField("active", "onActive")
end sub

function pageSize() as integer
    return 50
end function

sub onActive()
    if not m.top.active then return
    if not m.started
        m.started = true
        loadMore()
    end if
    if m.content.getChildCount() > 0
        m.entries.setFocus(true)
    else if m.message.action <> ""
        m.message.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

' Retry, after the history couldn't load.
sub onRetry()
    m.message.title = ""
    m.message.text = "Loading…"
    m.message.action = ""
    m.top.setFocus(true)
    loadMore()
end sub

sub loadMore()
    if m.loading or (m.total >= 0 and m.offset >= m.total) then return
    m.loading = true
    uiRequest({ method: "GET", path: "/api/history", query: { offset: m.offset, limit: pageSize() }, build: "history" }, "onPage")
end sub

sub onPage(event as object)
    m.loading = false
    answer = event.getData()
    if not answer.ok
        ' With nothing listed, the page says so, with Retry, as on Android TV.
        if m.content.getChildCount() = 0 and not uiFailed(answer)
            m.message.title = "Couldn't load your watch history"
            m.message.text = answer.message
            m.message.action = "Retry"
            if m.top.hasFocus() then m.message.setFocus(true)
        end if
        return
    end if
    m.message.title = ""
    m.message.action = ""
    m.total = answer.data.total
    m.offset = m.offset + answer.data.listed
    if answer.data.listed = 0 then m.total = m.offset
    rows = event.getRoSGNode().content
    if rows <> invalid and rows.getChildCount() > 0 then m.content.appendChildren(rows.getChildren(-1, 0))
    showCount()
    if m.content.getChildCount() = 0
        m.message.text = "No history yet. Movies, episodes and songs you finish show up here."
        return
    end if
    m.message.visible = false
    if m.top.hasFocus() then m.entries.setFocus(true)
end sub

sub showCount()
    if m.total = 1
        m.count.text = "1 play"
    else if m.total >= 0
        m.count.text = urlValue(m.total) + " plays"
    end if
end sub

sub onFocused(event as object)
    if event.getData() >= m.content.getChildCount() - 5 then loadMore()
end sub

sub onEntry(event as object)
    uiOpenTile(m.content.getChild(event.getData()))
end sub

' The Options key: the play in focus out of the history, at once.
sub removeFocused()
    entry = m.content.getChild(m.entries.itemFocused)
    if entry = invalid then return
    key = urlValue(entry.historyId)
    if m.removing.DoesExist(key) then return
    m.removing[key] = true
    uiRequest({ method: "DELETE", path: "/api/history/" + key, historyId: entry.historyId, title: entry.title }, "onRemoved")
end sub

' Taken out: its row goes, by its id (rows taken out meanwhile moved the
' others), and the focus stays in its place, on the play after it (the
' one before, for the last). Refused: a toast says why.
sub onRemoved(event as object)
    spec = event.getRoSGNode().spec
    gone = spec.historyId
    m.removing.Delete(urlValue(gone))
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then uiToast(answer.message, true)
        return
    end if
    focused = m.entries.itemFocused
    for i = m.content.getChildCount() - 1 to 0 step -1
        if m.content.getChild(i).historyId = gone
            m.content.removeChildIndex(i)
            if i < focused then focused = focused - 1
        end if
    end for
    uiToast("Removed “" + httpSentence(spec.title) + "”")
    count = m.content.getChildCount()
    if count > 0
        if focused >= count then focused = count - 1
        if focused < 0 then focused = 0
        m.entries.jumpToItem = focused
    end if
    m.total = m.total - 1
    m.offset = m.offset - 1
    showCount()
    if m.content.getChildCount() = 0
        m.message.text = "No history yet. Movies, episodes and songs you finish show up here."
        m.message.visible = true
        m.top.setFocus(true)
    end if
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if press and key = "options" and m.content.getChildCount() > 0
        removeFocused()
        return true
    end if
    return false
end function
