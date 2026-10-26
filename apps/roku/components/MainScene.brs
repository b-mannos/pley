sub init()
    m.top.backgroundUri = ""
    m.top.backgroundColor = uiColors().bg
    m.top.palette = uiPalette()
    m.screens = m.top.findNode("screens")
    m.stack = []

    session = CreateObject("roSGNode", "Session")
    m.global.addFields({ session: session })
    session.observeField("accessToken", "onAgentChanged")
    session.observeField("userAgent", "onAgentChanged")
    session.observeField("signedIn", "onSignedIn")
    onAgentChanged()

    m.api = CreateObject("roSGNode", "ApiTask")
    m.global.addFields({ api: m.api })
    m.api.observeField("ready", "onApiReady")
    m.api.control = "run"
end sub

' The task listens: the first request can go.
sub onApiReady()
    if m.api.ready then uiRequest({ op: "boot" }, "onBoot")
end sub

sub onBoot(event as object)
    answer = event.getData()
    screen = "servers"
    if answer.ok then screen = answer.data.screen
    show({ to: screen, reset: true })
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
end sub

sub pop()
    if m.stack.Count() <= 1 then return
    closeScreen(m.stack.Pop())
    top = m.stack.Peek()
    top.visible = true
    top.active = true
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
