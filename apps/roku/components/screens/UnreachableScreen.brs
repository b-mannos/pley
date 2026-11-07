' The server doesn't answer, in place of the screen that needed it, in
' Android TV's words: "Can't reach your server", why and what to check,
' Retry (that screen again) and Switch server, and USB drive while one is
' plugged in. It goes back to that screen
' by itself once the server answers: the scene asks every 5 s while the
' server is out of reach and tells this screen (reached); shown when the
' scene doesn't think so (another request answered meanwhile), it asks
' once itself.

sub init()
    m.text = m.top.findNode("text")
    m.server = m.top.findNode("server")
    m.buttons = m.top.findNode("buttons")
    m.buttons.observeField("buttonSelected", "onButton")
    ' It went back, or to the servers: nothing more to do.
    m.left = false
    m.top.observeField("active", "onActive")
    m.top.observeField("reached", "onReached")
    ' A USB drive plugged in or pulled out: its button comes or goes.
    m.global.drive.observeFieldScoped("drive", "showButtons")
    showButtons()
end sub

sub onActive()
    if not m.top.active then return
    session = m.global.session
    reason = httpSentence(m.top.params.reason)
    if reason = "" then reason = "The server can't be reached."
    typeSet(m.text, reason + " Check that the server is on and that this device is on its network.")
    typeSet(m.server, session.serverName + " · " + urlDisplayAddress(session.server))
    ' A drive plugged in that told nothing gets its button too.
    uiDrive({ op: "look" }, "")
    m.buttons.setFocus(true)
    uiLaunched()
    if not session.offline then uiRequest({ method: "GET", path: "/api/server", auth: session.signedIn, timeoutMs: 5000 }, "onAnswer")
end sub

' The scene's question answered: the server is back.
sub onReached()
    if m.top.active then back()
end sub

' This screen's own question: any answer but none at all is the server
' back; none, and the scene asks on. Signed out, the scene shows sign-in.
sub onAnswer(event as object)
    error = event.getData().error
    if error <> "network" and error <> "signed_out" and m.top.active then back()
end sub

' Retry and Switch server; and USB drive while one is plugged in, whose
' videos play with no server, as on Android TV.
sub showButtons()
    labels = ["Retry", "Switch server"]
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
        m.left = true
        uiNavigate({ to: "servers", reset: true })
        return
    end if
    if label = "USB drive"
        uiNavigate({ to: "drive", params: { folder: "" } })
        return
    end if
    back()
end sub

' The screen that needed the server, in this one's place.
sub back()
    if m.left then return
    m.left = true
    retry = m.top.params.retry
    if retry = invalid or httpString(retry.to) = "" then retry = { to: "home", params: {} }
    uiNavigate({ to: retry.to, params: retry.params, replace: true })
end sub
