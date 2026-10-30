sub init()
    m.back = m.top.findNode("back")
    m.label = m.top.findNode("label")
    m.track = m.top.findNode("track")
    m.fill = m.top.findNode("fill")
end sub

sub onLook()
    m.label.text = m.top.text
    m.label.width = 0
    textWidth = m.label.boundingRect().width
    if textWidth > 900 then textWidth = 900
    m.label.width = textWidth
    m.back.width = textWidth + 36
    m.back.color = m.top.color
    m.fill.color = m.top.color
    progress = m.top.progress
    shown = progress >= 0
    m.track.visible = shown
    m.fill.visible = shown
    if shown
        if progress > 1 then progress = 1
        m.track.translation = [textWidth + 60, 17]
        m.fill.translation = [textWidth + 60, 17]
        m.fill.width = Int(m.track.width * progress)
    end if
end sub
