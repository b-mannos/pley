' Signing in with a code, as Android TV's sign-in screen: the steps, the
' code in its two fields, the QR code at the right, the status under the
' steps. A code that expires is followed by a new one without a word, up to
' 3 times; then OK gets the next. params.reason says why it shows: the
' server ended the sign-in ("ended", "wider_scope"), Discover's or the
' requests' Sign in ("requests", "approvals"), a profile that needs its
' password ("profile", params.name naming it) or another profile
' ("another"), whose way back is Use another profile, to "Who's watching?".
' At the top right the globe picks the language; the screen draws its
' words again in a new one, the code and the focus staying.

sub init()
    m.title = m.top.findNode("title")
    m.intro = m.top.findNode("intro")
    m.note = m.top.findNode("note")
    m.steps = m.top.findNode("steps")
    m.scanStep = m.top.findNode("scanStep")
    m.goTo = m.top.findNode("goTo")
    m.url = m.top.findNode("url")
    m.enterCode = m.top.findNode("enterCode")
    m.code = m.top.findNode("code")
    m.code1 = m.top.findNode("code1")
    m.code2 = m.top.findNode("code2")
    m.codeBack1 = m.top.findNode("codeBack1")
    m.codeBack2 = m.top.findNode("codeBack2")
    m.dash = m.top.findNode("dash")
    m.notes = m.top.findNode("notes")
    m.sameNetwork = m.top.findNode("sameNetwork")
    m.otherDevice = m.top.findNode("otherDevice")
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
    m.globe = m.top.findNode("globe")
    m.languages = m.top.findNode("languages")
    m.started = false
    m.request = invalid
    m.signedIn = false
    ' Codes that came in place of one that expired since a button was
    ' pressed; codes asked for in a row that the server didn't answer.
    m.renewals = 0
    m.failures = 0
    ' What "Trying again in" counts down from, in seconds.
    m.retryLeft = 0
    ' What the screen says, drawn again in a new language: why it shows
    ' (params.reason, and the profile it names), what the server or this
    ' sign-in asks under the title (a key and its values), the status line
    ' ({ key, values, text, problem }), what Get a new code (or Retry) says
    ' and, signed in, the profile's name.
    m.reason = ""
    m.profileName = ""
    m.noteWords = invalid
    m.statusWords = { key: "", values: invalid, text: "", problem: false }
    m.againKey = ""
    m.signedInAs = ""
    ' Where the focus was before Up took it to the globe.
    m.beforeGlobe = invalid
    m.qr.observeField("loadStatus", "onQrStatus")
    m.again.observeField("buttonSelected", "onAgain")
    m.back.observeField("buttonSelected", "onBack")
    m.leave.observeField("fire", "onLeave")
    m.countdown.observeField("fire", "onCountdown")
    m.askAgain.observeField("fire", "onAskAgain")
    m.languages.observeField("picked", "onLanguagePicked")
    m.languages.observeField("closed", "onLanguagesClosed")
    m.top.observeField("active", "onActive")
    m.top.observeField("language", "relabel")
    relabel()
end sub

' The screen's words in the language shown; the code, the address and the
' focus stay.
sub relabel()
    typeSet(m.title, t(titleKey()))
    reasons = { ended: "signin.link.ended", wider_scope: "signin.link.widerScope", requests: "signin.link.requests", approvals: "signin.link.approvals" }
    if reasons.DoesExist(m.reason) then typeSet(m.intro, t(reasons[m.reason]))
    showNote()
    typeSet(m.scanStep, t("signin.link.scanStep"))
    typeSet(m.goTo, t("signin.link.goTo"))
    typeSet(m.enterCode, t("signin.link.enterCode"))
    typeSet(m.sameNetwork, t("signin.link.sameNetwork"))
    typeSet(m.otherDevice, t("signin.link.otherDevice"))
    typeSet(m.qrCaption, t("signin.link.scan"))
    ' The address follows "or go to", whose width the language changes.
    fitAddress()
    m.back.buttons = [t(backKey())]
    if m.againKey <> "" then m.again.buttons = [t(m.againKey)]
    if m.signedInAs <> ""
        typeSet(m.done, t("signin.link.signedInAs", { name: m.signedInAs }))
    else
        typeSet(m.done, t("signin.link.signedIn"))
    end if
    m.globe.language = m.global.language
    showStatus()
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
        m.reason = httpString(m.top.params.reason)
        m.profileName = httpSentence(m.top.params.name)
        showReason()
        relabel()
        setStatus("signin.link.gettingCode", invalid, false)
        askCode()
        ' Whether the server requires two-step sign-in, asked after the
        ' code, which never waits for it. Anyone may ask, so it goes
        ' without the token. Approving a profile is about the household,
        ' not the server's sign-in: it isn't said there.
        if not forProfile() then uiRequest({ method: "GET", path: "/api/server", auth: false }, "onServerInfo")
    end if
    if m.languages.visible
        m.languages.open = true
    else
        focusButtons()
    end if
end sub

' Signing in to pick a profile, from "Who's watching?".
function forProfile() as boolean
    return m.reason = "profile" or m.reason = "another"
end function

function backKey() as string
    if forProfile() then return "signin.profiles.useAnother"
    return "signin.link.changeServer"
end function

' The title: signing in, or, for a profile, approving on the phone.
function titleKey() as string
    if m.reason = "profile" and m.profileName <> "" then return "signin.link.approveTitle"
    if forProfile() then return "signin.profiles.another"
    return "signin.link.title"
end function

' Why the screen shows, under the title: a reason to sign in again, in the
' danger colour; for a profile, what to do on the phone.
sub showReason()
    reasons = ["ended", "wider_scope", "requests", "approvals"]
    for each reason in reasons
        ' Discover's Sign in ("requests"): this sign-in goes on until the
        ' new one.
        if reason = m.reason then m.intro.visible = true
    end for
    if m.reason = "profile" and m.profileName <> ""
        m.noteWords = { key: "signin.link.approveBody", values: { name: m.profileName } }
    else if forProfile()
        m.noteWords = { key: "signin.link.anotherBody", values: invalid }
    end if
end sub

' What the server or this sign-in asks, under the title: a line lower when
' the reason to sign in again shows. Gone once signed in.
sub showNote()
    if m.noteWords = invalid or m.signedIn then return
    typeSet(m.note, t(m.noteWords.key, m.noteWords.values))
    top = 280
    if m.intro.visible then top = 316
    m.note.translation = [96, top]
    m.note.visible = true
end sub

' A server that requires two-step sign-in says so under the title. Nothing
' when the server doesn't say or didn't answer: the sign-in goes on all
' the same.
sub onServerInfo(event as object)
    if authTwoStepNote(event.getData()) = "" or m.signedIn then return
    m.noteWords = { key: "signin.link.twoStep", values: invalid }
    showNote()
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
        setStatusText(answer.message, true)
        showAgain("common.actions.retry")
        return
    end if
    m.failures = 0
    showCode(answer.data.userCode)
    typeSet(m.url, authShownAddress(answer.data.verificationUri))
    fitAddress()
    showQr(answer.data.qr)
end sub

sub onAskAgain()
    setStatus("signin.link.gettingCode", invalid, false)
    askCode()
end sub

' The code in its two fields ("BCDF – GHJK"), or in one when it is in
' another form.
sub showCode(code as string)
    halves = authCodeHalves(code)
    typeSet(m.code1, halves[0])
    split = halves.Count() = 2
    m.dash.visible = split
    m.codeBack2.visible = split
    m.code2.visible = split
    if split
        typeSet(m.code2, halves[1])
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
    for each size in [44, 36, 32, 26]
        m.url.font = typeFont(700, size)
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
        setStatus("signin.link.waiting", invalid, false)
    else if status = "retrying"
        retrying(Int(httpNumber(update.seconds, 0)))
    else if status = "denied"
        cantUse("signin.link.declined", "", true, false)
    else if status = "expired"
        ' A new one at once: the TV may be left on this screen a while.
        ' After half an hour of them, OK gets the next, so a Roku left on
        ' overnight doesn't ask the server for codes all night.
        if m.renewals < renewalsMax()
            m.renewals = m.renewals + 1
            askCode()
        else
            cantUse("signin.link.expired", "", false, true)
        end if
    else if status = "signed_in"
        signedIn()
    else
        cantUse("", httpString(update.message), true, false)
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
    setStatus("signin.link.retrying", { seconds: m.retryLeft }, true)
end sub

' The code can't be used any more: a new one is a button away. Why: key's
' words, else text, as it came. An expired code and its QR code stay,
' dimmed.
sub cantUse(key as string, text as string, problem as boolean, dimmed as boolean)
    m.request = invalid
    if dimmed
        m.code.opacity = 0.4
        m.qrTile.opacity = 0.4
    end if
    if key <> ""
        setStatus(key, invalid, problem)
    else
        setStatusText(text, problem)
    end if
    showAgain("signin.link.newCode")
end sub

' Get a new code (or Retry, key's words) in place of the notes, with the
' focus unless the globe or the languages have it.
sub showAgain(key as string)
    m.againKey = key
    m.again.buttons = [t(key)]
    m.notes.visible = false
    m.again.visible = true
    if m.top.isInFocusChain() and not m.globe.isInFocusChain() and not m.languages.visible then m.again.setFocus(true)
end sub

sub hideAgain()
    hadFocus = m.again.isInFocusChain()
    m.againKey = ""
    m.again.visible = false
    m.notes.visible = true
    if hadFocus then m.back.setFocus(true)
    if m.beforeGlobe <> invalid and m.beforeGlobe.isSameNode(m.again) then m.beforeGlobe = m.back
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
    setStatus("signin.link.gettingCode", invalid, false)
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
    for each node in [m.title, m.intro, m.note, m.steps, m.side, m.status, m.globe, m.languages]
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
        if name <> ""
            m.signedInAs = name
            typeSet(m.done, t("signin.link.signedInAs", { name: name }))
        end if
    end if
    m.leave.control = "start"
end sub

sub onLeave()
    uiNavigate({ to: "home", reset: true })
end sub

' The status line: key's words with values.
sub setStatus(key as string, values as dynamic, problem as boolean)
    m.statusWords = { key: key, values: values, text: "", problem: problem }
    showStatus()
end sub

' The status line: a sentence that came written, an answer's message.
sub setStatusText(text as string, problem as boolean)
    m.statusWords = { key: "", values: invalid, text: text, problem: problem }
    showStatus()
end sub

sub showStatus()
    words = m.statusWords
    text = words.text
    if words.key <> "" then text = t(words.key, words.values)
    typeSet(m.status, text)
    if words.problem
        m.status.color = uiColors().danger
    else
        m.status.color = uiColors().muted
    end if
end sub

' --- The globe ---

' The languages, Automatic naming what that is on this Roku.
sub openLanguages()
    facts = uiLanguageFacts()
    m.languages.automatic = i18nSystem(facts.serverLanguage, facts.device)
    m.languages.current = facts.choice
    m.languages.open = true
end sub

' A choice shows at once and is kept on this Roku (the scene), Automatic
' forgets it; the focus stays on the globe as the screen is drawn again.
sub onLanguagePicked(event as object)
    m.globe.setFocus(true)
    choice = event.getData()
    if choice = "auto" then choice = ""
    m.top.getScene().languageChoice = choice
end sub

sub onLanguagesClosed()
    m.globe.setFocus(true)
end sub

' The focus goes back from the globe where it came from.
sub leaveGlobe()
    if m.beforeGlobe <> invalid and m.beforeGlobe.visible
        m.beforeGlobe.setFocus(true)
    else
        focusButtons()
    end if
end sub

' Up from the buttons reaches the globe at the top; Down goes back to
' them. Left and Right go between Get a new code and the way back, never
' to the globe.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press or m.signedIn then return false
    if m.globe.isInFocusChain()
        if key = "OK"
            openLanguages()
        else if key = "down"
            leaveGlobe()
        end if
        return key = "OK" or key = "up" or key = "down" or key = "left" or key = "right" or key = "options"
    end if
    if key = "up" and (m.again.isInFocusChain() or m.back.isInFocusChain())
        m.beforeGlobe = m.back
        if m.again.isInFocusChain() then m.beforeGlobe = m.again
        m.globe.setFocus(true)
        return true
    end if
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
