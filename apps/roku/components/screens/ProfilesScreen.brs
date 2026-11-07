' "Who's watching?" (GET /api/oauth/profiles), as the other TVs ask it: OK
' on a profile switches to it (the switch-profile grant, through the API
' task), after its PIN for one that has one; the one signed in goes on,
' its PIN asked first at a start. A profile that needs its password is
' approved with a code on a phone, the sign-in screen saying so ("Approve
' on your phone"); one that can't be used from the app says so on its PIN
' pad, as Android TV's locked pad does. Shown as the app starts when the
' profile left on has a PIN in a household of more than one, as a server
' is chosen, and from Settings. params.pin: a start as the profile signed
' in ("Always start as"), whose PIN pad opens first; Use another profile
' shows the profiles.

sub init()
    m.title = m.top.findNode("title")
    m.profiles = m.top.findNode("profiles")
    m.message = m.top.findNode("message")
    m.buttons = m.top.findNode("buttons")
    ' What the message says ({ key, values, text }) and the buttons under
    ' it ("retry", "another"), drawn again in a new language.
    m.words = { key: "", values: invalid, text: "" }
    m.buttonActions = []
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
    m.top.observeField("language", "relabel")
    relabel()
end sub

' The screen's words in the language shown; the profiles' names are
' their own, what each asks for under it is drawn again.
sub relabel()
    typeSet(m.title, t("signin.profiles.title"))
    showWords()
    if m.me <> invalid and m.answers.DoesExist("profiles") and m.list.Count() > 0
        focused = m.profiles.itemFocused
        m.list = profTiles(m.answers.profiles.data, m.global.session.server, m.me)
        m.profiles.content = nodesContent(m.list)
        if focused >= 0 and focused < m.list.Count() then m.profiles.jumpToItem = focused
    end if
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
        showMessage("signin.profiles.cantSwitchSignIn", ["another"])
        return
    end if
    showMessage("common.states.loading", [])
    m.answers = {}
    uiGet("/api/oauth/profiles", "onProfiles")
    ' Who the app is, for the PIN of the profile signed in; alongside the
    ' list.
    uiGet("/api/auth/me", "onMe")
end sub

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
            showMessage("signin.profiles.cantSwitchSignIn", ["retry", "another"])
        else
            showMessageText(answer.message, ["retry"])
        end if
        return
    end if
    m.me = profMe(m.answers.me.data)
    m.list = profTiles(answer.data, m.global.session.server, m.me)
    profKeepHousehold(CreateObject("roRegistrySection", "household"), m.global.session.server, answer.data)
    if m.list.Count() = 0
        m.failed = true
        showMessage("signin.profiles.none", ["retry"])
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

' The message: key's words ("" for none), and the buttons under it, by
' what they do.
sub showMessage(key as string, buttons as object)
    m.words = { key: key, values: invalid, text: "" }
    m.buttonActions = buttons
    showWords()
    if buttons.Count() > 0 and m.top.isInFocusChain() then m.buttons.setFocus(true)
end sub

' The message as it came written (an answer's), with its buttons.
sub showMessageText(text as string, buttons as object)
    m.words = { key: "", values: invalid, text: text }
    m.buttonActions = buttons
    showWords()
    if buttons.Count() > 0 and m.top.isInFocusChain() then m.buttons.setFocus(true)
end sub

' A word under the profiles, the buttons staying ("" for none).
sub say(key as string, values = invalid as dynamic)
    m.words = { key: key, values: values, text: "" }
    showWords()
end sub

sub sayText(text as string)
    m.words = { key: "", values: invalid, text: text }
    showWords()
end sub

sub showWords()
    text = m.words.text
    if m.words.key <> "" then text = t(m.words.key, m.words.values)
    typeSet(m.message, text)
    words = { retry: "common.actions.retry", another: "signin.profiles.another" }
    labels = []
    for each action in m.buttonActions
        labels.Push(t(words[action]))
    end for
    m.buttons.buttons = labels
    m.buttons.visible = labels.Count() > 0
end sub

sub onButton(event as object)
    if m.busy then return
    at = event.getData()
    if at < 0 or at >= m.buttonActions.Count() then return
    if m.buttonActions[at] = "retry"
        load()
    else
        uiNavigate({ to: "link", params: { reason: "another" } })
    end if
end sub

sub onPicked(event as object)
    if m.busy then return
    profile = m.list[event.getData()]
    say("")
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
        locked(profile, t("signin.profiles.unusable"))
    end if
end sub

' The PIN pad, saying why it asks again when it does; Use another profile
' goes back to the profiles.
sub askPin(profile as object, problem as string)
    m.picked = profile
    dialog = CreateObject("roSGNode", "StandardPinPadDialog")
    dialog.title = t("signin.profiles.pinFor", { name: profile.title })
    lines = []
    if problem <> "" then lines.Push(problem)
    dialog.message = lines
    dialog.buttons = [t("common.actions.ok"), t("signin.profiles.useAnother")]
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
        askPin(m.picked, t("signin.profiles.pinLength"))
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
    dialog.title = t("signin.profiles.pinFor", { name: profile.title })
    dialog.message = [why]
    dialog.buttons = [t("signin.profiles.useAnother")]
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
    say("signin.profiles.switching", { name: profile.title })
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
        say("signin.profiles.couldntSwitch", { name: m.picked.title })
        return
    end if
    say("")
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
        sayText(answer.message)
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
