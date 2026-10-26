sub init()
    m.label = m.top.findNode("label")
end sub

sub onText()
    m.label.text = m.top.text
end sub

' Keys this button doesn't take go up to its parent, the Video node, as if
' the video had the focus: trick play, pause and the rest work as usual.
function onKeyEvent(key as string, press as boolean) as boolean
    if not m.top.visible then return false
    if key = "OK"
        if press then m.top.pressed = true
        return true
    end if
    if key = "back"
        if press then m.top.dismissed = true
        return true
    end if
    return false
end function
