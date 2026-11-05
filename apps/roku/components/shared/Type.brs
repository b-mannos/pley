' Roboto on the screens: Font nodes in Android TV's styles (lib/Type.brs),
' and text from the server in Roboto where Roboto draws all of it, else
' in Roku's own font. Needs lib/Type.brs.

' A Font of Roboto in weight (400, 500 or 700) at size, one per component.
function typeFont(weight as integer, size as integer) as object
    if m.typeFonts = invalid then m.typeFonts = {}
    key = weight.ToStr() + "_" + size.ToStr()
    font = m.typeFonts[key]
    if font = invalid
        font = CreateObject("roSGNode", "Font")
        font.uri = "pkg:/fonts/roboto-" + weight.ToStr() + ".ttf"
        font.size = size
        m.typeFonts[key] = font
    end if
    return font
end function

' The Font of one of Android TV's styles: "label", "caption"…
function typeStyleFont(name as string) as object
    style = typeStyle(name)
    return typeFont(style.weight, style.size)
end function

' Sets a label's text from the server (a title, a name, a summary): in its
' Roboto font where Roboto draws every character, a rating's star (★,
' which Roboto lacks) drawn as an icon in its place on a left-aligned line
' that doesn't wrap; else in Roku's font nearest its size. font: the
' Roboto Font it takes, or invalid for the one its component gave it.
sub typeSet(label as object, text as dynamic, font = invalid as dynamic)
    if GetInterface(text, "ifString") = invalid then text = ""
    ' A component of Pley's own with a text (PageMessage, StagePill) sets
    ' its labels itself.
    if not label.hasField("font")
        label.text = text
        return
    end if
    if not label.hasField("typeRoboto")
        label.addField("typeRoboto", "node", false)
        label.addField("typeFallen", "boolean", false)
        label.typeRoboto = label.font
    end if
    if font <> invalid then label.typeRoboto = font
    roboto = label.typeRoboto
    starAt = 0
    if label.subtype() = "Label" and not label.wrap and label.horizAlign = "left" then starAt = Instr(1, text, "★")
    shown = text
    if starAt > 0 then shown = Left(text, starAt - 1) + Chr(&h2002) + Chr(&h2002) + Mid(text, starAt + 1)
    fallen = not typeCovers(shown)
    if fallen
        shown = text
        starAt = 0
        label.font = typeSystemFont(roboto.size, typeWeight(roboto))
    else if label.typeFallen or font <> invalid
        label.font = roboto
    end if
    label.typeFallen = fallen
    label.text = shown
    typeStar(label, roboto, starAt, shown)
end sub

' A Roboto Font's weight, from its file's name.
function typeWeight(font as object) as integer
    if Instr(1, font.uri, "700") > 0 then return 700
    if Instr(1, font.uri, "500") > 0 then return 500
    return 400
end function

' The star icon a label draws in place of a rating's ★ (at starAt in
' shown, where two spaces keep its room), or none (starAt 0). It is the
' label's child, in its colour.
sub typeStar(label as object, font as object, starAt as integer, shown as string)
    star = invalid
    if label.hasField("typeStar") then star = label.typeStar
    if starAt = 0
        if star <> invalid then star.visible = false
        return
    end if
    if star = invalid
        label.addField("typeStar", "node", false)
        star = label.createChild("Poster")
        star.uri = "pkg:/images/icons/star-fill.png"
        label.typeStar = star
    end if
    size = Int(font.size * 0.72)
    line = typeLineHeight(font)
    top = 0
    if label.height > line and label.vertAlign = "center" then top = (label.height - line) / 2
    if label.height > line and label.vertAlign = "bottom" then top = label.height - line
    star.width = size
    star.height = size
    star.loadWidth = size
    star.loadHeight = size
    star.translation = [Int(typeWidth(font, Left(shown, starAt - 1)) + font.size * 0.14), Int(top + (line - size) / 2)]
    star.blendColor = label.color
    star.visible = true
end sub

' How wide text is in font: measured with a bar after it, so that the
' spaces it ends with count.
function typeWidth(font as object, text as string) as float
    if text = "" then return 0
    measure = typeMeasure(font)
    measure.text = text + "|"
    width = measure.boundingRect().width
    measure.text = "|"
    return width - measure.boundingRect().width
end function

' How tall a line of font is.
function typeLineHeight(font as object) as float
    measure = typeMeasure(font)
    measure.text = "Ag"
    return measure.boundingRect().height
end function

function typeMeasure(font as object) as object
    if m.typeMeasureLabel = invalid then m.typeMeasureLabel = CreateObject("roSGNode", "Label")
    m.typeMeasureLabel.font = font
    return m.typeMeasureLabel
end function

' A list's fonts (font and focusedFont, the focused entry bold) for the
' texts of its entries: Roboto at size where it draws them all, else
' Roku's font nearest it.
sub typeListFonts(list as object, texts as object, size as integer)
    covered = true
    for each text in texts
        if not typeCovers(text)
            covered = false
            exit for
        end if
    end for
    if covered
        list.font = typeFont(400, size)
        list.focusedFont = typeFont(700, size)
    else
        list.font = typeSystemFont(size, 400)
        list.focusedFont = typeSystemFont(size, 700)
    end if
end sub

' A RowList's row titles in its Roboto (its rowLabelFont, from its XML)
' where Roboto draws all of them, else in Roku's font nearest it. After
' its content is set. The font changes only when that does: a RowList
' given a font lays out its rows again.
sub typeRowLabels(rows as object)
    if not rows.hasField("typeRoboto")
        rows.addField("typeRoboto", "node", false)
        rows.addField("typeFallen", "boolean", false)
        rows.typeRoboto = rows.rowLabelFont
    end if
    roboto = rows.typeRoboto
    covered = true
    content = rows.content
    if content <> invalid
        for i = 0 to content.getChildCount() - 1
            if not typeCovers(content.getChild(i).title)
                covered = false
                exit for
            end if
        end for
    end if
    if covered = not rows.typeFallen then return
    rows.typeFallen = not covered
    if covered
        rows.rowLabelFont = roboto
    else
        rows.rowLabelFont = typeSystemFont(roboto.size, typeWeight(roboto))
    end if
end sub
