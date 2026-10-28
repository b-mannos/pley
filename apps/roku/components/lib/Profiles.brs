' "Who's watching?": the household's profiles (GET /api/oauth/profiles)
' as the screen shows them, and when it asks at a start, as the other TVs
' do: on a server whose household has more than one profile, for a
' sign-in allowed to switch them.

' The profiles as tiles: [{ title, subtitle, image, color, initial, itemId,
' needs, current }]. needs: what switching to it takes ("none", "pin",
' "password", "locked").
function profTiles(profiles as dynamic, server as string) as object
    tiles = []
    for each profile in httpArray(profiles)
        if GetInterface(profile, "ifAssociativeArray") <> invalid and httpNumber(profile.id, 0) > 0
            name = httpSentence(profile.name)
            needs = httpString(profile.needs)
            if needs = "" then needs = "none"
            tiles.Push({
                title: name,
                subtitle: profNeedsLabel(needs),
                image: urlImage(server, profile.avatar_url, 220),
                color: profColor(profile.avatar_color),
                initial: UCase(Left(name, 1)),
                itemId: httpNumber(profile.id, 0),
                needs: needs,
                current: httpTrue(profile.current)
            })
        end if
    end for
    return tiles
end function

' What a profile asks for under its name.
function profNeedsLabel(needs as string) as string
    if needs = "pin" then return "PIN"
    if needs = "password" then return "Password"
    if needs = "locked" then return "Not from the app"
    return ""
end function

' A profile's colour ("#38bdf8") as Roku writes colours; the app's own
' surface without one.
function profColor(hex as dynamic) as string
    text = httpString(hex)
    re = CreateObject("roRegex", "^#[0-9a-fA-F]{6}$", "")
    if not re.IsMatch(text) then return "0x2E3440FF"
    return "0x" + UCase(Mid(text, 2)) + "FF"
end function

' Whether the app asks "Who's watching?" as it starts: a sign-in allowed to
' switch profiles, on a server whose household had more than one when last
' asked.
function profAskWho(scope as string, household as integer) as boolean
    return authAllows(scope, "profiles") and household > 1
end function
