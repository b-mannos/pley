sub init()
    m.top.focusable = true
    m.backs = []
    m.labels = []
    m.top.observeField("focusedChild", "onLook")
end sub

' Chips side by side, 16 pixels apart.
sub onChips()
    m.top.removeChildrenIndex(m.top.getChildCount(), 0)
    m.backs = []
    m.labels = []
    x = 0
    for each chip in m.top.chips
        back = m.top.createChild("Rectangle")
        back.translation = [x, 0]
        back.width = chip.width
        back.height = 56
        label = m.top.createChild("Label")
        label.translation = [x, 0]
        label.width = chip.width
        label.height = 56
        label.horizAlign = "center"
        label.vertAlign = "center"
        label.font = "font:SmallSystemFont"
        label.text = chip.label
        m.backs.Push(back)
        m.labels.Push(label)
        x = x + chip.width + 16
    end for
    if m.top.focusIndex >= m.backs.Count() then m.top.focusIndex = 0
    onLook()
end sub

sub onLook()
    focused = m.top.hasFocus()
    for i = 0 to m.backs.Count() - 1
        if focused and i = m.top.focusIndex
            m.backs[i].color = "0x38BDF8FF"
            m.labels[i].color = "0x16191EFF"
        else if i = m.top.current
            m.backs[i].color = "0x2E3440FF"
            m.labels[i].color = "0xEBEBEBFF"
        else
            m.backs[i].color = "0x1E222AFF"
            m.labels[i].color = "0xA1A9B6FF"
        end if
    end for
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if key = "left" and m.top.focusIndex > 0
        m.top.focusIndex = m.top.focusIndex - 1
        return true
    end if
    if key = "right" and m.top.focusIndex < m.backs.Count() - 1
        m.top.focusIndex = m.top.focusIndex + 1
        return true
    end if
    if key = "OK"
        m.top.chosen = m.top.focusIndex
        return true
    end if
    return false
end function
