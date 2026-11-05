sub init()
    m.frame = m.top.findNode("frame")
    m.fallback = m.top.findNode("fallback")
    m.cover = m.top.findNode("cover")
    m.cells = [m.top.findNode("cell0"), m.top.findNode("cell1"), m.top.findNode("cell2"), m.top.findNode("cell3")]
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.images = []
end sub

' Four pictures make the mosaic; fewer, the first fills the cover; none,
' the name shows in the frame.
sub onContent()
    item = m.top.itemContent
    if item = invalid then return
    m.images = []
    if item.hasField("images") and item.images <> invalid then m.images = item.images
    m.title.text = item.title
    m.subtitle.text = item.subtitle
    m.fallback.text = item.title
    ' The scene's agent carries the token; it changes with each refresh.
    agent = m.top.getScene().getHttpAgent()
    m.cover.setHttpAgent(agent)
    for each cell in m.cells
        cell.setHttpAgent(agent)
    end for
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
    w = m.top.width
    if w <= 0 then return
    m.frame.width = w
    m.frame.height = w
    inner = w - 8
    m.cover.translation = [4, 4]
    m.cover.width = inner
    m.cover.height = inner
    m.cover.loadWidth = inner
    m.cover.loadHeight = inner
    cell = Int(inner / 2)
    for i = 0 to 3
        m.cells[i].translation = [4 + (i mod 2) * cell, 4 + Int(i / 2) * cell]
        m.cells[i].width = cell
        m.cells[i].height = cell
        m.cells[i].loadWidth = cell
        m.cells[i].loadHeight = cell
    end for
    m.fallback.translation = [16, 16]
    m.fallback.width = w - 32
    m.fallback.height = w - 32
    m.title.translation = [0, w + 12]
    m.title.width = w
    m.subtitle.translation = [0, w + 44]
    m.subtitle.width = w
end sub

' As a title's tile: the frame in the accent while focused.
sub onFocus()
    focused = m.top.itemHasFocus and m.top.gridHasFocus
    if focused
        m.frame.color = "0x38BDF8FF"
        m.title.color = "0x7DD3FCFF"
        m.fallback.color = "0x16191EFF"
    else
        m.frame.color = "0x262B35FF"
        m.title.color = "0xEBEBEBFF"
        m.fallback.color = "0xA1A9B6FF"
    end if
end sub
