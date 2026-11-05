' What can be done with a request from the screens that show it (a TMDB
' title's page, Discover's hero, the requests' panel), and signing in
' again for requests. On the render thread, with Ui.brs and
' lib/Requests.brs. A screen that includes this file has
' onRequestActed(), called once the server did what an action asked, or
' refused it as the request had changed or gone (m.requestTarget is the
' request it was for). What came of an action is said in a toast, as on
' the other TVs.

' Does an action (reqActions'): Watch opens the library's title; Request
' again asks as before; Withdraw, Remove and Delete ask first; Decline
' with a reason… asks for the reason on the keyboard; the others go at
' once. target is { requestId, title, status, stage, kind, tmdbId,
' seasons, libraryId, actions, mine }.
sub uiRequestAct(target as object, action as string)
    m.requestTarget = target
    if action = "watch"
        if httpNumber(target.libraryId, 0) > 0 then uiNavigate({ to: "details", params: { itemId: target.libraryId } })
        return
    end if
    if action = "ask_again"
        uiRequest({ method: "POST", path: "/api/requests", jsonText: reqAskBody(target.kind, target.tmdbId, target.seasons), action: action, requestTitle: target.title }, "uiRequestActed")
        return
    end if
    if action = "declineReason"
        uiRequestReason(target, "uiRequestReasonGiven")
        return
    end if
    confirm = reqConfirm(action, target.title, httpString(target.status))
    if confirm <> invalid
        m.requestConfirmed = action
        dialog = CreateObject("roSGNode", "StandardMessageDialog")
        dialog.title = confirm.message
        dialog.buttons = [confirm.button, "Cancel"]
        dialog.observeField("buttonSelected", "uiRequestConfirmed")
        m.top.getScene().dialog = dialog
        return
    end if
    uiRequestCall(action, "")
end sub

sub uiRequestConfirmed(event as object)
    uiCloseDialog()
    if event.getData() = 0 then uiRequestCall(m.requestConfirmed, "")
end sub

' A decline's reason on the keyboard, its buttons Decline and Cancel;
' callback gets the dialog's choice (0 for Decline) and reads the words
' with uiRequestReasonText.
sub uiRequestReason(target as object, callback as string)
    question = reqDeclineQuestion(target.title, httpTrue(target.mine))
    dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
    dialog.title = question.title
    dialog.message = question.message
    dialog.buttons = ["Decline", "Cancel"]
    dialog.observeField("buttonSelected", callback)
    m.top.getScene().dialog = dialog
end sub

' The reason typed, the keyboard closed.
function uiRequestReasonText() as string
    dialog = m.top.getScene().dialog
    reason = ""
    if dialog <> invalid then reason = dialog.text
    uiCloseDialog()
    return reason
end function

sub uiRequestReasonGiven(event as object)
    reason = uiRequestReasonText()
    if event.getData() = 0 then uiRequestCall("declineReason", reason)
end sub

sub uiRequestCall(action as string, reason as string)
    target = m.requestTarget
    if target = invalid then return
    spec = reqActionCall(action, target.requestId, reason)
    if spec = invalid then return
    spec.action = action
    spec.requestTitle = target.title
    uiRequest(spec, "uiRequestActed")
end sub

' The server's answer to an action. One it refused says why; the screen
' shows the request as it is now when it changed meanwhile (409, deleted:
' 404) or the limits moved (429).
sub uiRequestActed(event as object)
    answer = event.getData()
    spec = event.getRoSGNode().spec
    if not answer.ok
        if uiFailed(answer) then return
        text = reqActProblem(answer, spec.action, reqSummaryOf(m.top.getScene().requestSummary), httpString(m.requestTarget.kind), CreateObject("roDateTime").AsSeconds())
        uiToast(text, true)
        if answer.status = 404 or answer.status = 409 or answer.status = 429 then onRequestActed()
        return
    end if
    if spec.action = "ask_again" then uiToast(reqAsked(answer.status, answer.data, spec.requestTitle))
    onRequestActed()
end sub

' Signing in again, so this TV may ask for movies and shows (reason
' "requests") or decide on them ("approvals"), asked first: Sign in opens
' the link screen with a new code, whose approval replaces this sign-in.
sub uiSignInAgain(reason as string)
    m.signInReason = reason
    question = reqSignInQuestion(m.global.session.server)
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = question.title
    dialog.message = [question.message]
    dialog.buttons = ["Sign in", "Cancel"]
    dialog.observeField("buttonSelected", "uiSignInAgainChosen")
    m.top.getScene().dialog = dialog
end sub

sub uiSignInAgainChosen(event as object)
    uiCloseDialog()
    if event.getData() = 0 then uiNavigate({ to: "link", params: { reason: m.signInReason } })
end sub
