' "pley?" to UDP port 7539 as a broadcast, sent twice; the answers are read
' for two seconds (apps/server/src/discovery.rs).

sub init()
    m.top.functionName = "discoveryRun"
end sub

sub discoveryRun()
    port = CreateObject("roMessagePort")
    socket = CreateObject("roDatagramSocket")
    socket.setMessagePort(port)
    ' Any free port: the servers answer to the one the request came from.
    bindTo = CreateObject("roSocketAddress")
    bindTo.setPort(0)
    socket.setAddress(bindTo)
    socket.setBroadcast(true)
    socket.notifyReadable(true)
    targets = discoveryTargets(CreateObject("roDeviceInfo").GetConnectionInfo().ip)
    found = []
    clock = CreateObject("roTimespan")
    sent = 0
    while clock.TotalMilliseconds() < 2000
        ' Once at the start and once after half a second: UDP may lose one.
        if sent = 0 or (sent = 1 and clock.TotalMilliseconds() >= 500)
            discoverySend(socket, targets)
            sent = sent + 1
        end if
        msg = wait(100, port)
        if type(msg) = "roSocketEvent" and msg.getSocketID() = socket.getID()
            while socket.isReadable()
                text = socket.receiveStr(2048)
                from = socket.getReceivedFromAddress()
                address = ""
                if from <> invalid then address = from.getAddress()
                server = discoveryAnswer(text, address)
                if server <> invalid
                    found = discoveryAdd(found, server)
                    m.top.found = found
                end if
            end while
        end if
    end while
    socket.close()
    m.top.done = true
end sub

sub discoverySend(socket as object, targets as object)
    for each target in targets
        address = CreateObject("roSocketAddress")
        address.setAddress(target + ":" + discoveryPort().ToStr())
        socket.setSendToAddress(address)
        socket.sendStr(discoveryRequest())
    end for
end sub
