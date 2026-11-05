sub init()
    m.background = m.top.findNode("background")
    m.name = m.top.findNode("name")
    m.address = m.top.findNode("address")
    m.tag = m.top.findNode("tag")
    m.warning = false
    m.found = false
    m.problem = false
end sub

sub onContent()
    row = m.top.itemContent
    if row = invalid then return
    m.name.text = row.title
    m.address.text = row.address
    m.tag.text = row.tag
    m.warning = row.warning
    m.found = row.found
    m.problem = row.hasField("problem") and row.problem = true
    onFocus()
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.listHasFocus
    if focused
        m.background.color = "0x38BDF8FF"
        m.name.color = "0x16191EFF"
        m.address.color = "0x16191EFF"
        m.tag.color = "0x16191EFF"
        return
    end if
    m.background.color = "0x262B35FF"
    m.name.color = "0xEBEBEBFF"
    ' "Can't reach it" in the danger colour, as on Android TV.
    if m.problem
        m.address.color = "0xEB6B70FF"
    else
        m.address.color = "0xA1A9B6FF"
    end if
    if m.warning
        m.tag.color = "0xEB6B70FF"
    else if m.found
        m.tag.color = "0x3FB27FFF"
    else
        m.tag.color = "0xA1A9B6FF"
    end if
end sub
