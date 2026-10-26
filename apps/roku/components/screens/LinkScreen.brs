sub init()
    m.title = m.top.findNode("title")
    m.server = m.top.findNode("server")
    m.intro = m.top.findNode("intro")
    m.steps = m.top.findNode("steps")
    m.url = m.top.findNode("url")
    m.code = m.top.findNode("code")
    m.qrBox = m.top.findNode("qrBox")
    m.qr = m.top.findNode("qr")
    m.status = m.top.findNode("status")
    m.actions = m.top.findNode("actions")
    m.leave = m.top.findNode("leave")
    m.started = false
    m.request = invalid
    m.done = false
    m.qr.observeField("loadStatus", "onQrStatus")
    m.actions.observeField("buttonSelected", "onAction")
    m.leave.observeField("fire", "onLeave")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        stopAsking()
        return
    end if
    if not m.started
        m.started = true
        session = m.global.session
        m.server.text = "on " + session.serverName + " (" + urlDisplayAddress(session.server) + ")"
        reason = httpString(m.top.params.reason)
        if reason = "ended"
            m.intro.text = "The server ended this TV's sign-in: sign in again."
            m.intro.visible = true
        else if reason = "wider_scope"
            m.intro.text = "The server allowed more than pley for Roku asks for, so it was signed out."
            m.intro.visible = true
        end if
        ask()
    end if
    m.actions.setFocus(true)
end sub

' Asks for a code, and goes on asking whether it was approved.
sub ask()
    stopAsking()
    m.code.text = "…"
    m.code.opacity = 1.0
    m.url.text = ""
    hideQr()
    setStatus("Getting a code…", false)
    setButtons(["Change server"])
    m.request = uiRequest({ op: "signIn" }, "onCode")
    m.request.observeField("update", "onUpdate")
end sub

sub stopAsking()
    if m.request = invalid then return
    m.request.unobserveField("result")
    m.request.unobserveField("update")
    m.request = invalid
    if not m.done then uiRequest({ op: "signInStop" }, "")
end sub

sub onCode(event as object)
    answer = event.getData()
    if not answer.ok
        m.request = invalid
        if uiFailed(answer) then return
        setStatus(answer.message, true)
        setButtons(["Try again", "Change server"])
        return
    end if
    m.code.text = answer.data.userCode
    m.url.text = answer.data.verificationUri
    showQr(answer.data.qr)
end sub

' The server's QR code of the address with the code in it, shown once it
' has loaded: a server older than it answers 404, and the code is enough.
' Anyone may fetch it, so it goes without the token.
sub showQr(address as string)
    m.qrBox.visible = false
    m.qr.setHttpAgent(uiHttpAgent(false))
    m.qr.uri = address
end sub

sub hideQr()
    m.qrBox.visible = false
    m.qr.uri = ""
end sub

sub onQrStatus()
    m.qrBox.visible = m.qr.loadStatus = "ready" and m.qr.uri <> ""
end sub

sub onUpdate(event as object)
    update = event.getData()
    status = update.status
    if status = "waiting"
        setStatus("Waiting for approval…", false)
    else if status = "retrying"
        setStatus("Can't reach the server. Trying again in " + urlValue(update.seconds) + " s.", true)
    else if status = "denied"
        expired("Request declined.", true)
    else if status = "expired"
        expired("Code expired: press OK for a new one.", false)
    else if status = "signed_in"
        signedIn()
    else
        expired(httpString(update.message), true)
    end if
end sub

' The code can't be used any more: a new one is a button away.
sub expired(text as string, problem as boolean)
    m.request = invalid
    m.code.opacity = 0.4
    hideQr()
    setStatus(text, problem)
    setButtons(["Get a new code", "Change server"])
end sub

sub signedIn()
    m.done = true
    m.request = invalid
    m.steps.visible = false
    m.intro.visible = false
    m.title.text = "Signed in"
    setStatus("", false)
    setButtons([])
    m.top.setFocus(true)
    uiGet("/api/auth/me", "onMe")
end sub

sub onMe(event as object)
    answer = event.getData()
    if answer.ok and answer.data <> invalid
        name = httpSentence(answer.data.name)
        if name <> "" then m.title.text = "Signed in as " + name
    end if
    m.leave.control = "start"
end sub

sub onLeave()
    uiNavigate({ to: "home", reset: true })
end sub

sub setStatus(text as string, problem as boolean)
    m.status.text = text
    if problem
        m.status.color = uiColors().danger
    else
        m.status.color = uiColors().muted
    end if
end sub

sub setButtons(labels as object)
    m.actions.buttons = labels
    m.actions.visible = labels.Count() > 0
    if labels.Count() > 0 and m.top.active
        m.actions.focusButton = 0
        m.actions.setFocus(true)
    end if
end sub

sub onAction(event as object)
    label = m.actions.buttons[event.getData()]
    if label = "Change server"
        uiNavigate({ to: "servers", reset: true })
    else
        ask()
    end if
end sub
