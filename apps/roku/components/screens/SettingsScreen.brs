sub init()
    m.server = m.top.findNode("server")
    m.profile = m.top.findNode("profile")
    m.device = m.top.findNode("device")
    m.version = m.top.findNode("version")
    m.buttons = m.top.findNode("buttons")
    m.dialog = invalid
    m.buttons.buttons = ["Sort libraries", "Switch server", "Sign out"]
    m.buttons.observeField("buttonSelected", "onButton")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active then return
    session = m.global.session
    m.server.text = session.serverName + " (" + urlDisplayAddress(session.server) + ")"
    m.device.text = session.device
    m.version.text = session.appVersion
    m.buttons.setFocus(true)
    uiGet("/api/auth/me", "onMe")
end sub

' Settings stay usable when the server doesn't answer: switching server
' and signing out need no answer.
sub onMe(event as object)
    answer = event.getData()
    if answer.ok and answer.data <> invalid
        m.profile.text = httpSentence(answer.data.name)
    else if answer.error = "network"
        m.profile.text = "The server can't be reached right now."
    end if
end sub

sub onButton(event as object)
    chosen = m.buttons.buttons[event.getData()]
    if chosen = "Sort libraries"
        uiNavigate({ to: "sortLibraries" })
        return
    end if
    if chosen = "Switch server"
        uiNavigate({ to: "servers", reset: true })
        return
    end if
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = "Sign out of " + m.global.session.serverName + "?"
    dialog.message = ["This TV forgets its sign-in to this server. Signing in again takes a new code."]
    dialog.buttons = ["Sign out", "Cancel"]
    dialog.observeField("buttonSelected", "onSignOutButton")
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

' Signed out, the scene shows sign-in (MainScene.onSignedIn).
sub onSignOutButton(event as object)
    if m.dialog <> invalid then m.dialog.close = true
    m.dialog = invalid
    if event.getData() = 0 then uiRequest({ op: "signOut" }, "")
end sub
