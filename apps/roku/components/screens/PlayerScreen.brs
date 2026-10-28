' The player (docs/API.md, "Playback"). Progress goes to the server about
' every 10 s while playing, on pause, and as "stopped" on the way out, then
' the session is closed. A stream that breaks is checked with the server:
' stopped by an administrator (410), the player leaves and says so;
' otherwise it starts again where it was, with a new session and the
' current token (a token that ran out answers 401 to the Video node).
' Skip Intro and Skip Credits follow the web player: an episode's credits
' bring up Up Next instead. Down brings up subtitle sync for a file with
' text subtitles (SubtitleSync.brs). Subtitles (or audio) the person picks
' in Roku's options are remembered on the server for the show or the movie
' (Tracks.brs), so the next episode starts with matching ones.

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
    m.sync = m.top.findNode("sync")
    m.syncValue = m.top.findNode("syncValue")
    m.syncButtons = m.top.findNode("syncButtons")
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
    ' Subtitle sync: the file's offset, under what it is remembered, the
    ' offset when the panel opened, and how many text tracks there are.
    m.syncMs = 0
    m.syncKey = ""
    m.syncWas = 0
    m.textTracks = 0
    ' The tracks shown (stream indexes; -1 none, -2 not known yet), the
    ' audio played, and from when (clock ms) a change is the person's: -1
    ' until the stream played.
    m.subtitleShown = -1
    m.audioShown = -2
    m.audioPlaying = -1
    m.tracksFrom = -1
    m.syncButtons.buttons = ["Done", "Reset"]
    m.syncButtons.observeField("buttonSelected", "onSyncButton")
    m.video.notificationInterval = 1
    m.video.observeField("state", "onState")
    m.video.observeField("position", "onPosition")
    for each field in ["currentSubtitleTrack", "globalCaptionMode", "currentAudioTrack", "availableAudioTracks"]
        m.video.observeField(field, "onTracks")
    end for
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
    m.textTracks = 0
    m.tracksFrom = -1
    m.sync.visible = false
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
    ' The sync remembered for this file moves every text track.
    m.syncKey = syncKey(m.global.session.server, m.info.file_id)
    m.syncMs = syncOffset(syncSection(), m.syncKey)
    stream = contentStream(m.info, m.item, m.global.session.server, m.startMs)
    m.textTracks = stream.subtitleTracks.Count()
    ' What pley shows: what the Roku shows until the stream played.
    m.subtitleShown = tracksSubtitleShown("On", stream.subtitle, m.info.subtitles, m.global.session.server)
    m.audioShown = -2
    m.audioPlaying = tracksAudioPlaying(m.info, contentFile(m.item, m.info.file_id))
    m.tracksFrom = -1
    showStream(stream, syncTrackUrl(stream.subtitle, m.syncMs))
    if stream.subtitle <> ""
        captionsOn()
    else
        captionsBack()
    end if
    m.video.control = "play"
    m.video.setFocus(true)
    m.lastReportAt = m.clock.TotalMilliseconds()
end sub

' Gives the Video node stream, its text tracks moved as the sync says,
' showing the track subtitle ("" leaves it to the Roku).
sub showStream(stream as object, subtitle as string)
    content = CreateObject("roSGNode", "ContentNode")
    content.url = stream.url
    content.streamFormat = stream.streamFormat
    content.title = stream.title
    content.playStart = stream.playStart
    content.subtitleTracks = syncTracks(stream.subtitleTracks, m.syncMs)
    if LCase(Left(stream.url, 8)) = "https://" then nodesFill(content, { HttpCertificatesFile: "common:/certs/ca-bundle.crt" })
    m.video.setHttpAgent(uiHttpAgent())
    m.video.content = content
    if subtitle <> "" then m.video.subtitleTrack = subtitle
end sub

' Where subtitle sync is remembered on this Roku.
function syncSection() as object
    return CreateObject("roRegistrySection", "subtitleSync")
end function

' Subtitle sync is offered while a file with text subtitles plays.
function syncOffered() as boolean
    return m.info <> invalid and m.textTracks > 0 and not m.notice.visible and not m.upNext.visible
end function

sub openSync()
    hideSkip()
    m.syncWas = m.syncMs
    m.syncValue.text = syncLabel(m.syncMs)
    m.sync.visible = true
    m.syncButtons.focusButton = 0
    m.syncButtons.setFocus(true)
end sub

sub stepSync(byMs as integer)
    m.syncMs = syncClamp(m.syncMs + byMs)
    m.syncValue.text = syncLabel(m.syncMs)
end sub

sub onSyncButton(event as object)
    if m.syncButtons.buttons[event.getData()] = "Reset"
        stepSync(-m.syncMs)
    else
        closeSync()
    end if
end sub

' Done or Back: a new offset is remembered for the file, and the stream
' opens again where it was, its text tracks moved by the server.
sub closeSync()
    if not m.sync.visible then return
    m.sync.visible = false
    m.video.setFocus(true)
    if m.syncMs = m.syncWas then return
    syncRemember(syncSection(), m.syncKey, m.syncMs)
    reopen()
end sub

' The stream again from where it is: the same session and address, the
' subtitles that showed shown again, moved.
sub reopen()
    if m.item = invalid or m.info = invalid then return
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    showing = syncBaseUrl(httpString(m.video.currentSubtitleTrack))
    if showing = "" then showing = syncBaseUrl(httpString(m.video.subtitleTrack))
    m.positionMs = position
    m.loading.visible = true
    ' The tracks as the stream opens again are pley's doing.
    m.tracksFrom = -1
    showStream(contentStream(m.info, m.item, m.global.session.server, position), syncTrackUrl(showing, m.syncMs))
    m.video.control = "play"
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

' Captions turned on or off, or another subtitle or audio track, in Roku's
' options: once the stream played, the person's pick, remembered on the
' server for the show or the movie. Before, what shows is pley's choice or
' the Roku's own, and only noted.
sub onTracks()
    if m.info = invalid or m.item = invalid then return
    file = contentFile(m.item, m.info.file_id)
    subtitle = tracksSubtitleShown(httpString(m.video.globalCaptionMode), httpString(m.video.currentSubtitleTrack), m.info.subtitles, m.global.session.server)
    ' A converted stream has one audio track; a file played as it is, all.
    audio = -2
    if not httpTrue(m.info.hls) then audio = tracksAudioShown(m.video.availableAudioTracks, tracksTrackId(m.video.currentAudioTrack), file)
    subtitlePicked = subtitle <> -2 and subtitle <> m.subtitleShown
    if subtitlePicked then m.subtitleShown = subtitle
    ' The first audio the Roku tells of is the one it started with.
    audioPicked = audio <> -2 and m.audioShown <> -2 and audio <> m.audioShown
    if audio <> -2
        m.audioShown = audio
        m.audioPlaying = audio
    end if
    if not subtitlePicked and not audioPicked then return
    if m.tracksFrom < 0 or m.clock.TotalMilliseconds() < m.tracksFrom then return
    saveTracks(file, subtitlePicked, audioPicked)
end sub

' PUT /api/items/{id}/streams with the tracks playing; kept in the file too,
' so a stream started again shows them. Text subtitles picked over picture
' ones the server draws need a stream without those.
sub saveTracks(file as dynamic, subtitlePicked as boolean, audioPicked as boolean)
    burned = -1
    if httpTrue(m.info.burn_subtitles) then burned = Int(httpNumber(m.info.subtitle_index, -1))
    subtitle = m.subtitleShown
    if not subtitlePicked and subtitle < 0 then subtitle = burned
    ' The audio only when it was picked: subtitles alone keep the audio picked before.
    audio = -2
    if audioPicked then audio = m.audioPlaying
    body = tracksStreamsBody(m.info.file_id, audio, subtitle)
    uiRequest({ method: "PUT", path: "/api/items/" + urlValue(m.itemId) + "/streams", jsonText: FormatJson(body) }, "")
    if file <> invalid
        if body.DoesExist("audio_index") then file.selected_audio = body.audio_index
        file.selected_subtitle = body.subtitle_index
    end if
    if not subtitlePicked or not tracksNeedNewSession(burned, subtitle) then return
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    open(m.itemId, position, m.item, m.mode)
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
        if m.tracksFrom < 0 then m.tracksFrom = m.clock.TotalMilliseconds() + tracksSettleMs()
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
    shown = marker <> invalid and not m.skipDismissed and m.video.state = "playing" and not m.upNext.visible and not m.notice.visible and not m.sync.visible
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
    m.tracksFrom = -1
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
    m.tracksFrom = -1
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
    closeSync()
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

' While subtitle sync shows: left and right (rewind and fast forward too)
' move the subtitles, Back closes it. While Up Next counts down: Back
' cancels it, and play, fast forward and rewind cancel it and act on the
' video. Otherwise Down brings up subtitle sync, and Back leaves the player
' (the scene), which closes the session.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.sync.visible
        if key = "left" or key = "rewind"
            stepSync(-syncStepMs())
            return true
        end if
        if key = "right" or key = "fastforward"
            stepSync(syncStepMs())
            return true
        end if
        if key = "back"
            closeSync()
            return true
        end if
        return false
    end if
    if m.upNext.visible
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
    end if
    if key = "down" and syncOffered()
        openSync()
        return true
    end if
    return false
end function
