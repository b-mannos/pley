sub init()
    m.top.backgroundUri = ""
    m.top.backgroundColor = uiColors().bg
    m.top.palette = uiPalette()
    m.screens = m.top.findNode("screens")
    m.stack = []
    ' The sign-in screen shown at launch, until another replaces it.
    m.launchDialog = invalid

    session = CreateObject("roSGNode", "Session")
    m.global.addFields({ session: session })
    session.observeField("accessToken", "onAgentChanged")
    session.observeField("userAgent", "onAgentChanged")
    session.observeField("signedIn", "onSignedIn")
    onAgentChanged()

    m.api = CreateObject("roSGNode", "ApiTask")
    m.global.addFields({ api: m.api })

    ' Music plays on while the screens change.
    m.music = CreateObject("roSGNode", "MusicPlayer")
    m.top.appendChild(m.music)
    m.global.addFields({ music: m.music })

    ' What the person's other devices change, as it happens (an http://
    ' server's event stream): gathered for 0.3 s, then the screens follow.
    m.events = CreateObject("roSGNode", "EventsTask")
    m.events.observeField("event", "onServerEvent")
    m.gather = m.top.findNode("gather")
    m.gather.observeField("fire", "onGathered")
    session.observeField("accessToken", "onStream")
    session.observeField("server", "onStream")
    m.events.control = "run"
    m.api.observeField("ready", "onApiReady")
    m.api.control = "run"
end sub

' The task listens: the first request can go.
sub onApiReady()
    if m.api.ready then uiRequest({ op: "boot" }, "onBoot")
end sub

' Roku's launch beacons: AppLaunchComplete once the first screen can be
' used (Servers and sign-in at once, Home once its rows show), and around a
' sign-in shown at launch AppDialogInitiate and AppDialogComplete, whose
' time Roku leaves out of the launch's.
sub onBoot(event as object)
    answer = event.getData()
    screen = "servers"
    if answer.ok then screen = answer.data.screen
    if screen = "link" then m.top.signalBeacon("AppDialogInitiate")
    show({ to: screen, reset: true })
    if screen = "link" then m.launchDialog = m.stack.Peek()
    if screen <> "home" then uiLaunched()
end sub

' The sign-in shown at launch is over once another screen replaces it.
sub launchDialogDone()
    if m.launchDialog = invalid then return
    if m.stack.Count() > 0 and m.stack.Peek().isSameNode(m.launchDialog) then return
    m.launchDialog = invalid
    m.top.signalBeacon("AppDialogComplete")
end sub

' The event stream follows the server and the token signed in with.
sub onStream()
    session = m.global.session
    m.events.userAgent = session.userAgent
    m.events.server = session.server
    m.events.token = session.accessToken
end sub

' Watch state, My List or libraries changed elsewhere: Home asks again
' (now if it shows, else as it shows); a stream of this Roku's stopped from
' elsewhere ends at once.
sub onServerEvent(event as object)
    data = event.getData()
    if httpString(data.type) = "session_stopped"
        if m.stack.Count() > 0 and m.stack.Peek().hasField("serverStopped") then m.stack.Peek().serverStopped = data
        return
    end if
    if not evChangesHome(data) then return
    m.gather.control = "stop"
    m.gather.control = "start"
end sub

sub onGathered()
    m.top.remoteChanges = m.top.remoteChanges + 1
    if m.stack.Count() > 0 and m.stack.Peek().hasField("refresh") then m.stack.Peek().refresh = true
end sub

' Pictures load through the scene's HTTP agent (nodes without their own
' use their nearest parent's): a new token makes a new agent.
sub onAgentChanged()
    m.top.setHttpAgent(uiHttpAgent())
end sub

' Signed out by the server (or by the person, in Settings): sign in again.
sub onSignedIn()
    session = m.global.session
    if session.signedIn or m.stack.Count() = 0 or session.server = "" then return
    top = m.stack.Peek().subtype()
    if top = "LinkScreen" or top = "ServersScreen" then return
    show({ to: "link", reset: true, params: { reason: session.signOutReason } })
end sub

sub onNavigate(event as object)
    show(event.getData())
end sub

' nav: { to, params, reset, replace } or { back: true }, with a message
' ({ title, lines }) to show over the screen it leads to.
sub show(nav as object)
    if httpTrue(nav.back)
        pop()
        if nav.message <> invalid then uiMessage(nav.message.title, nav.message.lines)
        return
    end if
    route = httpString(nav.to)
    name = navScreen(route)
    if name = "" then return
    ' Requests failing together say so once.
    if name = "UnreachableScreen" and m.stack.Count() > 0
        if m.stack.Peek().subtype() = name then return
    end if
    if httpTrue(nav.reset)
        ' Another profile, sign-in or server: what it played ends.
        m.music.control = "stop"
        while m.stack.Count() > 0
            closeScreen(m.stack.Pop())
        end while
    else if httpTrue(nav.replace) and m.stack.Count() > 0
        closeScreen(m.stack.Pop())
    end if
    if m.stack.Count() > 0
        covered = m.stack.Peek()
        covered.active = false
        covered.visible = false
    end if
    screen = CreateObject("roSGNode", name)
    params = nav.params
    if params = invalid then params = {}
    screen.route = route
    screen.params = params
    screen.observeField("navigate", "onNavigate")
    m.stack.Push(screen)
    m.screens.appendChild(screen)
    screen.active = true
    launchDialogDone()
end sub

sub pop()
    if m.stack.Count() <= 1 then return
    closeScreen(m.stack.Pop())
    top = m.stack.Peek()
    top.visible = true
    top.active = true
    launchDialogDone()
end sub

sub closeScreen(screen as object)
    screen.unobserveField("navigate")
    screen.active = false
    m.screens.removeChild(screen)
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if press and key = "back" and m.stack.Count() > 1
        pop()
        return true
    end if
    ' Back on the first screen leaves the app.
    return false
end function
