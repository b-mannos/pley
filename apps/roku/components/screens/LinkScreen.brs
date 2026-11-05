' Signing in with a code, as Android TV's sign-in screen: the steps, the
' code in its two fields, the QR code at the right, the status under the
' steps. A code that expires is followed by a new one without a word, up to
' 3 times; then OK gets the next. params.reason says why it shows: the
' server ended the sign-in ("ended", "wider_scope"), Discover's or the
' requests' Sign in ("requests", "approvals"), a profile that needs its
' password ("profile", params.name naming it) or another profile
' ("another"), whose way back is Use another profile, to "Who's watching?".

sub init()
    m.title = m.top.findNode("title")
    m.intro = m.top.findNode("intro")
    m.note = m.top.findNode("note")
    m.steps = m.top.findNode("steps")
    m.goTo = m.top.findNode("goTo")
    m.url = m.top.findNode("url")
    m.code = m.top.findNode("code")
    m.code1 = m.top.findNode("code1")
    m.code2 = m.top.findNode("code2")
    m.codeBack1 = m.top.findNode("codeBack1")
    m.codeBack2 = m.top.findNode("codeBack2")
    m.dash = m.top.findNode("dash")
    m.notes = m.top.findNode("notes")
    m.again = m.top.findNode("again")
    m.status = m.top.findNode("status")
    m.side = m.top.findNode("side")
    m.qrTile = m.top.findNode("qrTile")
    m.qrBack = m.top.findNode("qrBack")
    m.qr = m.top.findNode("qr")
    m.qrCaption = m.top.findNode("qrCaption")
    m.back = m.top.findNode("back")
    m.done = m.top.findNode("done")
    m.leave = m.top.findNode("leave")
    m.countdown = m.top.findNode("countdown")
    m.askAgain = m.top.findNode("askAgain")
    m.started = false
    m.request = invalid
    m.signedIn = false
    ' Codes that came in place of one that expired since a button was
    ' pressed; codes asked for in a row that the server didn't answer.
    m.renewals = 0
    m.failures = 0
    ' What "Trying again in" counts down from, in seconds.
    m.retryLeft = 0
    m.qr.observeField("loadStatus", "onQrStatus")
    m.again.observeField("buttonSelected", "onAgain")
    m.back.observeField("buttonSelected", "onBack")
    m.leave.observeField("fire", "onLeave")
    m.countdown.observeField("fire", "onCountdown")
    m.askAgain.observeField("fire", "onAskAgain")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        stopAsking()
        m.countdown.control = "stop"
        m.askAgain.control = "stop"
        return
    end if
    if not m.started
        m.started = true
        showReason()
        m.back.buttons = [backLabel()]
        setStatus("Getting a code…", false)
        askCode()
        ' Whether the server requires two-step sign-in, asked after the
        ' code, which never waits for it. Anyone may ask, so it goes
        ' without the token. Approving a profile is about the household,
        ' not the server's sign-in: it isn't said there.
        if not forProfile() then uiRequest({ method: "GET", path: "/api/server", auth: false }, "onServerInfo")
    end if
    focusButtons()
end sub

' Signing in to pick a profile, from "Who's watching?".
function forProfile() as boolean
    reason = httpString(m.top.params.reason)
    return reason = "profile" or reason = "another"
end function

function backLabel() as string
    if forProfile() then return "Use another profile"
    return "Change server"
end function

' Why the screen shows, under the title: a reason to sign in again, in the
' danger colour; for a profile, the title and what to do on the phone.
sub showReason()
    reason = httpString(m.top.params.reason)
    name = httpSentence(m.top.params.name)
    intro = ""
    if reason = "ended"
        intro = "The server ended this TV's sign-in: sign in again."
    else if reason = "wider_scope"
        intro = "The server allowed more than Pley for Roku asks for, so it was signed out."
    else if reason = "requests"
        ' Discover's Sign in: this sign-in goes on until the new one.
        intro = "Sign in again to let this TV ask for movies and shows."
    else if reason = "approvals"
        intro = "Sign in again to approve and decline requests here."
    end if
    if intro <> ""
        m.intro.text = intro
        m.intro.visible = true
    end if
    if reason = "profile" and name <> ""
        m.title.text = "Approve on your phone"
        showNote(name + "'s profile needs its password: approve this code on a phone signed in to Pley, and pick " + name + " there.")
    else if forProfile()
        m.title.text = "Sign in as another profile"
        showNote("Approve this code on a phone signed in to Pley, and pick the profile there.")
    end if
end sub

' What the server or this sign-in asks, under the title: a line lower when
' the reason to sign in again shows.
sub showNote(text as string)
    m.note.text = text
    top = 280
    if m.intro.visible then top = 316
    m.note.translation = [96, top]
    m.note.visible = true
end sub

' A server that requires two-step sign-in says so under the title. Nothing
' when the server doesn't say or didn't answer: the sign-in goes on all
' the same.
sub onServerInfo(event as object)
    note = authTwoStepNote(event.getData())
    if note = "" or m.signedIn then return
    showNote(note)
end sub

' Asks for a code, and goes on asking whether it was approved. The code
' shown stays, dimmed, until the new one comes.
sub askCode()
    stopAsking()
    m.askAgain.control = "stop"
    m.countdown.control = "stop"
    hideAgain()
    m.code.opacity = 0.4
    m.request = uiRequest({ op: "signIn" }, "onCode")
    m.request.observeField("update", "onUpdate")
end sub

sub stopAsking()
    if m.request = invalid then return
    m.request.unobserveField("result")
    m.request.unobserveField("update")
    m.request = invalid
    if not m.signedIn then uiRequest({ op: "signInStop" }, "")
end sub

' The code, or why there is none. When the server doesn't answer, a code
' is asked for again after a wait that grows (2, 4, 8, 16, then 30 s), as
' the screen says, as on Android TV.
sub onCode(event as object)
    answer = event.getData()
    if not answer.ok
        m.request = invalid
        if answer.error = "network"
            m.failures = m.failures + 1
            waitMs = authPollWait(0, m.failures)
            retrying(Int(waitMs / 1000))
            m.askAgain.duration = waitMs / 1000
            m.askAgain.control = "start"
            return
        end if
        if uiFailed(answer) then return
        setStatus(answer.message, true)
        showAgain("Retry")
        return
    end if
    m.failures = 0
    showCode(answer.data.userCode)
    m.url.text = authShownAddress(answer.data.verificationUri)
    fitAddress()
    showQr(answer.data.qr)
end sub

sub onAskAgain()
    setStatus("Getting a code…", false)
    askCode()
end sub

' The code in its two fields ("BCDF – GHJK"), or in one when it is in
' another form.
sub showCode(code as string)
    halves = authCodeHalves(code)
    m.code1.text = halves[0]
    split = halves.Count() = 2
    m.dash.visible = split
    m.codeBack2.visible = split
    m.code2.visible = split
    if split
        m.code2.text = halves[1]
        m.code1.width = 320
        m.codeBack1.width = 320
    else
        m.code1.width = 744
        m.codeBack1.width = 744
    end if
    m.code.opacity = 1.0
end sub

' The address after "or go to", in the largest size that fits the rest of
' the line; one too long even in the smallest is cut.
sub fitAddress()
    left = 180 + Int(m.goTo.boundingRect().width) + 16
    room = 1136 - left
    m.url.translation = [left, m.url.translation[1]]
    m.url.width = 0
    for each font in ["font:LargeBoldSystemFont", "font:MediumBoldSystemFont", "font:SmallBoldSystemFont", "font:SmallestBoldSystemFont"]
        m.url.font = font
        if m.url.boundingRect().width <= room then exit for
    end for
    m.url.width = room
end sub

' The server's QR code of the address with the code in it, in its tile: a
' placeholder until the picture comes. Anyone may fetch it, so it goes
' without the token.
sub showQr(address as string)
    m.qrTile.opacity = 1.0
    m.qrBack.color = uiColors().surface2
    if address = ""
        hideQr()
        return
    end if
    m.qrTile.visible = true
    m.qrCaption.visible = true
    m.qr.setHttpAgent(uiHttpAgent(false))
    m.qr.uri = address
end sub

' A server older than the QR codes answers 404: the code is enough.
sub hideQr()
    m.qrTile.visible = false
    m.qrCaption.visible = false
    m.qr.uri = ""
end sub

' Drawn at a whole multiple of its size (the server draws 8 pixels a
' module, its quiet zone around), so its modules stay sharp, centred on
' the white tile.
sub onQrStatus()
    status = m.qr.loadStatus
    if status = "failed"
        hideQr()
        return
    end if
    if status <> "ready" or m.qr.uri = "" or m.qr.bitmapWidth <= 0 then return
    side = Int(m.qr.bitmapWidth)
    size = side * Int(qrSpace() / side)
    if size <= 0 then size = qrSpace()
    m.qr.width = size
    m.qr.height = size
    at = Int((qrSpace() - size) / 2)
    m.qr.translation = [at, at]
    ' The lightest the broadcast-safe colours go.
    m.qrBack.color = uiColors().fg
end sub

' The side of the QR code's tile.
function qrSpace() as integer
    return 536
end function

sub onUpdate(event as object)
    update = event.getData()
    status = update.status
    m.countdown.control = "stop"
    if status = "waiting"
        setStatus("Waiting for approval…", false)
    else if status = "retrying"
        retrying(Int(httpNumber(update.seconds, 0)))
    else if status = "denied"
        cantUse("Request declined", true, false)
    else if status = "expired"
        ' A new one at once: the TV may be left on this screen a while.
        ' After half an hour of them, OK gets the next, so a Roku left on
        ' overnight doesn't ask the server for codes all night.
        if m.renewals < renewalsMax()
            m.renewals = m.renewals + 1
            askCode()
        else
            cantUse("Code expired: press OK for a new one", false, true)
        end if
    else if status = "signed_in"
        signedIn()
    else
        cantUse(httpString(update.message), true, false)
    end if
end sub

' Codes given in a row with no button pressed: with the server's 10
' minutes a code, half an hour's worth, as on Android TV.
function renewalsMax() as integer
    return 3
end function

' The server didn't answer: when the next try goes, counted down each
' second.
sub retrying(seconds as integer)
    m.retryLeft = seconds
    showRetrying()
    m.countdown.control = "start"
end sub

sub onCountdown()
    if m.retryLeft > 0 then m.retryLeft = m.retryLeft - 1
    showRetrying()
    if m.retryLeft = 0 then m.countdown.control = "stop"
end sub

sub showRetrying()
    setStatus("Can't reach the server. Trying again in " + m.retryLeft.ToStr() + " s", true)
end sub

' The code can't be used any more: a new one is a button away. An expired
' code and its QR code stay, dimmed.
sub cantUse(text as string, problem as boolean, dimmed as boolean)
    m.request = invalid
    if dimmed
        m.code.opacity = 0.4
        m.qrTile.opacity = 0.4
    end if
    setStatus(text, problem)
    showAgain("Get a new code")
end sub

' Get a new code (or Retry) in place of the notes, with the focus.
sub showAgain(label as string)
    m.again.buttons = [label]
    m.notes.visible = false
    m.again.visible = true
    if m.top.isInFocusChain() then m.again.setFocus(true)
end sub

sub hideAgain()
    hadFocus = m.again.isInFocusChain()
    m.again.visible = false
    m.notes.visible = true
    if hadFocus then m.back.setFocus(true)
end sub

sub focusButtons()
    if m.signedIn
        m.top.setFocus(true)
    else if m.again.visible
        m.again.setFocus(true)
    else
        m.back.setFocus(true)
    end if
end sub

sub onAgain()
    m.renewals = 0
    m.failures = 0
    setStatus("Getting a code…", false)
    askCode()
end sub

' Change server; or, picking a profile, back to "Who's watching?".
sub onBack()
    if forProfile()
        uiNavigate({ back: true })
    else
        uiNavigate({ to: "servers", reset: true })
    end if
end sub

' Only "Signed in as …" shows, for a moment, then Home.
sub signedIn()
    ' The profile was chosen in the browser: no "Who's watching?" now.
    m.top.getScene().profileChosen = true
    m.signedIn = true
    m.request = invalid
    for each node in [m.title, m.intro, m.note, m.steps, m.side, m.status]
        node.visible = false
    end for
    m.done.visible = true
    m.top.setFocus(true)
    uiGet("/api/auth/me", "onMe")
end sub

sub onMe(event as object)
    answer = event.getData()
    if answer.ok and answer.data <> invalid
        name = httpSentence(answer.data.name)
        if name <> "" then m.done.text = "Signed in as " + name
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

' Left and Right go between Get a new code and the way back.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press or m.signedIn then return false
    if key = "right" and m.again.isInFocusChain()
        m.back.setFocus(true)
        return true
    end if
    if key = "left" and m.back.isInFocusChain() and m.again.visible
        m.again.setFocus(true)
        return true
    end if
    return false
end function
