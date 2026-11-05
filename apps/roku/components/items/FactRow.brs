sub init()
    m.focus = m.top.findNode("focus")
    m.label = m.top.findNode("label")
    m.value = m.top.findNode("value")
    m.heading = m.top.findNode("heading")
    m.isHeading = false
end sub

' The content's fields: title (the label, "" for a line without one),
' value, heading (true for a section's).
sub onContent()
    row = m.top.itemContent
    if row = invalid then return
    m.isHeading = row.heading = true
    m.heading.visible = m.isHeading
    m.label.visible = not m.isHeading
    m.value.visible = not m.isHeading
    if m.isHeading
        m.heading.text = row.title
    else
        m.label.text = row.title
        m.value.text = row.value
    end if
    onFocus()
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.listHasFocus and not m.isHeading
    m.focus.visible = focused
    if focused
        m.label.color = "0x16191EFF"
        m.value.color = "0x16191EFF"
    else
        m.label.color = "0xA1A9B6FF"
        m.value.color = "0xEBEBEBFF"
    end if
end sub
