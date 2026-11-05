sub init()
    m.top.focusable = true
    m.row = m.top.findNode("row")
    m.lookIn = m.top.findNode("lookIn")
    m.lookOut = m.top.findNode("lookOut")
    m.inScale = m.top.findNode("inScale")
    m.inWords = m.top.findNode("inWords")
    m.outScale = m.top.findNode("outScale")
    m.outWords = m.top.findNode("outWords")
    m.chips = []
    ' The chip drawn focused (-1 for none).
    m.lit = -1
    m.top.observeField("focusedChild", "onLook")
end sub

' Chips side by side, 16 pixels apart; a chip of width 0 is as wide as its
' words (and its icon and number), with room either side. A chip's number
' (count, "" for none) is a small box after its words.
sub onChips()
    m.lookIn.control = "stop"
    m.lookOut.control = "stop"
    m.row.removeChildrenIndex(m.row.getChildCount(), 0)
    m.chips = []
    m.lit = -1
    h = m.top.height
    places = []
    x = 0
    i = 0
    for each spec in m.top.chips
        chip = makeChip(spec, i)
        width = spec.width
        if chip.round
            width = h
        else if width <= 0
            width = chip.inner + m.top.pad
            if width < m.top.minWidth then width = m.top.minWidth
        end if
        chip.width = width
        place(chip, width, h)
        chip.group.translation = [x, 0]
        chip.selected = spec.DoesExist("selected") and spec.selected = true
        m.chips.Push(chip)
        places.Push([x, width])
        x = x + width + 16
        i = i + 1
    end for
    m.top.places = places
    m.top.places = places
    m.top.rowWidth = x - 16
    if x = 0 then m.top.rowWidth = 0
    if m.top.focusIndex >= m.chips.Count() then m.top.focusIndex = 0
    ' Chips made again under the focus (Now Playing's Pause, then Play)
    ' take its look at once.
    if m.top.hasFocus() and m.top.focusIndex < m.chips.Count()
        m.lit = m.top.focusIndex
        chip = m.chips[m.lit]
        scale = 1.05
        if chip.round then scale = 1.08
        chip.group.scale = [scale, scale]
        chip.under.opacity = 1.0
    end if
    onLook()
end sub

' A chip's nodes in a group of its own, which grows with the focus:
' { group, back, edge, icon, label, under, number, round, inner (how wide
' its words, icon and number are together) }.
function makeChip(spec as object, index as integer) as object
    group = m.row.createChild("Group")
    group.id = "chip" + index.ToStr()
    back = group.createChild("Poster")
    back.uri = "pkg:/images/pill.9.png"
    edge = group.createChild("Poster")
    edge.uri = "pkg:/images/pill-edge.9.png"
    edge.blendColor = "0x6C7584FF"
    edge.visible = m.top.onArt
    chip = { group: group, back: back, edge: edge, icon: invalid, label: invalid, under: invalid, number: invalid, round: false, inner: 0 }
    icon = ""
    if spec.DoesExist("icon") and spec.icon <> invalid then icon = spec.icon
    chip.round = spec.DoesExist("round") and spec.round = true and icon <> ""
    if icon <> ""
        chip.icon = group.createChild("Poster")
        chip.icon.uri = "pkg:/images/icons/" + icon + ".png"
        chip.icon.width = 40
        chip.icon.height = 40
        chip.icon.loadWidth = 40
        chip.icon.loadHeight = 40
    end if
    ' A round button's words go under it, shown while it has the focus.
    under = group.createChild("Label")
    under.id = "under" + index.ToStr()
    under.opacity = 0
    under.horizAlign = "center"
    if chip.round then typeSet(under, spec.label, typeStyleFont("caption"))
    chip.under = under
    if chip.round then return chip
    label = group.createChild("Label")
    typeSet(label, spec.label, typeStyleFont("label"))
    chip.label = label
    words = Int(label.boundingRect().width)
    ' Pixels to spare: a label as wide as its words, rounded down, cuts
    ' them short.
    if spec.DoesExist("count") and spec.count <> "" then chip.number = makeNumber(group, spec)
    if chip.number <> invalid or chip.icon <> invalid then words = words + 2
    chip.words = words
    chip.inner = words
    if chip.icon <> invalid then chip.inner = chip.inner + 56
    if chip.number <> invalid then chip.inner = chip.inner + chip.number.gap + chip.number.width
    return chip
end function

' Puts a chip's nodes in its width × h: the icon and words together in the
' middle, the words cut short in a chip too narrow for all of it.
sub place(chip as object, width as integer, h as integer)
    chip.group.scaleRotateCenter = [width / 2, h / 2]
    chip.back.width = width
    chip.back.height = h
    chip.edge.width = width
    chip.edge.height = h
    if chip.round
        chip.icon.translation = [(width - 40) / 2, (h - 40) / 2]
        chip.under.width = 400
        chip.under.translation = [width / 2 - 200, h + 12]
        return
    end if
    label = chip.label
    label.height = h
    label.vertAlign = "center"
    if chip.icon = invalid and chip.number = invalid
        label.translation = [0, 0]
        label.width = width
        label.horizAlign = "center"
        return
    end if
    extra = chip.inner - chip.words
    words = chip.words
    room = width - m.top.pad - extra
    if words > room then words = room
    if words < 0 then words = 0
    start = Int((width - (words + extra)) / 2)
    if chip.icon <> invalid
        chip.icon.translation = [start, (h - 40) / 2]
        start = start + 56
    end if
    label.translation = [start, 0]
    label.width = words
    number = chip.number
    if number <> invalid
        top = (h - 36) / 2
        number.back.translation = [start + words + number.gap, top]
        number.text.translation = [start + words + number.gap, top]
    end if
end sub

' A chip's number: { back, text, width, gap (from the words), tone }. Where
' it asks to be seen, in a box in the accent (accent) or the danger colour
' (danger), as the menu's; else plain, in bold after the words, as a
' row of eight tabs has room for.
function makeNumber(group as object, chip as object) as object
    back = group.createChild("Poster")
    back.uri = "pkg:/images/pill.9.png"
    text = group.createChild("Label")
    text.font = typeFont(700, 24)
    typeSet(text, chip.count)
    tone = "plain"
    if chip.DoesExist("accent") and chip.accent = true then tone = "accent"
    if chip.DoesExist("danger") and chip.danger = true then tone = "danger"
    width = Int(text.boundingRect().width) + 2
    gap = 10
    if tone <> "plain"
        width = width + 18
        if width < 36 then width = 36
        gap = 12
    end if
    back.width = width
    back.height = 36
    back.visible = tone <> "plain"
    text.width = width
    text.height = 36
    text.horizAlign = "center"
    text.vertAlign = "center"
    return { back: back, text: text, width: width, gap: gap, tone: tone }
end function

sub onLook()
    focused = m.top.hasFocus()
    lit = -1
    if focused and m.top.focusIndex < m.chips.Count() then lit = m.top.focusIndex
    for i = 0 to m.chips.Count() - 1
        lookChip(m.chips[i], i = lit, i = m.top.current or m.chips[i].selected)
    end for
    if lit = m.lit then return
    grow(m.lit, lit)
    m.lit = lit
end sub

' The colours of a chip: the accent pill focused; at rest the current one
' raised, or, over a picture, a faint fill with an edge.
sub lookChip(chip as object, lit as boolean, current as boolean)
    if lit
        chip.back.blendColor = "0x38BDF8FF"
        chip.back.opacity = 1.0
        chip.edge.visible = false
        words = "0x16191EFF"
    else if m.top.onArt
        chip.edge.visible = true
        if chip.round
            chip.back.blendColor = "0x16191EFF"
            chip.back.opacity = 0.8
        else
            chip.back.blendColor = "0xEBEBEBFF"
            chip.back.opacity = 0.12
        end if
        words = "0xEBEBEBFF"
    else if current
        chip.back.blendColor = "0x2E3440FF"
        chip.back.opacity = 1.0
        words = "0xEBEBEBFF"
    else
        chip.back.blendColor = "0x1E222AFF"
        chip.back.opacity = 1.0
        words = "0xA1A9B6FF"
    end if
    if chip.icon <> invalid then chip.icon.blendColor = words
    if chip.label <> invalid then chip.label.color = words
    chip.under.color = "0xEBEBEBFF"
    if chip.number <> invalid then lookNumber(chip.number, lit, words)
end sub

' The focus look moving from chip was to chip now (either -1 for none):
' the one going shrinks back, its words under it fading, over 120 ms; the
' one coming grows, over 150 ms. A round chip grows to 1.08.
sub grow(was as integer, now as integer)
    m.lookIn.control = "stop"
    m.lookOut.control = "stop"
    if was >= 0 and was < m.chips.Count()
        chip = m.chips[was]
        m.outScale.fieldToInterp = chip.group.id + ".scale"
        m.outScale.keyValue = [chip.group.scale, [1.0, 1.0]]
        m.outWords.fieldToInterp = chip.under.id + ".opacity"
        m.outWords.keyValue = [chip.under.opacity, 0.0]
        m.lookOut.control = "start"
    end if
    if now >= 0
        chip = m.chips[now]
        scale = 1.05
        if chip.round then scale = 1.08
        m.inScale.fieldToInterp = chip.group.id + ".scale"
        m.inScale.keyValue = [chip.group.scale, [scale, scale]]
        m.inWords.fieldToInterp = chip.under.id + ".opacity"
        m.inWords.keyValue = [chip.under.opacity, 1.0]
        m.lookIn.control = "start"
    end if
end sub

' On the accent chip, an accent number is dark instead; a plain one is in
' its words' colour (words).
sub lookNumber(number as object, lit as boolean, words as string)
    if number.tone = "danger"
        number.back.blendColor = "0xEB6B70FF"
        number.text.color = "0x16191EFF"
    else if number.tone = "accent" and lit
        number.back.blendColor = "0x16191EFF"
        number.text.color = "0x7DD3FCFF"
    else if number.tone = "accent"
        number.back.blendColor = "0x38BDF8FF"
        number.text.color = "0x16191EFF"
    else
        number.text.color = words
    end if
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if key = "left" and m.top.focusIndex > 0
        m.top.focusIndex = m.top.focusIndex - 1
        return true
    end if
    if key = "right" and m.top.focusIndex < m.chips.Count() - 1
        m.top.focusIndex = m.top.focusIndex + 1
        return true
    end if
    if key = "OK"
        m.top.chosen = m.top.focusIndex
        return true
    end if
    return false
end function
