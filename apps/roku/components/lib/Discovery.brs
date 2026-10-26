' Finding pley servers on the local network (apps/server/src/discovery.rs):
' "pley?" goes to UDP port 7539 as a broadcast, and each server answers
' JSON { pley, id, name, version, port } from its own address.

function discoveryRequest() as string
    return "pley?"
end function

function discoveryPort() as integer
    return 7539
end function

' Where the request goes: the general broadcast address and, from the TV's
' own IPv4 address, its /24 network's (roDeviceInfo says no netmask).
function discoveryTargets(localIp as dynamic) as object
    targets = ["255.255.255.255"]
    re = CreateObject("roRegex", "^([0-9]+)\.([0-9]+)\.([0-9]+)\.[0-9]+$", "")
    parts = re.Match(httpString(localIp))
    if parts.Count() = 4 then targets.Push(parts[1] + "." + parts[2] + "." + parts[3] + ".255")
    return targets
end function

' The server behind an answer from fromAddress ("192.168.1.20:7539", as
' roSocketAddress writes it): { url, name, id, version }, or invalid.
function discoveryAnswer(text as dynamic, fromAddress as string) as dynamic
    if GetInterface(text, "ifString") = invalid or text = "" then return invalid
    data = httpObject(httpJson(text))
    if data = invalid then return invalid
    if httpNumber(data.pley, 0) <> 1 then return invalid
    port = httpNumber(data.port, 0)
    if port <= 0 or port > 65535 then return invalid
    host = discoveryHost(fromAddress)
    if host = "" then return invalid
    name = httpSentence(data.name)
    if name = "" then name = host
    return {
        url: "http://" + host + ":" + Int(port).ToStr(),
        name: name,
        id: httpString(data.id),
        version: httpString(data.version)
    }
end function

' The host of "192.168.1.20:7539" or "[fe80::1]:7539", without the port.
function discoveryHost(address as string) as string
    if Left(address, 1) = "["
        bracket = Instr(1, address, "]")
        if bracket = 0 then return ""
        return LCase(Left(address, bracket))
    end if
    colon = Instr(1, address, ":")
    if colon = 0 then return address
    return Left(address, colon - 1)
end function

' found with server in it, once per server (by id, else address), by name.
function discoveryAdd(found as object, server as object) as object
    for each known in found
        if serversSame(known, server) then return found
    end for
    out = []
    added = false
    for each known in found
        if not added and LCase(server.name) < LCase(known.name)
            out.Push(server)
            added = true
        end if
        out.Push(known)
    end for
    if not added then out.Push(server)
    return out
end function
