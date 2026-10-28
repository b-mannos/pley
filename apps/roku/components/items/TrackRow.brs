sub init()
    m.background = m.top.findNode("background")
    m.bar = m.top.findNode("bar")
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.duration = m.top.findNode("duration")
end sub

sub onContent()
    track = m.top.itemContent
    if track = invalid then return
    m.title.text = track.title
    m.subtitle.text = track.subtitle
    m.duration.text = track.duration
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.listHasFocus
    m.bar.visible = focused
    if focused
        m.background.color = "0x2E3440FF"
        m.title.color = "0x7DD3FCFF"
    else
        m.background.color = "0x1E222AFF"
        m.title.color = "0xEBEBEBFF"
    end if
end sub
