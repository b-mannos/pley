' "Choose your server", as Android TV's: saved servers first, then those
' that answer on the local network. A saved server the search didn't find
' is asked directly, and its row says "Can't reach it" when it doesn't
' answer as Pley; the row chosen says "Connecting to …". Search this
' network asks every address of the TV's network (ApiTask's sweep), for
' servers that never hear the broadcast. An address typed on the keyboard
' that can't be used says why there, with the keyboard again.

sub init()
    m.list = m.top.findNode("list")
    m.actions = m.top.findNode("actions")
    m.status = m.top.findNode("status")
    m.hint = m.top.findNode("hint")
    m.actions.buttons = ["Enter an address", "Search this network"]
    m.saved = []
    m.found = []
    ' Those a sweep found, kept as the broadcast's list comes again.
    m.swept = []
    m.rows = []
    ' Saved servers that didn't answer as Pley ({ url: true }), and the row
    ' being connected to.
    m.unreachable = {}
    m.connecting = ""
    ' The latest choice, a row or an address typed, by number, and whether
    ' a server is being used: only the latest choice's answer leads on, as
    ' the session is that server's.
    m.choice = 0
    m.using = false
    m.started = false
    m.searching = false
    m.sweeping = false
    m.listFocused = false
    m.address = ""
    m.dialog = invalid
    m.forgetting = invalid
    m.list.observeField("itemSelected", "onServerSelected")
    m.actions.observeField("buttonSelected", "onAction")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        ' The sweep asks no more once the screen is left.
        if m.sweeping then uiRequest({ op: "sweepStop" }, "")
        m.sweeping = false
        return
    end if
    if not m.started
        m.started = true
        uiRequest({ op: "listServers" }, "onSaved")
        search()
    end if
    if m.rows.Count() > 0
        m.list.setFocus(true)
    else
        m.actions.setFocus(true)
    end if
end sub

sub onSaved(event as object)
    answer = event.getData()
    if answer.ok then m.saved = answer.data.servers
    render()
    probeSaved()
end sub

' Saved servers are asked directly, without a token, whether they answer as
' Pley (the address check's GET /api/server), as the search may not reach
' them: those that don't say so on their row. The ten used most lately.
sub probeSaved()
    asked = 0
    for each server in serversByLastUse(m.saved)
        if asked >= 10 then exit for
        asked = asked + 1
        uiRequest({ op: "checkServer", address: server.url }, "onProbed")
    end for
end sub

' What isn't Pley, or doesn't answer, can't be reached; a server not set up,
' or one beyond the home network over plain HTTP, answers as Pley.
sub onProbed(event as object)
    url = httpString(event.getRoSGNode().spec.address)
    answer = event.getData()
    if answer.ok or (answer.error <> "unreachable" and answer.error <> "not_pley")
        m.unreachable.Delete(url)
    else
        m.unreachable[url] = true
    end if
    render()
end sub

sub search()
    if m.searching then return
    m.searching = true
    setStatus("Looking for Pley on your network…", false)
    m.discovery = CreateObject("roSGNode", "DiscoveryTask")
    m.discovery.observeField("found", "onFound")
    m.discovery.observeField("done", "onSearched")
    m.discovery.control = "run"
end sub

sub onFound(event as object)
    m.found = event.getData()
    render()
end sub

sub onSearched()
    m.searching = false
    if not m.sweeping then sayFound()
end sub

sub sayFound()
    if m.rows.Count() = 0
        setStatus("No servers found. Make sure this TV and the server are on the same network.", false)
    else
        setStatus("", false)
    end if
end sub

' Search this network: every other address of the TV's /24, the servers
' coming as they answer. Without an IPv4 address, the broadcast again.
sub sweep()
    if m.sweeping then return
    range = discoverySweepRange(CreateObject("roDeviceInfo").GetConnectionInfo().ip)
    if range = ""
        search()
        return
    end if
    m.sweeping = true
    setStatus("Checking " + range + "…", false)
    request = uiRequest({ op: "sweep" }, "onSwept")
    request.observeField("update", "onSweepFound")
end sub

' A server the sweep found.
sub onSweepFound(event as object)
    server = httpObject(event.getData().server)
    if server = invalid then return
    m.found = discoveryAdd(m.found, server)
    m.unreachable.Delete(server.url)
    render()
end sub

sub onSwept()
    if not m.sweeping then return
    m.sweeping = false
    if not m.searching then sayFound()
end sub

sub render()
    focusedUrl = ""
    if m.rows.Count() > 0 and m.list.itemFocused >= 0 and m.list.itemFocused < m.rows.Count() then focusedUrl = m.rows[m.list.itemFocused].url
    m.rows = serversMerge(m.saved, m.found)
    items = []
    anySaved = false
    focusIndex = 0
    for i = 0 to m.rows.Count() - 1
        row = m.rows[i]
        tag = ""
        warning = false
        if row.lookalike
            tag = "Not your saved server"
            warning = true
        else if row.unencrypted
            tag = "Not encrypted"
            warning = true
        else if row.found
            tag = "On this network"
        end if
        ' Under the name: its address, or how connecting to it goes.
        line = row.address
        problem = false
        if row.url = m.connecting
            line = "Connecting to " + row.address + "…"
        else if not row.found and m.unreachable.DoesExist(row.url)
            line = "Can't reach it"
            problem = true
        end if
        if row.saved then anySaved = true
        if row.url = focusedUrl then focusIndex = i
        items.Push({ title: row.name, address: line, problem: problem, tag: tag, warning: warning, found: row.found })
    end for
    m.list.content = nodesContent(items)
    m.list.jumpToItem = focusIndex
    m.hint.visible = anySaved
    ' The first servers to show take the focus, once, unless a dialog has it.
    if m.rows.Count() > 0 and not m.listFocused and m.top.isInFocusChain()
        m.listFocused = true
        m.list.setFocus(true)
    end if
end sub

sub setStatus(text as string, problem as boolean)
    m.status.text = text
    if problem
        m.status.color = uiColors().danger
    else
        m.status.color = uiColors().muted
    end if
end sub

' One server is used at a time: a second OK before the first one's answer
' would have that answer lead on from a session already on the second
' server, past its PIN.
sub onServerSelected(event as object)
    if m.using then return
    row = m.rows[event.getData()]
    m.choice = m.choice + 1
    m.connecting = row.url
    render()
    useServer(row.url, row.name, row.id)
end sub

sub useServer(url as string, name as string, id as string)
    m.using = true
    uiRequest({ op: "useServer", url: url, name: name, id: id, choice: m.choice }, "onServerUsed")
end sub

sub onServerUsed(event as object)
    answer = event.getData()
    m.using = false
    m.connecting = ""
    if event.getRoSGNode().spec.choice <> m.choice
        render()
        return
    end if
    if not answer.ok
        render()
        setStatus(answer.message, true)
        return
    end if
    setStatus("", false)
    data = answer.data
    if not data.signedIn
        render()
        uiNavigate({ to: "link" })
        return
    end if
    ' A server chosen is a start on it, as on the other TVs: "Who's
    ' watching?" as the app's start asks it, and the PIN of the profile
    ' signed in there, whatever was answered on the server before.
    m.top.getScene().profileChosen = false
    uiNavigate({ to: data.screen, reset: true, params: { pin: httpTrue(data.pin) } })
end sub

sub onAction(event as object)
    if event.getData() = 0
        askAddress("")
    else
        sweep()
    end if
end sub

' The keyboard, with the address typed before; problem: why that one can't
' be used, said where it was typed, as on Android TV.
sub askAddress(problem as string)
    dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
    dialog.title = "Enter your server's address"
    if problem = ""
        dialog.message = ["Such as 192.168.1.20, nas or pley.example.com. Port 7539 is used by default."]
    else
        dialog.message = [problem]
    end if
    dialog.buttons = ["Connect", "Cancel"]
    dialog.text = m.address
    dialog.observeField("buttonSelected", "onAddressButton")
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

sub onAddressButton(event as object)
    if m.dialog = invalid then return
    address = m.dialog.text.Trim()
    m.dialog.close = true
    m.dialog = invalid
    if event.getData() <> 0 or address = "" then return
    m.address = address
    if m.using then return
    m.choice = m.choice + 1
    setStatus("Checking " + address + "…", false)
    uiRequest({ op: "checkServer", address: address, choice: m.choice }, "onChecked")
end sub

' A check that answers once a row or another address was chosen is let go.
sub onChecked(event as object)
    if m.using or event.getRoSGNode().spec.choice <> m.choice then return
    answer = event.getData()
    if not answer.ok
        setStatus("", false)
        askAddress(answer.message)
        return
    end if
    server = answer.data
    setStatus("Connecting to " + server.name + "…", false)
    useServer(server.url, server.name, "")
end sub

sub askForget(row as object)
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = "Forget " + row.name + "?"
    dialog.message = ["Pley signs this TV out of it and deletes what it saved from it."]
    dialog.buttons = ["Cancel", "Forget"]
    dialog.observeField("buttonSelected", "onForgetButton")
    m.forgetting = row
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

sub onForgetButton(event as object)
    if m.dialog <> invalid then m.dialog.close = true
    m.dialog = invalid
    if event.getData() <> 1 then return
    uiRequest({ op: "forgetServer", url: m.forgetting.url }, "onForgotten")
end sub

sub onForgotten(event as object)
    answer = event.getData()
    if answer.ok then m.saved = answer.data.servers
    render()
    if m.rows.Count() = 0 and m.top.isInFocusChain() then m.actions.setFocus(true)
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if key = "options" and m.list.isInFocusChain() and m.rows.Count() > 0
        row = m.rows[m.list.itemFocused]
        if row.saved
            askForget(row)
            return true
        end if
    end if
    if key = "right" and m.list.isInFocusChain()
        m.actions.setFocus(true)
        return true
    end if
    if key = "left" and m.actions.isInFocusChain() and m.rows.Count() > 0
        m.list.setFocus(true)
        return true
    end if
    return false
end function
