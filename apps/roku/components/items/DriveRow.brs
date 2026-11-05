sub init()
    m.background = m.top.findNode("background")
    m.bar = m.top.findNode("bar")
    m.icon = m.top.findNode("icon")
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.track = m.top.findNode("track")
    m.fill = m.top.findNode("fill")
    m.size = m.top.findNode("size")
end sub

sub onContent()
    line = m.top.itemContent
    if line = invalid then return
    m.icon.uri = line.hdPosterUrl
    m.title.text = line.title
    m.subtitle.text = line.description
    m.size.text = line.shortDescriptionLine2
    ' A folder's name alone, in the middle of its line.
    if line.description = ""
        m.title.translation = [120, 28]
    else
        m.title.translation = [120, 12]
    end if
    progress = 0
    if line.hasField("progress") then progress = line.progress
    m.track.visible = progress > 0
    m.fill.visible = progress > 0
    m.fill.width = Int(m.track.width * progress)
    onFocus()
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.listHasFocus
    m.bar.visible = focused
    if focused
        m.background.color = "0x2E3440FF"
        m.title.color = "0x7DD3FCFF"
        m.icon.blendColor = "0x7DD3FCFF"
    else
        m.background.color = "0x1E222AFF"
        m.title.color = "0xEBEBEBFF"
        m.icon.blendColor = "0xA1A9B6FF"
    end if
end sub
