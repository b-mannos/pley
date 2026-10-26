' The player (docs/API.md, "Playback"). Progress goes to the server about
' every 10 s while playing, on pause, and as "stopped" on the way out, then
' the session is closed. A stream that breaks is checked with the server:
' stopped by an administrator (410), the player leaves and says so;
' otherwise it starts again where it was, with a new session and the
' current token (a token that ran out answers 401 to the Video node).
' Skip Intro and Skip Credits follow the web player: an episode's credits
' bring up Up Next instead.

sub init()
    m.video = m.top.findNode("video")
    m.skip = m.top.findNode("skip")
    m.loading = m.top.findNode("loading")
    m.upNext = m.top.findNode("upNext")
    m.nextImage = m.top.findNode("nextImage")
    m.nextTitle = m.top.findNode("nextTitle")
    m.nextLine = m.top.findNode("nextLine")
    m.countdown = m.top.findNode("countdown")
    m.nextButtons = m.top.findNode("nextButtons")
    m.notice = m.top.findNode("notice")
    m.noticeTitle = m.top.findNode("noticeTitle")
    m.noticeText = m.top.findNode("noticeText")
    m.noticeButtons = m.top.findNode("noticeButtons")
    m.countdownTimer = m.top.findNode("countdownTimer")
    m.clock = CreateObject("roTimespan")
    m.started = false
    ' Which opening the answers belong to: answers for an earlier one are dropped.
    m.attempt = 0
    ' The playback session asked for last, until it answers.
    m.playRequest = invalid
    m.itemId = 0
    m.item = invalid
    m.info = invalid
    m.sessionId = ""
    m.mode = "auto"
    m.startMs = 0
    m.positionMs = 0
    m.durationMs = 0
    m.lastReportAt = 0
    m.paused = false
    ' Whether this opening's video has played yet (errors before are the file's).
    m.played = false
    m.next = invalid
    m.upNextAt = -1
    m.upNextShown = false
    m.upNextCancelled = false
    m.countdownLeft = 0
    m.restarts = []
    ' The caption mode before pley turned captions on for its subtitles.
    m.captionMode = invalid
    ' The file's intro and credits, and the one the playhead is in; a part
    ' just skipped keeps its button away until the playhead is past it.
    m.markers = []
    m.skipMarker = invalid
    m.skipKey = ""
    m.skipDismissed = false
    m.video.notificationInterval = 1
    m.video.observeField("state", "onState")
    m.video.observeField("position", "onPosition")
    m.skip.observeField("pressed", "onSkip")
    m.skip.observeField("playKey", "onSkipKey")
    m.nextButtons.buttons = ["Play now", "Cancel"]
    m.nextButtons.observeField("buttonSelected", "onNextButton")
    m.noticeButtons.observeField("buttonSelected", "onNoticeButton")
    m.countdownTimer.observeField("fire", "onCountdown")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        ' Leaving: Back, or the sign-in ended.
        finish()
        return
    end if
    if m.started then return
    m.started = true
    ' Playlists and segments carry the token: a refreshed one goes to the
    ' Video node's agent at once.
    m.global.session.observeFieldScoped("accessToken", "onToken")
    params = m.top.params
    open(params.itemId, Int(httpNumber(params.startMs, 0)), params.item, "auto")
end sub

sub onToken()
    m.video.setHttpAgent(uiHttpAgent())
end sub

' Plays itemId from startMs. With its details already there (item), they
' and a playback session are asked for together; else the details first,
' since its file says which subtitles to ask the server to draw.
sub open(itemId as dynamic, startMs as integer, item as dynamic, mode as string)
    abandonPlayback()
    m.attempt = m.attempt + 1
    m.itemId = itemId
    m.startMs = startMs
    m.positionMs = startMs
    m.mode = mode
    m.item = item
    m.info = invalid
    m.sessionId = ""
    m.paused = false
    m.played = false
    m.next = invalid
    m.upNextShown = false
    m.upNextCancelled = false
    m.markers = []
    m.skipMarker = invalid
    m.skipKey = ""
    m.skipDismissed = false
    hideSkip()
    hideUpNext()
    m.notice.visible = false
    m.loading.visible = true
    m.top.setFocus(true)
    if m.item = invalid
        uiRequest({ method: "GET", path: "/api/items/" + urlValue(itemId), attempt: m.attempt }, "onItem")
        return
    end if
    itemKnown()
    askPlayback()
end sub

' POST /api/playback for the first file (the server's default too), with
' its picture subtitles to draw when the person's choices pick some.
sub askPlayback()
    spec = { op: "play", itemId: m.itemId, startMs: m.startMs, mode: m.mode, attempt: m.attempt }
    file = contentFile(m.item, invalid)
    if file <> invalid
        spec.fileId = httpNumber(file.id, 0)
        burn = playerBurnIndex(file)
        if burn >= 0 then spec.subtitleIndex = burn
    end if
    m.playRequest = uiRequest(spec, "onPlayback")
end sub

' The session asked for isn't wanted any more: the task closes it if it
' comes (and onPlayback does, if it came already).
sub abandonPlayback()
    if m.playRequest = invalid then return
    m.playRequest.abandoned = true
    m.playRequest = invalid
end sub

function isCurrent(event as object) as boolean
    return event.getRoSGNode().spec.attempt = m.attempt
end function

sub onItem(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        failed(answer)
        return
    end if
    m.item = answer.data
    itemKnown()
    askPlayback()
end sub

' An episode has a next one to offer before its end.
sub itemKnown()
    if httpString(m.item.kind) <> "episode" then return
    uiRequest({ method: "GET", path: "/api/items/" + urlValue(m.itemId) + "/next", attempt: m.attempt }, "onNext")
end sub

sub onPlayback(event as object)
    answer = event.getData()
    if not isCurrent(event)
        ' For an opening left meanwhile (Back while it started): the server
        ' would go on preparing it.
        if answer.ok and answer.data <> invalid then closeSession(httpString(answer.data.session_id))
        return
    end if
    m.playRequest = invalid
    if not answer.ok or answer.data = invalid
        failed(answer)
        return
    end if
    m.info = answer.data
    m.sessionId = httpString(m.info.session_id)
    begin()
end sub

' Plays once both the item and the session are there.
sub begin()
    if m.item = invalid or m.info = invalid then return
    m.durationMs = httpNumber(m.info.duration_ms, 0)
    m.markers = playerFileMarkers(m.item, m.info.file_id)
    m.upNextAt = playerUpNextAt(m.durationMs, m.markers)
    stream = contentStream(m.info, m.item, m.global.session.server, m.startMs)
    content = CreateObject("roSGNode", "ContentNode")
    content.url = stream.url
    content.streamFormat = stream.streamFormat
    content.title = stream.title
    content.playStart = stream.playStart
    content.subtitleTracks = stream.subtitleTracks
    if LCase(Left(stream.url, 8)) = "https://" then nodesFill(content, { HttpCertificatesFile: "common:/certs/ca-bundle.crt" })
    m.video.setHttpAgent(uiHttpAgent())
    m.video.content = content
    if stream.subtitle <> ""
        m.video.subtitleTrack = stream.subtitle
        captionsOn()
    else
        captionsBack()
    end if
    m.video.control = "play"
    m.video.setFocus(true)
    m.lastReportAt = m.clock.TotalMilliseconds()
end sub

' Roku shows a subtitle track only while captions are on, a setting of
' the whole Roku: the subtitles pley picks turn them on for its playback,
' and they go back as they were after, unless the person changed them
' meanwhile.
sub captionsOn()
    if m.captionMode = invalid then m.captionMode = m.video.globalCaptionMode
    m.video.globalCaptionMode = "On"
end sub

sub captionsBack()
    if m.captionMode = invalid then return
    if m.video.globalCaptionMode = "On" then m.video.globalCaptionMode = m.captionMode
    m.captionMode = invalid
end sub

' A request before playback failed: the server doesn't answer (its own
' screen), the sign-in ended (the scene shows sign-in), or a refusal.
sub failed(answer as object)
    if answer.error = "signed_out" then return
    if answer.error = "network"
        uiUnreachable({ to: "player", params: { itemId: m.itemId, startMs: m.positionMs } })
        return
    end if
    notice("Can't play this item", answer.message, ["Back"])
end sub

sub onState(event as object)
    state = event.getData()
    if state = "playing"
        m.played = true
        m.loading.visible = false
        if m.paused
            m.paused = false
            report("playing")
        end if
        updateSkip()
        return
    end if
    ' Paused, in trick play or waiting: the Video node's own controls.
    hideSkip()
    if state = "paused"
        m.paused = true
        report("paused")
    else if state = "finished"
        ended()
    else if state = "error"
        broken()
    end if
end sub

sub onPosition(event as object)
    seconds = event.getData()
    if seconds = invalid then return
    m.positionMs = Int(seconds * 1000)
    if playerShouldReport(m.video.state, m.clock.TotalMilliseconds() - m.lastReportAt) then report("playing")
    if m.next <> invalid and not m.upNextShown and m.upNextAt > 0 and m.positionMs >= m.upNextAt then showUpNext()
    updateSkip()
end sub

' Skip Intro or Skip Credits while the playhead is in one and the video
' plays.
sub updateSkip()
    marker = playerSkipFor(m.markers, m.positionMs, m.next <> invalid and not m.upNextCancelled)
    key = ""
    if marker <> invalid then key = marker.key
    if key <> m.skipKey
        m.skipKey = key
        m.skipDismissed = false
        if marker <> invalid then m.skip.text = playerSkipLabel(marker)
    end if
    m.skipMarker = marker
    shown = marker <> invalid and not m.skipDismissed and m.video.state = "playing" and not m.upNext.visible and not m.notice.visible
    if shown = m.skip.visible then return
    if shown
        m.skip.visible = true
        m.skip.setFocus(true)
    else
        hideSkip()
    end if
end sub

sub hideSkip()
    if m.skip.hasFocus() then m.video.setFocus(true)
    m.skip.visible = false
end sub

sub onSkip()
    marker = m.skipMarker
    if marker = invalid then return
    ' Until the playhead is past it, the button stays away.
    m.skipDismissed = true
    hideSkip()
    target = playerSkipTarget(marker, m.durationMs)
    if target < 0
        ended()
        return
    end if
    seekTo(target)
end sub

' Play, fast forward or rewind while the skip button has the focus.
sub onSkipKey(event as object)
    hideSkip()
    playerKey(event.getData())
end sub

' What play, fast forward and rewind do when the skip button or Up Next
' has the focus instead of the video: pause or go on, or 10 s further or
' back.
sub playerKey(key as string)
    if key = "play"
        if m.video.state = "paused"
            m.video.control = "resume"
        else
            m.video.control = "pause"
        end if
    else if key = "fastforward"
        seekTo(m.positionMs + 10000)
    else
        seekTo(m.positionMs - 10000)
    end if
end sub

sub seekTo(targetMs as integer)
    target = targetMs
    last = Int(m.durationMs) - 1000
    if last > 0 and target > last then target = last
    if target < 0 then target = 0
    m.positionMs = target
    m.video.seek = target / 1000
end sub

' POST /api/items/{id}/progress for the current session.
sub report(state as string)
    if m.sessionId = "" then return
    m.lastReportAt = m.clock.TotalMilliseconds()
    path = "/api/items/" + urlValue(m.itemId) + "/progress"
    uiRequest({ method: "POST", path: path, json: playerProgress(m.positionMs, m.durationMs, state, m.sessionId), session: m.sessionId }, "onReported")
end sub

sub onReported(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId then return
    answer = event.getData()
    if answer.status = 410 then stopped(answer.data)
end sub

' DELETE /api/playback/{session}: the server stops any conversion.
sub endSession()
    closeSession(m.sessionId)
    m.sessionId = ""
end sub

sub closeSession(sessionId as string)
    if sessionId = "" then return
    uiRequest({ method: "DELETE", path: "/api/playback/" + urlEncode(sessionId) }, "")
end sub

sub ended()
    if m.durationMs > 0 then m.positionMs = m.durationMs
    report("stopped")
    endSession()
    if m.next <> invalid and not m.upNextCancelled
        playNext()
    else
        uiNavigate({ back: true })
    end if
end sub

' Leaving the player: the place is kept and the session closed (or, still
' to come, closed when it does).
sub finish()
    abandonPlayback()
    m.attempt = m.attempt + 1
    m.countdownTimer.control = "stop"
    if m.sessionId <> ""
        report("stopped")
        endSession()
    end if
    m.video.control = "stop"
    captionsBack()
    if m.started then m.global.session.unobserveFieldScoped("accessToken")
    ' Where titles were left changed: screens that show it ask again.
    scene = m.top.getScene()
    scene.played = scene.played + 1
end sub

' The server stopped the stream: the player leaves, and the scene says why
' over the screen before.
sub stopped(data as dynamic)
    m.sessionId = ""
    m.video.control = "stop"
    lines = playerStoppedMessage(data).Split(Chr(10))
    message = []
    if lines.Count() > 1 then message.Push(lines[1])
    uiNavigate({ back: true, message: { title: lines[0], lines: message } })
end sub

sub broken()
    if playerErrorKind(m.video.errorCode, m.video.errorInfo, m.played) = "media"
        ' The original doesn't play on this Roku after all: converted, once.
        if m.info <> invalid and httpString(m.info.method) <> "transcode" and m.mode <> "transcode"
            open(m.itemId, m.positionMs, m.item, "transcode")
            return
        end if
        endSession()
        text = "This video can't be played on this Roku."
        if m.info <> invalid
            reason = httpSentence(m.info.reason)
            if reason <> "" then text = text + " " + reason
        end if
        notice("Can't play this item", text, ["Back"])
        return
    end if
    if m.sessionId = ""
        notice("Can't play this item", "Playback stopped and couldn't get going again.", ["Try again", "Back"])
        return
    end if
    ' What became of the session says what to do.
    m.loading.visible = true
    uiRequest({ method: "GET", path: "/api/playback/" + urlEncode(m.sessionId), session: m.sessionId }, "onSessionState")
end sub

sub onSessionState(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId then return
    answer = event.getData()
    meaning = playerSessionState(answer.status)
    if meaning = "stopped"
        stopped(answer.data)
    else if meaning = "unreachable"
        uiUnreachable({ to: "player", params: { itemId: m.itemId, startMs: m.positionMs } })
    else if meaning <> "signed_out"
        restart()
    end if
end sub

' The session lives on (the token ran out) or expired: a new one from the
' same place, with the current token. The server ends the old one itself
' (same client_id).
sub restart()
    now = m.clock.TotalMilliseconds()
    if not playerMayRestart(m.restarts, now)
        notice("Can't play this item", "Playback stopped and couldn't get going again.", ["Try again", "Back"])
        return
    end if
    m.restarts.Push(now)
    open(m.itemId, m.positionMs, m.item, m.mode)
end sub

sub notice(title as string, text as string, buttons as object)
    m.loading.visible = false
    hideSkip()
    hideUpNext()
    m.noticeTitle.text = title
    m.noticeText.text = text
    m.noticeButtons.buttons = buttons
    m.notice.visible = true
    m.noticeButtons.setFocus(true)
end sub

sub onNoticeButton(event as object)
    if m.noticeButtons.buttons[event.getData()] = "Try again"
        m.restarts = []
        open(m.itemId, m.positionMs, m.item, m.mode)
    else
        uiNavigate({ back: true })
    end if
end sub

sub onNext(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if not answer.ok or answer.data = invalid then return
    item = httpObject(answer.data.item)
    if item <> invalid then m.next = contentUpNext(item, m.global.session.server)
end sub

sub showUpNext()
    hideSkip()
    m.upNextShown = true
    m.nextTitle.text = m.next.title
    m.nextLine.text = m.next.line
    m.nextImage.setHttpAgent(m.top.getScene().getHttpAgent())
    m.nextImage.uri = m.next.image
    m.countdownLeft = playerUpNextSeconds()
    m.countdown.text = "Next in " + m.countdownLeft.ToStr() + " s"
    m.upNext.visible = true
    m.nextButtons.focusButton = 0
    m.nextButtons.setFocus(true)
    m.countdownTimer.control = "start"
end sub

sub hideUpNext()
    m.countdownTimer.control = "stop"
    m.upNext.visible = false
end sub

sub onCountdown()
    m.countdownLeft = m.countdownLeft - 1
    if m.countdownLeft <= 0
        playNext()
    else
        m.countdown.text = "Next in " + m.countdownLeft.ToStr() + " s"
    end if
end sub

sub onNextButton(event as object)
    if event.getData() = 0
        playNext()
    else
        cancelUpNext()
    end if
end sub

' The rest plays out, credits with their Skip Credits button.
sub cancelUpNext()
    m.upNextCancelled = true
    hideUpNext()
    m.video.setFocus(true)
    updateSkip()
end sub

' Up Next shows at the credits: moving on counts the episode as watched.
sub playNext()
    nextEpisode = m.next
    hideUpNext()
    if nextEpisode = invalid then return
    if m.sessionId <> ""
        if m.durationMs > 0 then m.positionMs = m.durationMs
        report("stopped")
        endSession()
    end if
    m.video.control = "stop"
    m.restarts = []
    open(nextEpisode.itemId, nextEpisode.startMs, invalid, "auto")
end sub

' While Up Next counts down: Back cancels it, and play, fast forward and
' rewind cancel it and act on the video. Otherwise Back leaves the player
' (the scene), which closes the session.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press or not m.upNext.visible then return false
    if key = "back"
        cancelUpNext()
        return true
    end if
    if key = "play" or key = "fastforward" or key = "rewind" or key = "replay"
        cancelUpNext()
        playerKey(key)
        return true
    end if
    return false
end function
