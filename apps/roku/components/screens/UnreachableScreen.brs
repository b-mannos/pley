sub init()
    m.title = m.top.findNode("title")
    m.text = m.top.findNode("text")
    m.buttons = m.top.findNode("buttons")
    m.buttons.buttons = ["Retry", "Switch server"]
    m.buttons.observeField("buttonSelected", "onButton")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active then return
    session = m.global.session
    m.title.text = "Can't reach " + session.serverName
    m.text.text = "Pley for Roku can't reach the server at " + urlDisplayAddress(session.server) + ". Check that it's on, and that this TV is on the same network as the server."
    m.buttons.setFocus(true)
    uiLaunched()
end sub

sub onButton(event as object)
    if event.getData() = 1
        uiNavigate({ to: "servers", reset: true })
        return
    end if
    retry = m.top.params.retry
    if retry = invalid or httpString(retry.to) = "" then retry = { to: "home", params: {} }
    uiNavigate({ to: retry.to, params: retry.params, replace: true })
end sub
