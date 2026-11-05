sub init()
    m.body = m.top.findNode("body")
    m.card = m.top.findNode("card")
    m.frame = m.top.findNode("frame")
    m.fallback = m.top.findNode("fallback")
    m.poster = m.top.findNode("poster")
    m.track = m.top.findNode("track")
    m.fill = m.top.findNode("fill")
    m.watched = m.top.findNode("watched")
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.badgeBack = m.top.findNode("badgeBack")
    m.badge = m.top.findNode("badge")
    m.qualityBacks = [m.top.findNode("qualityBack0"), m.top.findNode("qualityBack1")]
    m.qualities = [m.top.findNode("quality0"), m.top.findNode("quality1")]
    m.corners = m.top.findNode("corners")
    m.ring = m.top.findNode("ring")
    m.look = m.top.findNode("look")
    m.lookScale = m.top.findNode("lookScale")
    m.lookRing = m.top.findNode("lookRing")
    m.wide = false
    m.square = false
    m.progress = 0.0
    m.titleText = ""
    ' The look drawn (focused or not), and whether the tile has shown one:
    ' a tile given new content takes its look at once.
    m.focused = false
    m.looked = false
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
    m.titleText = item.title
    ' See all, a row's Try again or note, a grid's Look further and Request
    ' more seasons: their words in the frame only.
    if item.hasField("kind") and (item.kind = "more" or item.kind = "retry" or item.kind = "note" or item.kind = "further" or item.kind = "askMore") then m.titleText = ""
    typeSet(m.subtitle, item.subtitle)
    m.subtitle.color = "0xA1A9B6FF"
    if item.hasField("subtitleColor") then m.subtitle.color = item.subtitleColor
    typeSet(m.fallback, item.title)
    badge = ""
    if item.hasField("badge") then badge = item.badge
    m.badge.text = badge
    m.badge.visible = badge <> ""
    m.badgeBack.visible = badge <> ""
    if badge <> "" and item.hasField("badgeColor") then m.badgeBack.color = item.badgeColor
    qualities = []
    if item.hasField("qualityBadges") and item.qualityBadges <> invalid then qualities = item.qualityBadges
    for i = 0 to m.qualities.Count() - 1
        text = ""
        if i < qualities.Count() then text = qualities[i]
        m.qualities[i].text = text
        m.qualities[i].visible = text <> ""
        m.qualityBacks[i].visible = text <> ""
    end for
    ' The scene's agent carries the token; it changes with each refresh.
    m.poster.setHttpAgent(m.top.getScene().getHttpAgent())
    m.looked = false
    onLayout()
    m.poster.uri = item.image
    onFocus()
end sub

sub onLayout()
    cell = m.top.width
    if cell <= 0 then return
    ' The tile inside its cell, with the room its focus look takes left
    ' and right and above it (contentTileCell's inverse); the captions
    ' below leave room enough under it.
    w = Int(cell / 1.05 + 0.5) - 20
    m.body.translation = [Int((cell - w) / 2), 20]
    imageHeight = Int(w * 1.5)
    if m.wide then imageHeight = Int(w * 9 / 16)
    if m.square then imageHeight = Int(w)
    m.frame.width = w
    m.frame.height = imageHeight
    m.poster.width = w
    m.poster.height = imageHeight
    ' Decoded at the size it's drawn: small textures for small devices.
    m.poster.loadWidth = w
    m.poster.loadHeight = imageHeight
    m.corners.width = w
    m.corners.height = imageHeight
    ' The ring 4 pixels out from the picture, 6 wide.
    m.ring.translation = [-10, -10]
    m.ring.width = w + 20
    m.ring.height = imageHeight + 20
    m.card.scaleRotateCenter = [w / 2, imageHeight / 2]
    m.fallback.translation = [16, 16]
    m.fallback.width = w - 32
    m.fallback.height = imageHeight - 32
    ' How far along it is: a bar along the picture's foot, inside its
    ' corners.
    m.track.translation = [8, imageHeight - 14]
    m.track.width = w - 16
    m.track.height = 6
    m.fill.translation = [8, imageHeight - 14]
    m.fill.width = Int((w - 16) * m.progress)
    m.fill.height = 6
    m.track.visible = m.progress > 0
    m.fill.visible = m.progress > 0
    m.watched.translation = [w - 52, 12]
    ' The badge at the poster's top left, as wide as its words; cut short
    ' only when they don't fit (a label as wide as it measures can lose
    ' its last letters to rounding).
    if m.badge.visible
        m.badge.width = 0
        textWidth = m.badge.boundingRect().width
        if textWidth > w - 48
            textWidth = w - 48
            m.badge.width = textWidth
        end if
        m.badge.translation = [24, 12]
        m.badgeBack.translation = [12, 12]
        m.badgeBack.width = textWidth + 24
    end if
    ' The quality badges side by side at the poster's foot, above how far
    ' along it is; one that doesn't fit whole isn't drawn.
    x = 12
    y = imageHeight - 46
    if m.progress > 0 then y = imageHeight - 56
    for i = 0 to m.qualities.Count() - 1
        label = m.qualities[i]
        back = m.qualityBacks[i]
        if label.text <> ""
            label.width = 0
            chipWidth = label.boundingRect().width + 20
            fits = x + chipWidth <= w - 12
            label.visible = fits
            back.visible = fits
            label.translation = [x + 10, y]
            back.translation = [x, y]
            back.width = chipWidth
            x = x + chipWidth + 8
        end if
    end for
    ' The captions clear of the grown picture and its ring.
    m.title.translation = [0, imageHeight + 26]
    m.title.width = w
    m.subtitle.translation = [0, imageHeight + 60]
    m.subtitle.width = w
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.rowHasFocus and m.top.rowListHasFocus and m.top.gridHasFocus
    if focused = m.focused and m.looked then return
    animate = m.looked
    m.focused = focused
    m.looked = true
    ' The title's weight switches, never animates.
    weight = 500
    if focused then weight = 700
    typeSet(m.title, m.titleText, typeFont(weight, 26))
    if focused
        m.fallback.color = "0xEBEBEBFF"
        scale = 1.05
        ring = 1.0
    else
        m.fallback.color = "0xA1A9B6FF"
        scale = 1.0
        ring = 0.0
    end if
    if not animate
        m.look.control = "stop"
        m.card.scale = [scale, scale]
        m.ring.opacity = ring
        return
    end if
    ' From where the look is now: focus passing over a row turns a tile
    ' back from part of the way.
    m.lookScale.keyValue = [m.card.scale, [scale, scale]]
    m.lookRing.keyValue = [m.ring.opacity, ring]
    if focused
        m.look.duration = 0.15
        m.look.easeFunction = "outCubic"
    else
        m.look.duration = 0.12
        m.look.easeFunction = "inCubic"
    end if
    m.look.control = "start"
end sub
