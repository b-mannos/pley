sub init()
    m.background = m.top.findNode("background")
    m.bar = m.top.findNode("bar")
    m.title = m.top.findNode("title")
    m.subtitle = m.top.findNode("subtitle")
    m.duration = m.top.findNode("duration")
    ' A disc's heading (kind "disc"; QueueRow has none): its words alone,
    ' never drawn focused.
    m.heading = m.top.findNode("heading")
    m.isHeading = false
    ' The track playing (current), in Up next or an album's list: in bold,
    ' in the accent.
    m.isCurrent = false
end sub

sub onContent()
    track = m.top.itemContent
    if track = invalid then return
    m.isHeading = m.heading <> invalid and track.hasField("kind") and track.kind = "disc"
    if m.heading <> invalid
        m.heading.visible = m.isHeading
        m.heading.text = ""
        if m.isHeading then typeSet(m.heading, track.title)
    end if
    m.isCurrent = track.hasField("current") and track.current = true
    weight = 400
    if m.isCurrent then weight = 700
    m.background.visible = not m.isHeading
    m.title.visible = not m.isHeading
    typeSet(m.title, track.title, typeFont(weight, 32))
    typeSet(m.subtitle, track.subtitle)
    typeSet(m.duration, track.duration)
    onFocus()
end sub

sub onFocus()
    focused = m.top.itemHasFocus and m.top.listHasFocus and not m.isHeading
    m.bar.visible = focused
    if focused
        m.background.color = "0x2E3440FF"
        m.title.color = "0x7DD3FCFF"
    else
        m.background.color = "0x1E222AFF"
        m.title.color = "0xEBEBEBFF"
        if m.isCurrent then m.title.color = "0x38BDF8FF"
    end if
end sub
