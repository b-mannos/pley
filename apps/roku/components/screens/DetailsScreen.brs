sub init()
    m.backdrop = m.top.findNode("backdrop")
    m.heading = m.top.findNode("heading")
    m.line = m.top.findNode("line")
    m.genres = m.top.findNode("genres")
    m.tagline = m.top.findNode("tagline")
    m.summary = m.top.findNode("summary")
    m.buttons = m.top.findNode("buttons")
    m.note = m.top.findNode("note")
    m.message = m.top.findNode("message")
    ' The item's JSON, which the player starts with, and what it shows.
    m.item = invalid
    m.detail = invalid
    m.buttons.observeField("buttonSelected", "onButton")
    m.top.observeField("active", "onActive")
end sub

' Loaded each time it shows: back from the player, the place has changed.
' Asking for it also has the server read the start of the file ahead.
sub onActive()
    if not m.top.active then return
    if m.detail = invalid then m.top.setFocus(true)
    uiGet("/api/items/" + urlValue(m.top.params.itemId), "onDetail")
end sub

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
    m.backdrop.setHttpAgent(m.top.getScene().getHttpAgent())
    m.backdrop.uri = m.detail.backdrop
    hasButtons = m.detail.buttons.Count() > 0
    m.buttons.visible = hasButtons
    m.note.visible = not hasButtons
    m.note.text = m.detail.note
    if hasButtons
        m.buttons.buttons = m.detail.buttons
        if m.top.active then m.buttons.setFocus(true)
    end if
end sub

sub onButton(event as object)
    if m.detail = invalid then return
    startMs = 0
    if m.detail.actions[event.getData()] = "resume" then startMs = m.detail.positionMs
    ' The player has the item already: one request less before it plays.
    uiNavigate({ to: "player", params: { itemId: m.detail.itemId, startMs: startMs, item: m.item } })
end sub
