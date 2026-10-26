sub init()
    m.label = m.top.findNode("label")
end sub

sub onText()
    m.label.text = m.top.text
end sub

' Keys this button doesn't take go up to its parent, the Video node, and on
' to the player: Back leaves the player as it always does.
function onKeyEvent(key as string, press as boolean) as boolean
    if not m.top.visible then return false
    if key = "OK"
        if press then m.top.pressed = true
        return true
    end if
    if key = "play" or key = "fastforward" or key = "rewind" or key = "replay"
        if press then m.top.playKey = key
        return true
    end if
    return false
end function
