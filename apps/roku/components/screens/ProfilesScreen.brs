' "Who's watching?" (GET /api/oauth/profiles), as the other TVs ask it: OK
' on a profile switches to it (the switch-profile grant, through the API
' task), after its PIN for one that has one; the one signed in goes on,
' its PIN asked first at a start. A profile that needs its password is
' approved with a code on a phone, the sign-in screen saying so ("Approve
' on your phone"); one that can't be used from the app says so on its PIN
' pad, as Android TV's locked pad does. Shown as the app starts when the
' household has more than one profile, as a server is chosen, and from
' Settings. params.pin: a start as the profile signed in ("Always start
' as"), whose PIN pad opens first; Use another profile shows the profiles.

sub init()
    m.profiles = m.top.findNode("profiles")
    m.message = m.top.findNode("message")
    m.buttons = m.top.findNode("buttons")
    m.started = false
    m.list = []
    ' Who the app is (profMe), and the answers load waits for.
    m.me = invalid
    m.answers = {}
    ' The profile a PIN is being typed for, and the dialog.
    m.picked = invalid
    m.dialog = invalid
    m.busy = false
    ' A PIN went with the switch under way.
    m.sentPin = false
    ' The PIN pad of the profile signed in opens once the profiles show it.
    m.padFirst = false
    ' The profiles couldn't be loaded: Retry shows, and the server answering
    ' again (reached) loads them by itself.
    m.failed = false
    m.profiles.observeField("itemSelected", "onPicked")
    m.buttons.observeField("buttonSelected", "onButton")
    m.top.observeField("active", "onActive")
    m.top.observeField("reached", "onReached")
end sub

sub onActive()
    if not m.top.active then return
    if not m.started
        m.started = true
        m.padFirst = m.top.params <> invalid and httpTrue(m.top.params.pin) and not m.top.getScene().profileChosen
        load()
    end if
    if m.list.Count() > 0
        m.profiles.setFocus(true)
    else if m.buttons.visible
        m.buttons.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

' A sign-in from before the app asked to switch profiles can't list them:
' signing in again as another profile, choosing it in the browser, does
' instead.
sub load()
    m.failed = false
    if not authAllows(m.global.session.scope, "profiles")
        showMessage("This sign-in can't switch profiles: sign in again, and choose the profile in the browser.", [anotherProfile()])
        return
    end if
    showMessage("Loading…", [])
    m.answers = {}
    uiGet("/api/oauth/profiles", "onProfiles")
    ' Who the app is, for the PIN of the profile signed in; alongside the
    ' list.
    uiGet("/api/auth/me", "onMe")
end sub

function anotherProfile() as string
    return "Sign in as another profile"
end function

sub onReached()
    if m.failed and not m.busy then load()
end sub

sub onProfiles(event as object)
    m.answers.profiles = event.getData()
    if m.answers.DoesExist("me") then showProfiles()
end sub

sub onMe(event as object)
    m.answers.me = event.getData()
    if m.answers.DoesExist("profiles") then showProfiles()
end sub

sub showProfiles()
    answer = m.answers.profiles
    ' Without who the app is, a start can't tell whether to ask its PIN:
    ' that is tried again too.
    if answer.ok and not m.answers.me.ok then answer = m.answers.me
    if not answer.ok
        if answer.error = "signed_out" then return
        m.failed = true
        if answer.status = 403
            showMessage("This sign-in can't switch profiles: sign in again, and choose the profile in the browser.", ["Retry", anotherProfile()])
        else
            showMessage(answer.message, ["Retry"])
        end if
        return
    end if
    m.me = profMe(m.answers.me.data)
    m.list = profTiles(answer.data, m.global.session.server, m.me)
    profKeepHousehold(CreateObject("roRegistrySection", "household"), m.global.session.server, answer.data)
    if m.list.Count() = 0
        m.failed = true
        showMessage("No profiles to choose from.", ["Retry"])
        return
    end if
    shown = m.list.Count()
    if shown > 6 then shown = 6
    m.profiles.numColumns = shown
    m.profiles.translation = [(1920 - shown * 220 - (shown - 1) * 48) / 2, 400]
    m.profiles.content = nodesContent(m.list)
    for i = 0 to m.list.Count() - 1
        if m.list[i].current then m.profiles.jumpToItem = i
    end for
    showMessage("", [])
    if m.top.isInFocusChain() then m.profiles.setFocus(true)
    pad = profFirstPad(m.list, m.padFirst)
    m.padFirst = false
    if pad <> invalid then askPin(pad, "")
end sub

sub showMessage(text as string, buttons as object)
    m.message.text = text
    m.buttons.buttons = buttons
    m.buttons.visible = buttons.Count() > 0
    if buttons.Count() > 0 and m.top.isInFocusChain() then m.buttons.setFocus(true)
end sub

sub onButton(event as object)
    if m.busy then return
    if m.buttons.buttons[event.getData()] = "Retry"
        load()
    else
        uiNavigate({ to: "link", params: { reason: "another" } })
    end if
end sub

sub onPicked(event as object)
    if m.busy then return
    profile = m.list[event.getData()]
    m.message.text = ""
    choice = profChoice(profile, m.me, not m.top.getScene().profileChosen)
    if choice = "done"
        done()
    else if choice = "switch"
        switchTo(profile, "")
    else if choice = "confirm" or choice = "pin"
        askPin(profile, "")
    else if choice = "approve"
        approve(profile)
    else
        locked(profile, "This profile can't be used from the app.")
    end if
end sub

' The PIN pad, saying why it asks again when it does; Use another profile
' goes back to the profiles.
sub askPin(profile as object, problem as string)
    m.picked = profile
    dialog = CreateObject("roSGNode", "StandardPinPadDialog")
    dialog.title = "Enter " + profile.title + "'s PIN"
    lines = []
    if problem <> "" then lines.Push(problem)
    dialog.message = lines
    dialog.buttons = ["OK", "Use another profile"]
    dialog.observeField("buttonSelected", "onPin")
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

sub onPin(event as object)
    pin = ""
    if m.dialog <> invalid then pin = httpString(m.dialog.pin)
    closeDialog()
    if event.getData() <> 0 or m.picked = invalid then return
    if Len(pin) < 4
        askPin(m.picked, "PINs are 4 to 8 digits.")
        return
    end if
    switchTo(m.picked, pin)
end sub

' A profile whose PIN can't be typed here (it can't be used from the app,
' or its PIN is locked after too many wrong tries): Android TV's pad,
' dimmed, with why and Use another profile. Roku's pad can't be dimmed, so
' a message stands in its place.
sub locked(profile as object, why as string)
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = "Enter " + profile.title + "'s PIN"
    dialog.message = [why]
    dialog.buttons = ["Use another profile"]
    dialog.observeField("buttonSelected", "onLocked")
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

sub onLocked()
    closeDialog()
end sub

' The profile needs its password, which apps never see: a code approved on
' a phone signed in to Pley, picking the profile there, as Android TV's
' "Approve on your phone". Use another profile comes back here.
sub approve(profile as object)
    uiNavigate({ to: "link", params: { reason: "profile", name: profile.title } })
end sub

sub closeDialog()
    if m.dialog <> invalid then m.dialog.close = true
    m.dialog = invalid
end sub

sub switchTo(profile as object, pin as string)
    m.busy = true
    m.picked = profile
    m.sentPin = pin <> ""
    m.message.text = "Switching to " + profile.title + "…"
    uiRequest({ op: "switchProfile", profile: profile.itemId, pin: pin }, "onSwitched")
end sub

sub onSwitched(event as object)
    m.busy = false
    answer = event.getData()
    ' No PIN to check is fine for the profile signed in only.
    if answer.ok and (not answer.data.through or m.picked.current)
        done()
        return
    end if
    if answer.ok
        m.message.text = "The server couldn't switch to " + m.picked.title + "."
        return
    end if
    m.message.text = ""
    if answer.error = "signed_out" then return
    if answer.error = "wrong_pin" and not m.sentPin
        ' The list was older than the profile's PIN: it is asked for.
        askPin(m.picked, "")
    else if answer.error = "wrong_pin" or answer.error = "throttled"
        ' The pad again, saying why: a wrong PIN, or how long to wait.
        askPin(m.picked, answer.message)
    else if answer.error = "pin_locked"
        locked(m.picked, answer.message)
    else if answer.error = "browser"
        approve(m.picked)
    else
        m.message.text = answer.message
    end if
end sub

' While a switch is under way the screen stays: Back would leave the app
' half switched.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press or not m.busy then return false
    return key = "back" or key = "OK" or key = "options"
end function

' The profile chosen: Home, everything as that profile sees it.
sub done()
    m.top.getScene().profileChosen = true
    uiNavigate({ to: "home", reset: true })
end sub
