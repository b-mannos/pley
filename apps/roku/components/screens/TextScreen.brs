' A page from Settings (its route says which): Diagnostics, as Android
' TV's, a column of labels and one of values under "This TV" (the app and
' this Roku) and "What it plays" (what the Roku tells the server it plays,
' as it asks for each title: lib/Caps.brs); or the open-source licences,
' Pley's own and then the notices of what the package bundles
' (licenses/), as text that Up and Down scroll.

sub init()
    m.title = m.top.findNode("title")
    m.text = m.top.findNode("text")
    m.facts = m.top.findNode("facts")
    ' The fact focused last, which says which way the focus was going when
    ' it lands on a heading.
    m.factRow = 0
    m.facts.observeField("itemFocused", "onFactFocused")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active then return
    if m.top.route = "licences"
        if m.text.text = ""
            typeSet(m.title, t("settings.licences"))
            typeSet(m.text, licences())
        end if
        m.text.visible = true
        m.text.setFocus(true)
        return
    end if
    if m.facts.content = invalid
        typeSet(m.title, t("settings.diagnostics"))
        m.facts.content = nodesContent(diagnostics())
    end if
    m.facts.visible = true
    m.facts.setFocus(true)
    if isHeading(m.facts.itemFocused) then m.facts.jumpToItem = m.facts.itemFocused + 1
end sub

' The facts: [{ title, value, heading }].
function diagnostics() as object
    session = m.global.session
    device = CreateObject("roDeviceInfo")
    rows = [heading(t("settings.diagnosticsPage.thisTv"))]
    rows.Push(fact(t("settings.diagnosticsPage.app"), "Pley " + session.appVersion))
    rows.Push(fact(t("settings.diagnosticsPage.model"), httpSentence(device.GetModelDisplayName())))
    rows.Push(fact(t("settings.diagnosticsPage.os"), httpOsVersion(device.GetOSVersion())))
    if session.deviceName <> "" then rows.Push(fact(t("settings.diagnosticsPage.name"), session.deviceName))
    rows.Push(heading(t("settings.diagnosticsPage.plays")))
    for each line in capsLines(capsBuild(device))
        rows.Push(factOf(line))
    end for
    return rows
end function

function heading(text as string) as object
    return { title: text, value: "", heading: true }
end function

function fact(label as string, value as string) as object
    return { title: label, value: value, heading: false }
end function

' A line of what the Roku plays ("HDR: HDR10, Dolby Vision") as its label
' and its value; in French, the no-break space before the colon goes with
' it ("HDR : HDR10").
function factOf(line as string) as object
    at = Instr(1, line, ": ")
    if at = 0 then return fact("", line)
    label = Left(line, at - 1)
    if Right(label, 1) = Chr(160) then label = Left(label, Len(label) - 1)
    return fact(label, Mid(line, at + 2))
end function

function isHeading(row as integer) as boolean
    if m.facts.content = invalid then return false
    node = m.facts.content.getChild(row)
    return node <> invalid and node.heading = true
end function

' A heading passes the focus on: to the fact beyond it, the way the focus
' was going, or to the first fact from the top.
sub onFactFocused(event as object)
    row = event.getData()
    if row < 0 or m.facts.content = invalid then return
    if not isHeading(row)
        m.factRow = row
        return
    end if
    beyond = row + 1
    if row < m.factRow and row > 0 then beyond = row - 1
    if beyond >= m.facts.content.getChildCount() then beyond = row - 1
    if beyond >= 0 then m.facts.jumpToItem = beyond
end sub

' Pley's licence, then those of what the package bundles: BrighterScript's
' helpers, Lucide's icons and the Roboto font.
function licences() as string
    parts = [t("settings.licence"), ReadAsciiFile("pkg:/licenses/bslib.txt")]
    parts.Push(t("settings.licencesPage.lucide") + Chr(10) + Chr(10) + ReadAsciiFile("pkg:/licenses/lucide.txt"))
    parts.Push(t("settings.licencesPage.roboto") + Chr(10) + Chr(10) + ReadAsciiFile("pkg:/licenses/roboto.txt"))
    return parts.Join(Chr(10) + Chr(10))
end function
