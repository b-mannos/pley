sub init()
    m.chosen = m.top.findNode("chosen")
    m.focus = m.top.findNode("focus")
    m.name = m.top.findNode("name")
    m.value = m.top.findNode("value")
    m.state = m.top.findNode("state")
    m.danger = false
end sub

' The content's fields: title, value ("" for none), state ("On", "Off" or
' ""), danger, and the row's size (rowWidth, rowHeight), which its list
' gives.
sub onContent()
    row = m.top.itemContent
    if row = invalid then return
    width = row.rowWidth
    height = row.rowHeight
    for each node in [m.chosen, m.focus]
        node.width = width
        node.height = height
    end for
    typeSet(m.name, row.title)
    typeSet(m.value, row.value)
    typeSet(m.state, row.state)
    m.danger = row.danger = true
    m.state.translation = [width - 184, 0]
    m.state.width = 160
    m.state.height = height
    room = width - 48
    if row.state <> "" then room = width - 216
    m.name.width = room
    m.value.width = room
    ' With a value, the label above it; alone, in the middle.
    if row.value <> ""
        m.name.translation = [24, 10]
        m.name.height = 44
    else
        m.name.translation = [24, 0]
        m.name.height = height
    end if
    onFocus()
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.listHasFocus
    m.focus.visible = focused
    ' The category shown, or the row a panel is open for.
    m.chosen.visible = m.top.itemHasFocus and not m.top.listHasFocus
    if focused
        m.name.color = "0x16191EFF"
        m.name.font = typeFont(700, 28)
        m.value.color = "0x16191EFF"
        m.state.color = "0x16191EFF"
        return
    end if
    m.name.font = typeFont(400, 28)
    if m.danger
        m.name.color = "0xEB6B70FF"
    else
        m.name.color = "0xEBEBEBFF"
    end if
    m.value.color = "0xA1A9B6FF"
    m.state.color = "0xA1A9B6FF"
end sub
