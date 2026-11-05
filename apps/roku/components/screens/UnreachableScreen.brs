sub init()
    m.title = m.top.findNode("title")
    m.text = m.top.findNode("text")
    m.buttons = m.top.findNode("buttons")
    m.buttons.observeField("buttonSelected", "onButton")
    m.top.observeField("active", "onActive")
    ' A USB drive plugged in or pulled out: its button comes or goes.
    m.global.drive.observeFieldScoped("drive", "showButtons")
    showButtons()
end sub

sub onActive()
    if not m.top.active then return
    session = m.global.session
    m.title.text = "Can't reach " + session.serverName
    m.text.text = "Pley for Roku can't reach the server at " + urlDisplayAddress(session.server) + ". Check that it's on, and that this TV is on the same network as the server."
    ' A drive plugged in that told nothing gets its button too.
    uiDrive({ op: "look" }, "")
    m.buttons.setFocus(true)
    uiLaunched()
end sub

' Try again and Switch server; and USB drive while one is plugged in, whose
' videos play with no server, as on Android TV.
sub showButtons()
    labels = ["Try again", "Switch server"]
    drive = m.global.drive.drive
    if drive <> invalid and httpString(drive.volume) <> "" then labels.Push("USB drive")
    if FormatJson(labels) = FormatJson(m.buttons.buttons) then return
    focused = m.buttons.buttonFocused
    m.buttons.buttons = labels
    if focused > 0 and focused < labels.Count() then m.buttons.focusButton = focused
end sub

sub onButton(event as object)
    label = m.buttons.buttons[event.getData()]
    if label = "Switch server"
        uiNavigate({ to: "servers", reset: true })
        return
    end if
    if label = "USB drive"
        uiNavigate({ to: "drive", params: { folder: "" } })
        return
    end if
    retry = m.top.params.retry
    if retry = invalid or httpString(retry.to) = "" then retry = { to: "home", params: {} }
    uiNavigate({ to: retry.to, params: retry.params, replace: true })
end sub
