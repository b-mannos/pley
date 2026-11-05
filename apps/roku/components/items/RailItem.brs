sub init()
    m.cell = m.top.findNode("cell")
    m.pill = m.top.findNode("pill")
    m.mark = m.top.findNode("mark")
    m.icon = m.top.findNode("icon")
    m.avatar = m.top.findNode("avatar")
    m.disc = m.top.findNode("disc")
    m.initial = m.top.findNode("initial")
    m.hint = m.top.findNode("hint")
    m.dot = m.top.findNode("dot")
    m.name = m.top.findNode("name")
    m.countBack = m.top.findNode("countBack")
    m.count = m.top.findNode("count")
    m.opening = m.top.findNode("opening")
    m.look = m.top.findNode("look")
    m.lookScale = m.top.findNode("lookScale")
    ' The profile's picture, made round once there is one.
    m.picture = invalid
    m.profile = false
    m.danger = false
    m.current = false
    m.counted = false
    m.titleText = ""
    ' Where the name goes, closed and open: after the icon, or beside the
    ' avatar on its first line.
    m.nameClosed = [48, 0]
    m.nameOpen = [96, 0]
    ' The rail's state drawn (open, and this entry focused); invalid until
    ' the first look, which is drawn at once.
    m.open = invalid
    m.focused = invalid
end sub

sub onContent()
    entry = m.top.itemContent
    if entry = invalid then return
    m.profile = entry.hasField("profile") and entry.profile = true
    m.icon.visible = not m.profile
    m.avatar.visible = m.profile
    m.hint.visible = m.profile
    if m.profile
        showAvatar(entry)
        m.nameClosed = [48, -18]
        m.nameOpen = [112, -18]
    else
        m.icon.uri = "pkg:/images/icons/" + entry.icon + ".png"
        m.nameClosed = [48, 0]
        m.nameOpen = [96, 0]
    end if
    m.titleText = entry.title
    typeSet(m.name, m.titleText)
    m.current = entry.hasField("current") and entry.current = true
    m.mark.visible = m.current
    count = 0
    if entry.hasField("count") then count = entry.count
    m.danger = entry.hasField("danger") and entry.danger = true
    m.counted = count > 0
    m.count.visible = m.counted
    m.countBack.visible = m.counted
    m.dot.visible = m.counted
    m.name.width = 392
    if m.counted
        text = count.ToStr()
        if count > 99 then text = "99+"
        m.count.text = text
        m.count.width = 0
        boxWidth = m.count.boundingRect().width + 20
        if boxWidth < 36 then boxWidth = 36
        m.count.width = boxWidth
        m.count.translation = [488 - boxWidth, 22]
        m.countBack.width = boxWidth
        m.countBack.translation = [488 - boxWidth, 22]
        m.name.width = 392 - boxWidth - 16
    end if
    m.open = invalid
    m.focused = invalid
    onOpen()
end sub

' The avatar: the profile's initial on its colour, its picture over them
' where it has one, round (Roku's MaskGroup, made for this entry alone).
sub showAvatar(entry as object)
    m.disc.blendColor = entry.color
    typeSet(m.initial, entry.initial)
    picture = ""
    if entry.hasField("picture") then picture = entry.picture
    if picture = "" and m.picture = invalid then return
    if m.picture = invalid
        mask = m.avatar.createChild("MaskGroup")
        mask.maskUri = "pkg:/images/round.png"
        mask.maskSize = [64, 64]
        m.picture = mask.createChild("Poster")
        m.picture.width = 64
        m.picture.height = 64
        m.picture.loadWidth = 64
        m.picture.loadHeight = 64
        m.picture.loadDisplayMode = "scaleToZoom"
    end if
    ' The scene's agent carries the token, which a profile's picture needs.
    m.picture.setHttpAgent(m.top.getScene().getHttpAgent())
    m.picture.uri = picture
end sub

' The rail opens with the focus in it (MarkupList's listHasFocus), and
' closes as it goes.
sub onOpen()
    open = m.top.listHasFocus
    if m.open <> invalid and open = m.open then return
    first = m.open = invalid
    m.open = open
    m.mark.visible = m.current and not open
    m.opening.control = "stop"
    amount = 0.0
    if open then amount = 1.0
    if first then setOpen(amount) else slide(m.name.opacity, amount)
    onFocus()
end sub

' From open by from to open by upTo, over 200 ms eased out, as Android
' TV's rail opens and closes.
sub slide(from as float, upTo as float)
    pillTo = 0.0
    if showsPill() then pillTo = upTo
    m.top.findNode("openPill").keyValue = [[0.22 + 0.78 * from, 1.0], [0.22 + 0.78 * upTo, 1.0]]
    m.top.findNode("openPillFade").keyValue = [m.pill.opacity, pillTo]
    m.top.findNode("openName").keyValue = [between(m.nameClosed, m.nameOpen, from), between(m.nameClosed, m.nameOpen, upTo)]
    m.top.findNode("openNameFade").keyValue = [from, upTo]
    m.top.findNode("openCount").keyValue = [from, upTo]
    m.top.findNode("openCountText").keyValue = [from, upTo]
    m.top.findNode("openHint").keyValue = [between([48, 42], [112, 42], from), between([48, 42], [112, 42], upTo)]
    m.top.findNode("openHintFade").keyValue = [from, upTo]
    m.opening.control = "start"
end sub

sub setOpen(amount as float)
    m.pill.scale = [0.22 + 0.78 * amount, 1.0]
    m.pill.opacity = 0.0
    if showsPill() then m.pill.opacity = amount
    m.name.translation = between(m.nameClosed, m.nameOpen, amount)
    m.name.opacity = amount
    m.hint.translation = between([48, 42], [112, 42], amount)
    m.hint.opacity = amount
    m.countBack.opacity = amount
    m.count.opacity = amount
end sub

' The point amount (0 to 1) of the way from a to b.
function between(a as object, b as object, amount as float) as object
    return [a[0] + (b[0] - a[0]) * amount, a[1] + (b[1] - a[1]) * amount]
end function

' Open, the focused entry is on the accent pill, the current one on a
' raised one; the others on none.
function showsPill() as boolean
    return m.top.itemHasFocus or m.current
end function

sub onFocus()
    if m.open = invalid then return
    focused = m.top.itemHasFocus and m.open
    m.dot.visible = m.counted and not m.open
    if focused
        m.pill.blendColor = "0x38BDF8FF"
        m.icon.blendColor = "0x16191EFF"
        m.name.color = "0x16191EFF"
        m.hint.color = "0x16191EFF"
        typeSet(m.name, m.titleText, typeFont(700, 28))
    else
        m.pill.blendColor = "0x2E3440FF"
        if m.current or m.profile
            m.icon.blendColor = "0xEBEBEBFF"
            m.name.color = "0xEBEBEBFF"
        else
            m.icon.blendColor = "0xA1A9B6FF"
            m.name.color = "0xA1A9B6FF"
        end if
        m.hint.color = "0xA1A9B6FF"
        typeSet(m.name, m.titleText, typeFont(500, 28))
    end if
    ' Open, an entry that isn't focused or current has no pill.
    if m.open and m.opening.state <> "running"
        if showsPill() then m.pill.opacity = 1.0 else m.pill.opacity = 0.0
    end if
    ' On the accent pill, an accent number is dark instead.
    if m.danger
        m.countBack.color = "0xEB6B70FF"
        m.count.color = "0x16191EFF"
        m.dot.blendColor = "0xEB6B70FF"
    else if focused
        m.countBack.color = "0x16191EFF"
        m.count.color = "0x7DD3FCFF"
        m.dot.blendColor = "0x38BDF8FF"
    else
        m.countBack.color = "0x38BDF8FF"
        m.count.color = "0x16191EFF"
        m.dot.blendColor = "0x38BDF8FF"
    end if
    if m.focused <> invalid and focused = m.focused then return
    first = m.focused = invalid
    m.focused = focused
    scale = 1.0
    if focused then scale = 1.05
    m.look.control = "stop"
    if first
        m.cell.scale = [scale, scale]
        return
    end if
    m.lookScale.keyValue = [m.cell.scale, [scale, scale]]
    if focused
        m.look.duration = 0.15
        m.look.easeFunction = "outCubic"
    else
        m.look.duration = 0.12
        m.look.easeFunction = "inCubic"
    end if
    m.look.control = "start"
end sub
