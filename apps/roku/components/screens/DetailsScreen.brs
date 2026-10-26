sub init()
    m.backdrop = m.top.findNode("backdrop")
    m.poster = m.top.findNode("poster")
    m.column = m.top.findNode("column")
    m.text = m.top.findNode("text")
    m.heading = m.top.findNode("heading")
    m.line = m.top.findNode("line")
    m.genres = m.top.findNode("genres")
    m.tagline = m.top.findNode("tagline")
    m.summary = m.top.findNode("summary")
    m.buttons = m.top.findNode("buttons")
    m.note = m.top.findNode("note")
    m.message = m.top.findNode("message")
    m.column.visible = false
    ' The item's JSON, which the player starts with, and what it shows.
    m.item = invalid
    m.detail = invalid
    m.buttons.observeField("buttonSelected", "onButton")
    m.top.observeField("active", "onActive")
end sub

' Loaded each time it shows: back from the player, the place has changed.
' Asking for it also has the server read the start of the file ahead. The
' focus comes here at once, as the screen before had it: the remote works
' whatever the answer, and a second Back goes back.
sub onActive()
    if not m.top.active then return
    if m.buttons.visible
        m.buttons.setFocus(true)
    else
        m.top.setFocus(true)
    end if
    uiGet("/api/items/" + urlValue(m.top.params.itemId), "onDetail")
end sub

' A reload that fails leaves the details as they were.
sub onDetail(event as object)
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        if m.detail = invalid and not uiFailed(answer) then m.message.text = answer.message
        return
    end if
    m.item = answer.data
    m.detail = contentDetail(answer.data, m.global.session.server)
    m.message.visible = false
    m.heading.text = m.detail.heading
    m.line.text = m.detail.line
    m.genres.text = m.detail.genres
    m.tagline.text = m.detail.tagline
    m.summary.text = m.detail.summary
    layoutText()
    agent = m.top.getScene().getHttpAgent()
    m.backdrop.setHttpAgent(agent)
    m.backdrop.uri = m.detail.backdrop
    m.poster.setHttpAgent(agent)
    m.poster.uri = m.detail.poster
    hasButtons = m.detail.buttons.Count() > 0
    m.note.text = m.detail.note
    m.note.visible = not hasButtons
    if hasButtons then m.buttons.buttons = m.detail.buttons
    m.buttons.visible = hasButtons
    m.column.visible = true
    ' Only while this screen has the focus: a dialog over it keeps it.
    if m.top.isInFocusChain()
        if hasButtons
            m.buttons.setFocus(true)
        else
            m.top.setFocus(true)
        end if
    end if
end sub

' The labels with text, one under the other.
sub layoutText()
    shown = [m.heading]
    spacings = []
    for each part in [{ label: m.line, gap: 20 }, { label: m.genres, gap: 8 }, { label: m.tagline, gap: 28 }, { label: m.summary, gap: 20 }]
        if part.label.text <> ""
            spacings.Push(part.gap)
            shown.Push(part.label)
        end if
    end for
    m.text.removeChildrenIndex(m.text.getChildCount(), 0)
    m.text.appendChildren(shown)
    m.text.itemSpacings = spacings
end sub

sub onButton(event as object)
    if m.detail = invalid then return
    startMs = 0
    if m.detail.actions[event.getData()] = "resume" then startMs = m.detail.positionMs
    ' The player has the item already: one request less before it plays.
    uiNavigate({ to: "player", params: { itemId: m.detail.itemId, startMs: startMs, item: m.item } })
end sub
