' Saved servers and the ones found on the network, listed as the Android
' TV app lists them: saved ones first, most recently used first, then the
' ones found that aren't saved.

' The saved list from its JSON: [{ url, name, id, lastUsed }].
function serversParse(text as dynamic) as object
    list = []
    if GetInterface(text, "ifString") = invalid or text = "" then return list
    for each entry in httpArray(httpJson(text))
        if GetInterface(entry, "ifAssociativeArray") <> invalid
            url = urlOrigin(httpString(entry.url))
            if url <> invalid
                name = httpString(entry.name)
                if name = "" then name = urlDisplayAddress(url)
                list.Push({ url: url, name: name, id: httpString(entry.id), lastUsed: httpNumber(entry.last_used, 0) })
            end if
        end if
    end for
    return list
end function

function serversSerialize(list as object) as string
    out = []
    for each server in list
        out.Push({ url: server.url, name: server.name, id: server.id, last_used: server.lastUsed })
    end for
    return FormatJson(out)
end function

' Whether a and b are the same server: the same id, or the same address.
function serversSame(a as object, b as object) as boolean
    if a.id <> "" and a.id = b.id then return true
    return a.url = b.url
end function

' list with server in it, replacing one with the same address or id (a
' server found at a new address).
function serversWith(list as object, server as object) as object
    out = []
    for each saved in list
        if not serversSame(saved, server) then out.Push(saved)
    end for
    out.Push(server)
    return out
end function

function serversWithout(list as object, url as string) as object
    out = []
    for each saved in list
        if saved.url <> url then out.Push(saved)
    end for
    return out
end function

' The rows of the server list: { url, name, id, address, saved, found,
' lookalike, unencrypted }. lookalike: found, not saved, and named like a
' saved server it isn't.
function serversMerge(saved as object, found as object) as object
    rows = []
    for each server in serversByLastUse(saved)
        isFound = false
        for each other in found
            if serversSame(server, other) then isFound = true
        end for
        rows.Push(serversRow(server, true, isFound, false))
    end for
    for each server in found
        listed = false
        for each row in rows
            if serversSame(row, server) then listed = true
        end for
        if not listed
            lookalike = false
            for each other in saved
                if LCase(other.name.Trim()) = LCase(server.name.Trim()) then lookalike = true
            end for
            rows.Push(serversRow(server, false, true, lookalike))
        end if
    end for
    return rows
end function

function serversRow(server as object, saved as boolean, found as boolean, lookalike as boolean) as object
    return {
        url: server.url,
        name: server.name,
        id: server.id,
        address: urlDisplayAddress(server.url),
        saved: saved,
        found: found,
        lookalike: lookalike,
        unencrypted: urlUnencrypted(server.url)
    }
end function

' list, most recently used first.
function serversByLastUse(list as object) as object
    out = []
    for each server in list
        index = out.Count()
        for i = 0 to out.Count() - 1
            if server.lastUsed > out[i].lastUsed
                index = i
                exit for
            end if
        end for
        out = serversInsert(out, index, server)
    end for
    return out
end function

function serversInsert(list as object, index as integer, value as object) as object
    out = []
    for i = 0 to list.Count() - 1
        if i = index then out.Push(value)
        out.Push(list[i])
    end for
    if index >= list.Count() then out.Push(value)
    return out
end function

' The first screen: choosing a server, signing in to it, or Home.
function serversStartScreen(current as string, signedIn as boolean, askWho = false as boolean) as string
    if current = "" then return "servers"
    if not signedIn then return "link"
    if askWho then return "profiles"
    return "home"
end function

' The first screen on server, as the app starts or once the server is
' chosen: { screen, pin }. Signed in, "Who's watching?" comes first as
' atStart (Profiles.brs's profAtStart for the server) says, on the PIN
' pad of the profile signed in when pin is true. Choosing a server is a
' start on it: its profile's PIN is never skipped because another
' server's was given.
function serversStart(server as string, signedIn as boolean, atStart as string) as object
    if not signedIn then atStart = ""
    return { screen: serversStartScreen(server, signedIn, atStart <> ""), pin: atStart = "pin" }
end function
