sub init()
    m.heading = m.top.findNode("heading")
    m.body = m.top.findNode("body")
    m.button = m.top.findNode("button")
    m.top.layoutDirection = "vert"
    m.top.horizAlignment = "center"
    m.top.focusable = true
    m.button.observeField("buttonSelected", "onButton")
    ' The page gives the message the focus: its button takes it.
    m.top.observeField("focusedChild", "onFocusMoved")
    onText()
end sub

' Only the parts with words stay in the column, so a sentence alone sits
' where the title would.
sub onText()
    ' Fields set before init: init lays them out.
    if m.heading = invalid then return
    m.heading.text = m.top.title
    m.body.text = m.top.text
    m.heading.width = m.top.width
    m.body.width = m.top.width
    focused = m.button.isInFocusChain()
    shown = []
    spacings = []
    if m.top.title <> "" then shown.Push(m.heading)
    if m.top.text <> ""
        if shown.Count() > 0 then spacings.Push(16)
        shown.Push(m.body)
    end if
    if m.top.action <> ""
        if FormatJson(m.button.buttons) <> FormatJson([m.top.action]) then m.button.buttons = [m.top.action]
        if shown.Count() > 0 then spacings.Push(40)
        shown.Push(m.button)
    end if
    m.top.removeChildrenIndex(m.top.getChildCount(), 0)
    m.top.appendChildren(shown)
    m.top.itemSpacings = spacings
    ' Taken out and put back, the button would lose the focus; taken out
    ' for good, the message keeps it, so the page still has it and can
    ' give it on (to its titles once they come).
    if focused and m.top.action <> ""
        m.button.setFocus(true)
    else if focused
        m.top.setFocus(true)
    end if
end sub

sub onFocusMoved()
    if m.top.hasFocus() and m.top.action <> "" then m.button.setFocus(true)
end sub

sub onButton()
    m.top.chosen = true
end sub
