sub init()
    m.back = m.top.findNode("back")
    m.mark = m.top.findNode("mark")
    m.marked = false
    m.icon = m.top.findNode("icon")
    m.title = m.top.findNode("title")
    m.count = m.top.findNode("count")
    m.inner = false
    m.current = false
    m.heading = false
end sub

sub onContent()
    item = m.top.itemContent
    if item = invalid then return
    m.inner = item.inner = true
    m.current = item.current = true
    m.heading = item.heading = true
    if m.heading
        ' "REGIONS", "MORE GROUPS": a section's title, set at the bottom.
        typeSet(m.title, UCase(item.title), typeFont(700, 26))
        m.title.vertAlign = "bottom"
    else
        typeSet(m.title, item.title, typeFont(500, 26))
        m.title.vertAlign = "center"
    end if
    typeSet(m.count, item.countText)
    ' A region's mark: Lucide's chevron, right while it is closed and down
    ' while open.
    m.marked = item.mark <> ""
    m.mark.visible = m.marked
    if item.mark = "+" then m.mark.uri = "pkg:/images/icons/chevron-right.png"
    if item.mark = "−" then m.mark.uri = "pkg:/images/icons/chevron-down.png"
    icon = ""
    if item.hasField("icon") then icon = item.icon
    m.icon.visible = icon <> ""
    if icon <> "" then m.icon.uri = "pkg:/images/icons/" + icon + ".png"
    onLayout()
    onFocus()
end sub

sub onLayout()
    w = m.top.width
    h = m.top.height
    if w <= 0 or h <= 0 then return
    m.back.width = w
    m.back.height = h
    x = 16
    if m.inner then x = 48
    m.mark.translation = [x - 6, Int((h - 28) / 2)]
    textX = x + 28
    if not m.marked then textX = x
    ' An icon before the name: Search, Favourites, Recently watched, All
    ' channels.
    if m.icon.visible
        m.icon.translation = [textX, Int((h - 32) / 2)]
        textX = textX + 48
    end if
    m.title.translation = [textX, 0]
    m.title.width = w - textX - 120
    m.title.height = h - 8
    if not m.heading then m.title.height = h
    m.count.translation = [w - 112, 0]
    m.count.width = 96
    m.count.height = h
end sub

sub onFocus()
    if m.heading
        m.back.opacity = 0
        m.title.color = "0x8B95A5FF"
        return
    end if
    focused = m.top.itemHasFocus and m.top.listHasFocus
    if focused
        m.back.opacity = 1.0
        m.title.color = "0x16191EFF"
        m.count.color = "0x16191EFF"
        m.mark.blendColor = "0x16191EFF"
        m.icon.blendColor = "0x16191EFF"
        return
    end if
    ' The category shown on a faint accent pill.
    if m.current
        m.back.opacity = 0.15
        m.title.color = "0x7DD3FCFF"
    else
        m.back.opacity = 0
        m.title.color = "0xA1A9B6FF"
    end if
    m.icon.blendColor = m.title.color
    m.count.color = "0x8B95A5FF"
    m.mark.blendColor = "0x8B95A5FF"
end sub
