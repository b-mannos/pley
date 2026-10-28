sub init()
    m.frame = m.top.findNode("frame")
    m.fallback = m.top.findNode("fallback")
    m.poster = m.top.findNode("poster")
    m.track = m.top.findNode("track")
    m.fill = m.top.findNode("fill")
    m.watched = m.top.findNode("watched")
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.wide = false
    m.square = false
    m.progress = 0.0
end sub

' The title under the poster shows until a picture covers it, or when
' there is none: no observer per tile.
sub onContent()
    item = m.top.itemContent
    if item = invalid then return
    m.wide = false
    if item.hasField("wide") then m.wide = item.wide
    m.square = false
    if item.hasField("square") then m.square = item.square
    m.progress = 0.0
    if item.hasField("progress") then m.progress = item.progress
    m.watched.visible = item.hasField("watched") and item.watched = true
    m.title.text = item.title
    m.subtitle.text = item.subtitle
    m.fallback.text = item.title
    ' The scene's agent carries the token; it changes with each refresh.
    m.poster.setHttpAgent(m.top.getScene().getHttpAgent())
    onLayout()
    m.poster.uri = item.image
    onFocus()
end sub

sub onLayout()
    w = m.top.width
    if w <= 0 then return
    imageHeight = Int(w * 1.5)
    if m.wide then imageHeight = Int(w * 9 / 16)
    if m.square then imageHeight = Int(w)
    m.frame.width = w
    m.frame.height = imageHeight
    m.poster.translation = [4, 4]
    m.poster.width = w - 8
    m.poster.height = imageHeight - 8
    ' Decoded at the size it's drawn: small textures for small devices.
    m.poster.loadWidth = w - 8
    m.poster.loadHeight = imageHeight - 8
    m.fallback.translation = [16, 16]
    m.fallback.width = w - 32
    m.fallback.height = imageHeight - 32
    barWidth = w - 8
    m.track.translation = [4, imageHeight - 14]
    m.track.width = barWidth
    m.track.height = 10
    m.fill.translation = [4, imageHeight - 14]
    m.fill.width = Int(barWidth * m.progress)
    m.fill.height = 10
    m.track.visible = m.progress > 0
    m.fill.visible = m.progress > 0
    m.watched.translation = [w - 52, 12]
    m.title.translation = [0, imageHeight + 12]
    m.title.width = w
    m.subtitle.translation = [0, imageHeight + 44]
    m.subtitle.width = w
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.rowHasFocus and m.top.rowListHasFocus and m.top.gridHasFocus
    if focused
        m.frame.color = "0x38BDF8FF"
        m.title.color = "0x7DD3FCFF"
    else
        m.frame.color = "0x262B35FF"
        m.title.color = "0xEBEBEBFF"
    end if
end sub
