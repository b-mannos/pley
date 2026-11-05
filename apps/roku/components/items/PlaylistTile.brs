sub init()
    m.body = m.top.findNode("body")
    m.card = m.top.findNode("card")
    m.frame = m.top.findNode("frame")
    m.fallback = m.top.findNode("fallback")
    m.cover = m.top.findNode("cover")
    m.cells = [m.top.findNode("cell0"), m.top.findNode("cell1"), m.top.findNode("cell2"), m.top.findNode("cell3")]
    m.corners = m.top.findNode("corners")
    m.ring = m.top.findNode("ring")
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.look = m.top.findNode("look")
    m.lookScale = m.top.findNode("lookScale")
    m.lookRing = m.top.findNode("lookRing")
    m.images = []
    m.titleText = ""
    ' The look drawn, and whether one was: new content takes its look at
    ' once.
    m.focused = false
    m.looked = false
end sub

' Four pictures make the mosaic; fewer, the first fills the cover; none,
' the name shows in the well.
sub onContent()
    item = m.top.itemContent
    if item = invalid then return
    m.images = []
    if item.hasField("images") and item.images <> invalid then m.images = item.images
    m.titleText = item.title
    typeSet(m.subtitle, item.subtitle)
    typeSet(m.fallback, item.title)
    ' The scene's agent carries the token; it changes with each refresh.
    agent = m.top.getScene().getHttpAgent()
    m.cover.setHttpAgent(agent)
    for each cell in m.cells
        cell.setHttpAgent(agent)
    end for
    m.looked = false
    onLayout()
    mosaic = m.images.Count() >= 4
    m.cover.visible = not mosaic
    m.cover.uri = ""
    if not mosaic and m.images.Count() > 0 then m.cover.uri = m.images[0]
    for i = 0 to 3
        m.cells[i].visible = mosaic
        m.cells[i].uri = ""
        if mosaic then m.cells[i].uri = m.images[i]
    end for
    onFocus()
end sub

sub onLayout()
    cell = m.top.width
    if cell <= 0 then return
    ' Inside its cell with the room its focus look takes, as a tile
    ' (Content.brs's contentTileCell).
    w = Int(cell / 1.05 + 0.5) - 20
    m.body.translation = [Int((cell - w) / 2), 20]
    m.card.scaleRotateCenter = [w / 2, w / 2]
    m.frame.width = w
    m.frame.height = w
    m.corners.width = w
    m.corners.height = w
    m.ring.translation = [-10, -10]
    m.ring.width = w + 20
    m.ring.height = w + 20
    m.cover.width = w
    m.cover.height = w
    m.cover.loadWidth = w
    m.cover.loadHeight = w
    half = Int(w / 2)
    for i = 0 to 3
        m.cells[i].translation = [(i mod 2) * half, Int(i / 2) * half]
        m.cells[i].width = half
        m.cells[i].height = half
        m.cells[i].loadWidth = half
        m.cells[i].loadHeight = half
    end for
    m.fallback.translation = [16, 16]
    m.fallback.width = w - 32
    m.fallback.height = w - 32
    m.title.translation = [0, w + 26]
    m.title.width = w
    m.subtitle.translation = [0, w + 60]
    m.subtitle.width = w
end sub

' As a title's tile: grown with its ring while focused, its title bold.
sub onFocus()
    focused = m.top.itemHasFocus and m.top.gridHasFocus
    if focused = m.focused and m.looked then return
    animate = m.looked
    m.focused = focused
    m.looked = true
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
    m.look.control = "stop"
    if not animate
        m.card.scale = [scale, scale]
        m.ring.opacity = ring
        return
    end if
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
