' Helpers of the screens, on the render thread. Requests go to the API
' task (never roUrlTransfer here); pictures and video carry the server's
' token in their HTTP agent.

' The app's colours (Android's, kept within Roku's broadcast-safe range).
function uiColors() as object
    return {
        bg: "0x16191EFF",
        surface: "0x1E222AFF",
        surface2: "0x262B35FF",
        surface3: "0x2E3440FF",
        fg: "0xEBEBEBFF",
        muted: "0xA1A9B6FF",
        subtle: "0x8B95A5FF",
        accent: "0x38BDF8FF",
        accentBright: "0x7DD3FCFF",
        onAccent: "0x16191EFF",
        danger: "0xEB6B70FF",
        success: "0x3FB27FFF"
    }
end function

' Dialogs and keyboards in the app's colours: the Scene's palette.
function uiPalette() as object
    colors = uiColors()
    palette = CreateObject("roSGNode", "RSGPalette")
    palette.colors = {
        DialogBackgroundColor: colors.surface,
        DialogItemColor: colors.fg,
        DialogTextColor: colors.fg,
        DialogFocusColor: colors.accent,
        DialogFocusItemColor: colors.onAccent,
        DialogSecondaryTextColor: colors.muted,
        DialogSecondaryItemColor: colors.muted,
        DialogInputFieldColor: colors.surface3,
        DialogKeyboardColor: colors.surface2,
        DialogFootprintColor: colors.surface3
    }
    return palette
end function

' Sends spec to the API task. callback, a function of this component, gets
' the answer as its event's data: { ok, status, data, error, message }.
' With spec.build, the task also builds the list's ContentNodes and puts
' them in the request's content before the answer.
function uiRequest(spec as object, callback as string) as object
    request = CreateObject("roSGNode", "ApiRequest")
    request.spec = spec
    if callback <> "" then request.observeField("result", callback)
    m.global.api.request = request
    return request
end function

function uiGet(path as string, callback as string) as object
    return uiRequest({ method: "GET", path: path }, callback)
end function

' An HTTP agent for the server's pictures, playlists, segments and
' subtitles: the app's User-Agent and the access token, which never goes
' in an address. Without withToken, for what anyone may fetch.
function uiHttpAgent(withToken = true as boolean) as object
    session = m.global.session
    agent = CreateObject("roHttpAgent")
    agent.AddHeader("User-Agent", session.userAgent)
    if withToken and session.accessToken <> "" then agent.AddHeader("Authorization", "Bearer " + session.accessToken)
    if LCase(Left(session.server, 8)) = "https://" then agent.SetCertificatesFile("common:/certs/ca-bundle.crt")
    return agent
end function

' Asks the scene for a screen: { to, params, reset, replace }, or
' { back: true }.
sub uiNavigate(nav as object)
    m.top.navigate = nav
end sub

' Opens what a tile stands for (a ContentNode, or its fields).
sub uiOpenTile(tile as dynamic)
    if tile = invalid then return
    nav = navForTile(tile)
    if nav <> invalid then uiNavigate(nav)
end sub

' The server doesn't answer: this screen gives way to one that says so,
' with Retry (this screen again, or retry) and Switch server. A covered
' screen leaves it be; it asks again when it shows.
sub uiUnreachable(retry = invalid as dynamic)
    if not m.top.active then return
    if retry = invalid then retry = { to: m.top.route, params: m.top.params }
    uiNavigate({ to: "unreachable", replace: true, params: { retry: retry } })
end sub

' What a failed answer leads to: true when it was handled here (the server
' doesn't answer, or the sign-in ended and the scene shows sign-in).
function uiFailed(answer as object) as boolean
    if answer.error = "network"
        uiUnreachable()
        return true
    end if
    return answer.error = "signed_out"
end function

' Roku's launch beacon, once: the first screen can be used.
sub uiLaunched()
    scene = m.top.getScene()
    if scene.launched then return
    scene.launched = true
    scene.signalBeacon("AppLaunchComplete")
end sub

' A message with an OK button.
sub uiMessage(title as string, lines as object)
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = title
    dialog.message = lines
    dialog.buttons = ["OK"]
    dialog.observeField("buttonSelected", "uiCloseDialog")
    m.top.getScene().dialog = dialog
end sub

sub uiCloseDialog()
    dialog = m.top.getScene().dialog
    if dialog <> invalid then dialog.close = true
end sub
