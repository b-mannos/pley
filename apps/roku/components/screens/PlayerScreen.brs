' The player (docs/API.md, "Playback"). Progress goes to the server about
' every 10 s while playing, on pause and every 30 s while paused, and as
' "stopped" on the way out, then the session is closed. A stream that
' breaks is checked with the server: stopped by an administrator (410),
' the player says so, then leaves; otherwise it starts again where it was,
' with a new session and the current token (a token that ran out answers
' 401 to the Video node). Skip Intro and Skip Credits follow the web
' player: an episode's credits bring up Up Next instead. Down brings up
' Pley's menu: the file's subtitles and audio (every track, pictures too),
' Quality, subtitle sync for a file with text subtitles or pictures drawn
' here (SubtitleSync.brs), the file's chapters and the previous and next
' ones, the next episode, the sleep timer (Sleep.brs) and the playback's
' details (Details.brs). Subtitles (or audio) the person picks in Roku's
' options are remembered on the server for the show or the movie
' (Tracks.brs), so the next episode starts with matching ones; the user's
' remote, on another device, picks them as the menu does. The quality is
' the one chosen in Settings or the menu (the remote chooses it as the
' menu does); when it doesn't play well, Automatic lowers a conversion's
' bitrate as it is sent, or opens the stream again lower where it was once
' it stopped anyway (AutoQuality.brs). PGS subtitles are drawn over the
' video from the server's pictures (Pictures.brs), so the file plays as it
' is; the server burns them in only where it can't make those. A video on
' a USB drive (params.local) plays with no server at all (openLocal).

sub init()
    m.video = m.top.findNode("video")
    m.skip = m.top.findNode("skip")
    ' Waiting for the video (the spinner shows 500 ms after it began, as
    ' Android TV's ring), and "Reconnecting…" under it.
    m.spinner = m.top.findNode("spinner")
    m.spinner.poster.uri = "pkg:/images/spinner.png"
    m.spinner.poster.width = 64
    m.spinner.poster.height = 64
    m.reconnecting = m.top.findNode("reconnecting")
    m.waiting = false
    m.spinnerTimer = m.top.findNode("spinnerTimer")
    m.spinnerTimer.observeField("fire", "onSpinnerTimer")
    m.upNext = m.top.findNode("upNext")
    m.nextBack = m.top.findNode("nextBack")
    m.nextImage = m.top.findNode("nextImage")
    m.nextHeading = m.top.findNode("nextHeading")
    m.nextTitle = m.top.findNode("nextTitle")
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
    m.menu = m.top.findNode("menu")
    m.menuTitle = m.top.findNode("menuTitle")
    m.menuList = m.top.findNode("menuList")
    m.menuChoices = m.top.findNode("menuChoices")
    m.menuText = m.top.findNode("menuText")
    m.menuHint = m.top.findNode("menuHint")
    ' What the menu shows ("menu", "quality", "chapters", "details"), the
    ' actions of its entries, and the values of a list of choices.
    m.menuKind = ""
    m.menuActions = []
    m.menuValues = []
    ' Find subtitles…: the language looked in ("": the person's subtitle
    ' language), and the results listed ([{ id, label, hint }]) after the
    ' Language entry.
    m.findLanguage = ""
    m.found = []
    m.menuList.observeField("itemSelected", "onMenuEntry")
    m.menuList.observeField("itemFocused", "onMenuEntryFocused")
    m.menuChoices.observeField("itemSelected", "onMenuChoice")
    m.menuChoices.observeField("itemFocused", "onMenuChoiceFocused")
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
    ' Whether the stream opening was asked for by the user's remote (a
    ' track or a quality picked, a cast naming tracks), which hears of it as
    ' it plays.
    m.reportAtPlay = false
    ' The audio a cast named for its title (-1: the server's pick by the
    ' user's choices); its subtitles are in the item's file
    ' (Cast.brs's castGivenTracks).
    m.playAudio = -1
    ' Whether this opening's video has played yet (errors before are the file's).
    m.played = false
    ' What plays next (Content.brs's contentUpNext) and Up Next's card for
    ' it (Player.brs's playerUpNextCard).
    m.next = invalid
    m.nextCard = invalid
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
    ' The offset the stream's text tracks were opened with.
    m.streamSyncMs = 0
    m.textTracks = 0
    ' The tracks shown (stream indexes; -1 none, -2 not known yet), the
    ' audio played, and from when (clock ms) a change is the person's: -1
    ' until the stream played.
    m.subtitleShown = -1
    m.audioShown = -2
    m.audioPlaying = -1
    m.tracksFrom = -1
    ' Automatic (docs/QUALITY.md): how this opening was asked for
    ' (aqAsk), the title's watch, the quality Automatic opens the title
    ' at again ("" for the setting's choice), and what to say once it plays.
    m.asked = invalid
    m.watch = invalid
    m.watchItem = invalid
    m.autoQuality = ""
    m.autoNotice = ""
    m.autoTimer = m.top.findNode("autoTimer")
    m.autoTimer.observeField("fire", "onAutoTick")
    ' The sleep timer, off until set in the menu, for the whole time the
    ' player shows; when it paused the video (clock ms), -1 otherwise.
    m.sleep = sleepNew(0, 0#)
    m.sleepPausedAt = -1#
    m.sleepNotice = m.top.findNode("sleepNotice")
    m.sleepText = m.top.findNode("sleepText")
    m.sleepButtons = m.top.findNode("sleepButtons")
    m.sleepButtons.buttons = ["Keep watching"]
    m.sleepButtons.observeField("buttonSelected", "onKeepWatching")
    m.sleepTimer = m.top.findNode("sleepTimer")
    m.sleepTimer.observeField("fire", "onSleepTick")
    m.pausedTimer = m.top.findNode("pausedTimer")
    m.pausedTimer.duration = playerPausedReportEveryMs() / 1000
    m.pausedTimer.observeField("fire", "onPausedTick")
    m.video.observeField("downloadedSegment", "onSegment")
    ' Picture subtitles drawn here: the track (-1 none), its pictures known
    ' (Pictures.brs), where the video's picture shows (invalid when the
    ' file doesn't say its size), the folder of their files after the
    ' server, the cue showing (-1 none), the frames (the one at front shows,
    ' the other loads the next), which answer is awaited and whether a seek
    ' since wants another at once, the last position seen (and when) to
    ' tell a seek by, and the pictures that failed to load (and when). Once
    ' the server turns out not to make them, they are burned in for as long
    ' as the player shows.
    m.picturesOff = false
    m.pictureIndex = -1
    m.pictures = invalid
    m.picturePicture = invalid
    m.pictureFolder = ""
    m.pictureShown = -1
    m.pictureFrames = [m.top.findNode("pictureA"), m.top.findNode("pictureB")]
    m.pictureFront = 0
    for each frame in m.pictureFrames
        frame.setHttpAgent(uiHttpAgent(false))
        frame.observeField("loadStatus", "onPictureLoad")
    end for
    m.pictureFailed = {}
    ' Captions as last seen: the person turning them off turns off pictures
    ' drawn here.
    m.captionModeSeen = httpString(m.video.globalCaptionMode)
    m.pictureAsk = 0
    m.pictureWaiting = false
    m.pictureAgain = false
    m.pictureLastMs = -1
    m.pictureLastAt = 0
    m.picturesTimer = m.top.findNode("picturesTimer")
    m.picturesTimer.duration = picAgainMs() / 1000
    m.picturesTimer.observeField("fire", "askPictures")
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
    ' The title at the top as each item begins, once it plays (bannerUp),
    ' and while paused or Roku's bar shows (barShown), with how it plays,
    ' the clock on this Roku's 12- or 24-hour clock, and when it ends.
    m.banner = m.top.findNode("banner")
    m.bannerTitle = m.top.findNode("bannerTitle")
    m.bannerLine = m.top.findNode("bannerLine")
    m.bannerRight = m.top.findNode("bannerRight")
    m.bannerStatus = m.top.findNode("bannerStatus")
    m.bannerEnds = m.top.findNode("bannerEnds")
    m.bannerDue = false
    m.bannerUp = false
    m.barShown = false
    m.clockTicking = false
    m.twelve = CreateObject("roDeviceInfo").GetClockFormat() <> "24h"
    m.bannerTimer = m.top.findNode("bannerTimer")
    m.bannerTimer.duration = playerBannerMs() / 1000
    m.bannerTimer.observeField("fire", "hideBanner")
    m.clockTimer = m.top.findNode("clockTimer")
    m.clockTimer.observeField("fire", "updateBanner")
    ' Settings → Playback details: the readout in the title's place, read
    ' as the player opens.
    m.showDetails = false
    m.readout = m.top.findNode("readout")
    m.readoutBack = m.top.findNode("readoutBack")
    m.readoutText = m.top.findNode("readoutText")
    ' Whether Roku's bar shows, where the Video node says so; elsewhere
    ' the controls are up while paused alone.
    m.video.observeField("trickPlayBarVisibilityHint", "onBar")
    ' The server out of reach: since when (clock ms), -1 while it answers.
    ' Until something played, that is said at once.
    m.reconnectFrom = -1
    m.everPlayed = false
    m.reconnectTimer = m.top.findNode("reconnectTimer")
    m.reconnectTimer.duration = playerReconnectEveryMs() / 1000
    m.reconnectTimer.observeField("fire", "onReconnectTick")
    ' The playhead standing still: where, and since when (clock ms); and
    ' whether this opening said the server can't keep up.
    m.stillAt = -1
    m.stillSince = 0
    m.slowSaid = false
    m.stallTimer = m.top.findNode("stallTimer")
    m.stallTimer.observeField("fire", "onStallTick")
    ' The last position read, and when, to tell a seek by.
    m.seenMs = -1
    m.seenAt = 0
    ' The media ended: Up Next waits at its end. How often this title's
    ' stream was opened again for falling short of its end.
    m.ended = false
    m.shortTries = 0
    ' Why the player converted this title's file by itself (Player.brs's
    ' playerFallbackNote: "unplayable", "struggled", "" for none), whether
    ' it did so once already for its not moving, and what to say once the
    ' conversion plays.
    m.fallback = ""
    m.struggled = false
    m.playNotice = ""
    ' The menu's Playback details: their lines, the first shown (Up and
    ' Down scroll them), and whether they wait for the server's first
    ' answer.
    m.detailLines = []
    m.detailTop = 0
    m.detailsWaiting = false
    m.detailsTimer = m.top.findNode("detailsTimer")
    m.detailsTimer.observeField("fire", "onDetailsTick")
    m.stoppedTimer = m.top.findNode("stoppedTimer")
    m.stoppedTimer.duration = playerStoppedLeaveMs() / 1000
    m.stoppedTimer.observeField("fire", "onStoppedTimer")
    ' A video on a USB drive (params.local: { path, title }; invalid for
    ' the server's): the subtitle files beside it ([{ path, label }]), the
    ' one drawn (-1 none) and its cues (invalid until they come), when it
    ' was opened (clock ms), and the cue drawn over the video.
    m.local = invalid
    m.captionSet = ""
    m.localSidecars = []
    m.localSubtitle = -1
    m.localCues = invalid
    m.localFrom = 0
    m.localKeptAt = 0
    m.localKeptMs = -1
    m.cue = m.top.findNode("cue")
    m.cueText = m.top.findNode("cueText")
    m.cueShadow = m.top.findNode("cueShadow")
    m.top.observeField("active", "onActive")
    m.top.observeField("serverStopped", "onServerStopped")
    m.top.observeField("playerCommand", "onPlayerCommand")
end sub

' Paused, the progress goes on being told, so the session stays the
' player's.
sub onPausedTick()
    if m.video.state = "paused" then report("paused")
end sub

' This playback's session stopped from elsewhere: it ends now, not at its
' next report.
sub onServerStopped(event as object)
    data = event.getData()
    if m.sessionId <> "" and httpString(data.session_id) = m.sessionId then stopped(data)
end sub

' A command of the user's remote, on another device, for this playback's
' session (Remote.brs has the rule). Only a video playing or paused obeys:
' not one still starting, at its end, stopped, or saying what went wrong.
' A play or pause is reported as the video's own are, when its state
' changes; a seek at once; the next item with its own session; audio and
' subtitles as Pley's menu picks them (obeyTrack), and the quality too
' (obeyQuality).
sub onPlayerCommand(event as object)
    state = m.video.state
    if m.sessionId = "" or not m.top.active or not m.played or m.ended or m.notice.visible then return
    if state <> "playing" and state <> "paused" and state <> "buffering" then return
    command = event.getData()
    what = httpString(command.command)
    if what = "audio" or what = "subtitles"
        obeyTrack(command)
        return
    end if
    if what = "quality"
        obeyQuality(command)
        return
    end if
    duration = invalid
    if m.durationMs > 0 then duration = m.durationMs
    player = { kind: "video", session: m.sessionId, playing: state <> "paused", positionMs: m.positionMs, durationMs: duration, hasNext: m.next <> invalid }
    action = remoteAction(player, command)
    if action.act = "play"
        m.video.control = "resume"
    else if action.act = "pause"
        m.video.control = "pause"
    else if action.act = "report"
        ' As it is already: said again, so a remote a little behind settles.
        report(reportedState())
    else if action.act = "seek"
        ' As the rewind key does: left up, Up Next's countdown would play
        ' the next episode over the place the remote asked for.
        if m.upNext.visible then cancelUpNext()
        seekTo(action.positionMs)
        ' Told now, not as the next reading finds the playhead moved.
        m.seenMs = m.positionMs
        m.seenAt = m.clock.TotalMilliseconds()
        report(reportedState())
    else if action.act = "next"
        playNext(false)
    end if
end sub

' The remote's audio or subtitles (Remote.brs's remoteTrackAction), on the
' path of Pley's menu: saved for the show or the movie, and the stream
' opened again with it, which tells the server at once as it plays. The
' track playing already is said again at once.
sub obeyTrack(command as object)
    tracks = tracksIndexes(playingFile())
    player = { session: m.sessionId, audio: tracks.audio, subtitles: tracks.subtitles, audioIndex: m.audioPlaying, subtitleIndex: subtitlePlaying() }
    action = remoteTrackAction(player, command)
    if action.act = "none"
        if action.report then report(reportedState())
        return
    end if
    m.reportAtPlay = true
    pickTrack(action.act, action.index)
end sub

' The remote's quality (Remote.brs's remoteQualityAction), as a pick in
' Pley's Quality menu: kept as the Roku's, and the title opened again at
' it where it was, which tells the server at once as it plays. The quality
' picked already is said again at once.
sub obeyQuality(command as object)
    player = { session: m.sessionId, qualities: aqMenuIds(playingFile()), quality: qualityPicked() }
    action = remoteQualityAction(player, command)
    if action.act = "none"
        if action.report then report(reportedState())
        return
    end if
    m.reportAtPlay = true
    chooseQuality(action.quality)
end sub

' The Quality menu's pick for the file playing, as it ticks it and as the
' reports say it.
function qualityPicked() as string
    return aqPicked(aqChoice(playbackSection()), playingFile())
end function

' The subtitles showing, as the reports and the remote name them: those
' burned in, else those shown (Tracks.brs).
function subtitlePlaying() as integer
    burned = -1
    if m.info <> invalid and httpTrue(m.info.burn_subtitles) then burned = Int(httpNumber(m.info.subtitle_index, -1))
    return tracksSubtitlePlaying(m.subtitleShown, burned)
end function

' What the video is doing, as progress tells it.
function reportedState() as string
    if m.video.state = "paused" then return "paused"
    return "playing"
end function

sub onActive()
    if not m.top.active
        ' Leaving: Back, or the sign-in ended.
        finish()
        return
    end if
    if m.started then return
    m.started = true
    m.showDetails = playerShowsDetails(playbackSection())
    ' A video takes over from the music, which pauses and keeps its queue.
    m.global.music.control = "suspend"
    ' Playlists and segments carry the token: a refreshed one goes to the
    ' Video node's agent at once.
    m.global.session.observeFieldScoped("accessToken", "onToken")
    params = m.top.params
    ' A playlist's entries, this one first: Up Next brings on each next.
    m.queue = invalid
    m.queueAt = 0
    if GetInterface(params.queue, "ifArray") <> invalid then m.queue = params.queue
    m.playAudio = Int(httpNumber(params.audioIndex, -1))
    ' A cast's tracks: the remote hears of them as soon as they play.
    m.reportAtPlay = httpTrue(params.tracksGiven)
    newTitle()
    m.bannerDue = true
    m.shortTries = 0
    m.fallback = ""
    m.struggled = false
    m.local = httpObject(params.local)
    if m.local <> invalid
        openLocal(Int(httpNumber(params.startMs, 0)))
        return
    end if
    open(params.itemId, Int(httpNumber(params.startMs, 0)), params.item, "auto")
end sub

' --- A video on a USB drive ---

' Plays the video at m.local.path from startMs as it is, with no server
' (docs/ROKU.md, "Videos on a USB drive"): no session, conversion,
' quality, seek previews, Find subtitles…, saved track choices or next
' episode. The subtitle files beside it are drawn here (Drive.brs's cues),
' moved by subtitle sync as the server's are, and where it stopped is kept
' on this Roku by its path.
sub openLocal(startMs as integer)
    path = httpString(m.local.path)
    m.attempt = m.attempt + 1
    ' The sleep timer's and the banner's: the video by its path, its title.
    m.itemId = path
    m.item = { kind: "video", title: httpString(m.local.title) }
    m.info = invalid
    m.sessionId = ""
    m.startMs = startMs
    m.positionMs = startMs
    m.durationMs = 0
    m.played = false
    m.ended = false
    m.next = invalid
    m.markers = []
    m.localSidecars = []
    m.localSubtitle = -1
    m.localCues = invalid
    ' Its subtitle sync is kept by its path, as its place is.
    m.syncKey = "drive|" + driveResumeKey(path)
    m.syncMs = syncOffset(syncSection(), m.syncKey)
    styleCue()
    content = CreateObject("roSGNode", "ContentNode")
    content.url = path
    content.streamFormat = driveStreamFormat(path)
    content.title = m.item.title
    content.playStart = Int(startMs / 1000)
    m.video.content = content
    m.video.control = "play"
    m.video.setFocus(true)
    m.loading.visible = true
    m.localFrom = m.clock.TotalMilliseconds()
    m.localKeptAt = m.localFrom
    m.localKeptMs = -1
    m.stillAt = startMs
    m.stillSince = m.localFrom
    m.stallTimer.control = "start"
    uiDrive({ op: "sidecars", path: path, attempt: m.attempt }, "onSidecars")
end sub

sub onSidecars(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if answer.ok then m.localSidecars = httpArray(answer.data.sidecars)
end sub

' Where the video stopped, kept on this Roku as it pauses and as the
' player closes; past 95 % it was watched, and is forgotten. A video that
' never played keeps what it had.
sub keepLocal()
    if m.local = invalid or not m.played then return
    position = m.positionMs
    seconds = m.video.position
    if not m.ended and seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    driveRemember(CreateObject("roRegistrySection", "driveResume"), httpString(m.local.path), position, m.durationMs)
end sub

' A drive's video tells no server: its place is kept on the Roku as the
' server's reports go (Android TV keeps it with each), but while it plays
' only once a minute, and only when it moved, as the registry is the
' Roku's flash.
sub reportLocal(state as string)
    now = m.clock.TotalMilliseconds()
    if state = "playing" and now - m.localKeptAt < 60000 then return
    m.lastReportAt = now
    if m.positionMs = m.localKeptMs then return
    m.localKeptAt = now
    m.localKeptMs = m.positionMs
    keepLocal()
end sub

' Once a second: a picture that hasn't started in 12 s, or a playhead that
' stood still 12 s once it played (the drive pulled out), says so, as on
' Android TV and LG TVs.
sub localStallTick(now as integer, state as string)
    if m.notice.visible or m.ended then return
    if not m.played
        if now - m.localFrom >= playerStallMs() then localFailed("This video didn't start.")
        return
    end if
    meant = state = "playing" or state = "buffering"
    if not meant then m.stillSince = now
    if not meant or now - m.stillSince < playerStallMs() then return
    localFailed("This video stopped and didn't go on. The drive may have been pulled out.")
end sub

sub localFailed(text as string)
    keepLocal()
    m.video.control = "stop"
    hideCue()
    notice("Can't play this video", text, ["Back"])
end sub

' A pick in Pley's menu (driveSubtitleChoices' value, or an audio track's
' index). A subtitle file beside the video is read by the drive task and
' drawn, Roku's captions off meanwhile so that the file's own track doesn't
' show over it; the file's own track shows as the Roku draws it, captions
' on; Off turns captions off. Captions go back as they were when the player
' closes. An audio track is the Video node's.
sub pickLocal(kind as string, value as dynamic)
    if kind = "audio"
        tracks = httpArray(m.video.availableAudioTracks)
        if value >= 0 and value < tracks.Count() then m.video.audioTrack = httpString(tracks[value].Track)
        return
    end if
    m.localSubtitle = -1
    m.localCues = invalid
    hideCue()
    m.video.notificationInterval = 1
    if Left(value, 6) = "track:"
        captionsTo("On")
        m.video.subtitleTrack = Mid(value, 7)
        return
    end if
    index = -1
    if Left(value, 5) = "file:" then index = Val(Mid(value, 6))
    if index < 0 or index >= m.localSidecars.Count()
        captionsTo("Off")
        return
    end if
    m.localSubtitle = index
    captionsTo("Off")
    uiDrive({ op: "cues", path: m.localSidecars[index].path, at: index, attempt: m.attempt }, "onCues")
end sub

sub onCues(event as object)
    if not isCurrent(event) or event.getRoSGNode().spec.at <> m.localSubtitle then return
    answer = event.getData()
    if not answer.ok
        m.localSubtitle = -1
        uiToast(httpString(answer.message), true)
        return
    end if
    m.localCues = httpArray(answer.data.cues)
    m.video.notificationInterval = picNotifySeconds()
    drawCue()
end sub

' The cues showing now, moved by subtitle sync, while the video plays:
' paused or seeking, Roku's bar has the bottom of the screen.
sub drawCue()
    if m.localCues = invalid then return
    text = ""
    if m.video.state = "playing" then text = driveCueText(m.localCues, m.positionMs, m.syncMs)
    if text = m.cueText.text and m.cue.visible = (text <> "") then return
    m.cueText.text = text
    m.cueShadow.text = text
    m.cue.visible = text <> ""
end sub

sub hideCue()
    m.cueText.text = ""
    m.cueShadow.text = ""
    m.cue.visible = false
end sub

' The cues drawn as the Roku's caption settings say for the subtitles it
' draws itself (Drive.brs's driveCueStyle), read as each video opens.
sub styleCue()
    device = CreateObject("roDeviceInfo")
    options = {}
    for each name in driveCueOptions()
        options[name] = device.GetCaptionsOption(name)
    end for
    style = driveCueStyle(options)
    m.cueText.font = style.font
    m.cueShadow.font = style.font
    m.cueText.color = style.color
    m.cueShadow.color = style.shadowColor
    m.cueShadow.visible = style.shadow
end sub

' Pley's menu for a video on a drive: Subtitles (the file's own and the
' files beside it) and Audio (the Video node's tracks, where there are two
' or more), Subtitle sync while a file beside it shows, Sleep timer and
' Playback details.
sub openLocalMenu()
    hideSkip()
    m.menuKind = "menu"
    m.menuTitle.text = "Playback"
    items = []
    m.menuActions = []
    subtitles = localSubtitles()
    if subtitles.labels.Count() > 1
        items.Push({ title: "Subtitles: " + subtitles.labels[subtitles.current] })
        m.menuActions.Push("subtitles")
    end if
    audio = localAudio()
    if audio.labels.Count() > 1
        items.Push({ title: "Audio: " + audio.labels[audio.current] })
        m.menuActions.Push("audio")
    end if
    if m.localSubtitle >= 0
        items.Push({ title: "Subtitle sync: " + syncLabel(m.syncMs) })
        m.menuActions.Push("sync")
    end if
    items.Push({ title: "Sleep timer: " + sleepLabel(m.sleep, sleepNow(), playingKind()) })
    m.menuActions.Push("sleep")
    items.Push({ title: "Playback details" })
    m.menuActions.Push("details")
    at = m.menuList.itemFocused
    m.menuList.content = nodesContent(items)
    if at > 0 and at < items.Count() then m.menuList.jumpToItem = at
    m.menuChoices.visible = false
    m.menuText.visible = false
    m.menuList.visible = true
    m.menuHint.text = "Back closes this."
    m.menu.visible = true
    m.menuList.setFocus(true)
end sub

' The subtitles Pley's menu offers (driveSubtitleChoices), and the one
' showing: { labels, values, current }.
function localSubtitles() as object
    choices = driveSubtitleChoices(m.video.availableSubtitleTracks, m.localSidecars)
    shown = driveSubtitleShown(m.localSubtitle, httpString(m.video.globalCaptionMode), httpString(m.video.currentSubtitleTrack))
    choices.current = 0
    for i = 0 to choices.values.Count() - 1
        if choices.values[i] = shown then choices.current = i
    end for
    return choices
end function

' The Video node's audio tracks: { labels, current }.
function localAudio() as object
    labels = []
    current = 0
    playing = httpString(m.video.currentAudioTrack)
    tracks = httpArray(m.video.availableAudioTracks)
    for i = 0 to tracks.Count() - 1
        labels.Push(driveAudioName(tracks[i], i))
        if httpString(tracks[i].Track) = playing then current = i
    end for
    return { labels: labels, current: current }
end function

' The subtitles (Off first) or the audio tracks, the one playing checked.
sub openLocalTracks(kind as string)
    m.menuKind = kind
    items = []
    m.menuValues = []
    if kind = "audio"
        m.menuTitle.text = "Audio"
        audio = localAudio()
        for i = 0 to audio.labels.Count() - 1
            items.Push({ title: audio.labels[i] })
            m.menuValues.Push(i)
        end for
        m.trackCurrent = audio.current
        showMenuChoices(items, audio.current)
        return
    end if
    m.menuTitle.text = "Subtitles"
    subtitles = localSubtitles()
    for each label in subtitles.labels
        items.Push({ title: label })
    end for
    m.menuValues = subtitles.values
    m.trackCurrent = subtitles.values[subtitles.current]
    showMenuChoices(items, subtitles.current)
end sub

' What plays: the file as it is, from the drive.
sub showLocalDetails()
    lines = ["Playing the file as it is", "From the USB drive, with no server", ""]
    lines.Push("File: " + driveFileName(httpString(m.local.path)))
    video = detCodec(m.video.videoFormat)
    if video <> "" then lines.Push("Video: " + video)
    audio = detCodec(m.video.audioFormat)
    if audio <> "" then lines.Push("Audio: " + audio)
    if m.localSubtitle >= 0 then lines.Push("Subtitles: " + m.localSidecars[m.localSubtitle].label + " · Drawn by Pley")
    if m.durationMs > 0 then lines.Push("Position: " + fmtClock(m.positionMs) + " of " + fmtClock(m.durationMs))
    m.menuText.text = lines.Join(Chr(10))
end sub

' Another title: Automatic starts from the setting again.
sub newTitle()
    m.watch = invalid
    m.watchItem = invalid
    m.autoQuality = ""
    m.autoNotice = ""
    m.autoTimer.control = "stop"
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
    m.nextCard = invalid
    m.upNextShown = false
    m.upNextCancelled = false
    m.ended = false
    m.stallTimer.control = "stop"
    m.stoppedTimer.control = "stop"
    m.slowSaid = false
    m.seenMs = -1
    m.markers = []
    m.skipMarker = invalid
    m.skipKey = ""
    m.skipDismissed = false
    m.textTracks = 0
    m.tracksFrom = -1
    stopPictures()
    m.sync.visible = false
    closeMenu()
    hideSkip()
    hideUpNext()
    m.notice.visible = false
    showWaiting()
    m.top.setFocus(true)
    if m.item = invalid
        uiRequest({ method: "GET", path: "/api/items/" + urlValue(itemId), attempt: m.attempt }, "onItem")
        return
    end if
    itemKnown()
    askPlayback()
end sub

' POST /api/playback for the version chosen, else the first file, with
' its picture subtitles to burn in when the person's choices pick some the
' app doesn't draw itself, at the
' quality chosen in Settings (or Automatic's). A title's first opening in
' Automatic asks the server what it knows of the network first.
sub askPlayback()
    choice = aqChoice(playbackSection())
    if choice = "auto" and m.autoQuality = ""
        startAutomatic()
        return
    end if
    requestPlayback(choice, 0)
end sub

' Where Automatic starts (AutoQuality.brs): the server's word on this
' network, asked 1.5 s at most (the API task keeps it 10 minutes, so a play
' at home doesn't wait), after a speed test where the server asks for one;
' without an answer, what this Roku remembers. The loading circle shows
' meanwhile.
sub startAutomatic()
    m.networkAt = m.clock.TotalMilliseconds()
    m.probe = invalid
    uiRequest({ op: "network", attempt: m.attempt }, "onNetwork")
end sub

sub onNetwork(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if not answer.ok
        requestPlayback("auto", aqRemembered(playbackSection(), m.global.session.server, nowSeconds()))
        return
    end if
    if aqWantsProbe(answer.data)
        timeout = aqProbeTimeoutMs(m.clock.TotalMilliseconds() - m.networkAt)
        uiRequest({ op: "speedTest", timeoutMs: timeout, attempt: m.attempt }, "onSpeedTest")
        return
    end if
    requestPlayback("auto", aqNetworkCeiling(answer.data))
end sub

' The speed test's bytes over its time go to the server, whose answer is
' the ceiling; too little measured starts as if nothing were known.
sub onSpeedTest(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    result = invalid
    if answer.ok and GetInterface(answer.data, "ifAssociativeArray") <> invalid then result = aqProbeResult(httpNumber(answer.data.bytes, 0), httpNumber(answer.data.ms, 0))
    if result = invalid
        requestPlayback("auto", 0)
        return
    end if
    m.probe = result
    uiRequest({ op: "tellNetwork", probeKbps: result.probe_kbps, timeoutMs: 1000, attempt: m.attempt }, "onNetworkTold")
end sub

sub onNetworkTold(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    ceiling = aqProbeCeiling(m.probe)
    if answer.ok and GetInterface(answer.data, "ifAssociativeArray") <> invalid then ceiling = aqNetworkCeiling(answer.data)
    requestPlayback("auto", ceiling)
end sub

' The session at the choice's quality; ceilingKbps is where Automatic
' starts under (0 for none).
sub requestPlayback(choice as string, ceilingKbps as float)
    ' Keyed addresses: Roku fetches the seek previews itself, without the
    ' Authorization header.
    spec = { op: "play", itemId: m.itemId, startMs: m.startMs, mode: m.mode, attempt: m.attempt, keyed: true }
    ' PGS the app can't draw (no pictures from this server) is burned in:
    ' the caps say the app doesn't show it, or the server would leave it out.
    drawn = picCodecs()
    if m.picturesOff
        drawn = []
        spec.picturesOff = true
    end if
    ' The version chosen on the title's page; a title played next has none
    ' of its files, so it plays its first (the server's default too).
    file = contentFile(m.item, m.top.params.fileId)
    if file <> invalid
        spec.fileId = httpNumber(file.id, 0)
        burn = playerBurnIndex(file, drawn)
        if burn >= 0 then spec.subtitleIndex = burn
    end if
    again = ""
    if choice = "auto" then again = m.autoQuality
    ' What it obeys of the user's remote: next where something may come
    ' next, the file's audio and subtitles, and the quality.
    tracks = tracksIndexes(file)
    spec.controls = remoteVideoControls(playerMayHaveNext(m.item, m.queue, m.queueAt), tracks.audio.Count(), tracks.subtitles.Count(), true)
    ' The audio a cast named, for this play (Cast.brs's castGivenTracks).
    if m.playAudio >= 0 then spec.audioIndex = m.playAudio
    m.asked = aqAsk(choice, again, ceilingKbps, file)
    ' A file that failed to play as it is stays converted (broken()).
    if m.mode <> "transcode" then spec.mode = m.asked.mode
    spec.maxBitrate = m.asked.maxBitrate
    spec.maxHeight = m.asked.maxHeight
    m.playRequest = uiRequest(spec, "onPlayback")
end sub

' Where the quality setting and Automatic's ceilings are kept on this Roku.
function playbackSection() as object
    return CreateObject("roRegistrySection", "playback")
end function

function nowSeconds() as integer
    return CreateObject("roDateTime").AsSeconds()
end function

' Automatic's ceiling for the next titles (kbps, 0 forgets it): kept on
' this Roku for 30 minutes and told to the server, which keeps it a day
' for this Roku on this network. Nothing waits for the server's answer.
sub keepCeiling(kbps as float)
    aqRemember(playbackSection(), m.global.session.server, kbps, nowSeconds())
    uiRequest({ op: "tellNetwork", ceilingKbps: kbps }, "")
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

' An episode has a next one to offer before its end; a playlist's entry,
' the entry after it, whose details give Up Next's card its episode.
sub itemKnown()
    if m.queue <> invalid
        if m.queueAt + 1 >= m.queue.Count() then return
        m.next = m.queue[m.queueAt + 1]
        m.nextCard = playerUpNextCard(invalid, m.global.session.server, m.next)
        uiRequest({ method: "GET", path: "/api/items/" + urlValue(m.next.itemId), attempt: m.attempt }, "onNextItem")
        return
    end if
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
    m.stillAt = m.startMs
    m.stillSince = m.clock.TotalMilliseconds()
    m.stallTimer.control = "start"
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
    watchPlayback()
    startPictures()
end sub

' Automatic's watch over what now plays: the title's own after a restart,
' told the quality it plays at now (the original when the server played
' the file as it is), the bitrate that is, the server's own quality for
' the file, the lines the stream is named by, whether its bitrate changes
' as it is sent (live_bitrate) and whether it comes in several sizes.
sub watchPlayback()
    if m.asked = invalid or not m.asked.automatic or httpString(m.item.kind) = "track"
        newTitle()
        return
    end if
    file = contentFile(m.item, m.info.file_id)
    fileKbps = aqFileKbps(file)
    fileLines = aqFileLines(file)
    converted = httpString(m.info.method) = "transcode"
    cap = httpString(m.info.auto_quality)
    quality = m.asked.quality
    if not converted then quality = "original"
    if quality = "original"
        playing = fileKbps
        if converted and aqKbps(cap) > 0 then playing = aqKbps(cap)
    else
        playing = aqKbps(quality)
        if fileKbps > 0 and fileKbps < playing then playing = fileKbps
    end if
    live = converted and httpTrue(m.info.live_bitrate)
    lines = aqStreamLines(quality, converted, cap, aqNamedLines(aqFileWidth(file), fileLines))
    now = m.clock.TotalMilliseconds()
    if m.watch <> invalid and m.watchItem = m.itemId
        aqRestarted(m.watch, now, quality, playing, live, lines, converted)
    else
        m.watch = aqWatch(now, quality, playing, fileKbps, fileLines, cap, m.asked.lowered and quality <> "original", live, lines, converted)
        m.watchItem = m.itemId
    end if
    m.watch.adaptive = adaptiveStream()
    m.autoTimer.control = "start"
end sub

' The Video node's state, for Automatic: a wait while it plays (one the
' Roku says is for want of data, where it says), and the notice of a step
' down once the stream plays.
sub watchState(state as string)
    if m.watch = invalid then return
    if state = "playing" and m.autoNotice <> ""
        uiToast(m.autoNotice)
        m.autoNotice = ""
    end if
    underrun = invalid
    if state = "buffering"
        status = m.video.bufferingStatus
        if GetInterface(status, "ifAssociativeArray") <> invalid and GetInterface(status.isUnderrun, "ifBoolean") <> invalid then underrun = status.isUnderrun
    end if
    decision = aqState(m.watch, m.clock.TotalMilliseconds(), state, underrun)
    if decision <> invalid then autoStep(decision)
end sub

sub onAutoTick()
    if m.watch = invalid or m.info = invalid then return
    decision = aqTick(m.watch, m.clock.TotalMilliseconds())
    if decision <> invalid then autoStep(decision)
end sub

' The stream comes in several sizes the Video node moves between by itself,
' as the connection allows: Automatic leaves its waits to it, and only
' raises where the next title starts.
function adaptiveStream() as boolean
    if m.info = invalid then return false
    flag = m.info.adaptive
    return GetInterface(flag, "ifBoolean") <> invalid and flag
end function

' Each HLS segment the Video node downloaded: its size over the time it
' took is what the network carried.
sub onSegment(event as object)
    if m.watch = invalid then return
    segment = event.getData()
    if GetInterface(segment, "ifAssociativeArray") = invalid then return
    ' 1 is audio alone and 3 captions: the video's are muxed (0) or video (2).
    kind = Int(httpNumber(segment.SegType, 0))
    if kind = 1 or kind = 3 then return
    aqSegment(m.watch, m.clock.TotalMilliseconds(), httpNumber(segment.SegSize, 0), httpNumber(segment.DownloadDuration, 0))
end sub

' Carries out Automatic's step (aqStep). The network's memory follows each
' (a step up to a quality keeps its bitrate too;
' back at the original, it is forgotten). A step down says so: at once
' when the stream plays on, else once the new one plays.
sub autoStep(decision as object)
    if decision.memory >= 0 then keepCeiling(decision.memory)
    if decision.kind = "next" then return
    if decision.kind = "bitrate"
        if decision.down then uiToast("Quality lowered to " + aqBitrateLabel(decision.kbps, m.watch.lines))
        setBitrate(decision.kbps)
        return
    end if
    if decision.down then m.autoNotice = "Quality lowered to " + aqLabel(decision.quality)
    mode = m.mode
    quality = decision.quality
    if quality = "converted"
        ' The original converted, at the server's own quality.
        quality = "original"
        mode = "transcode"
    end if
    m.autoQuality = quality
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    m.autoTimer.control = "stop"
    open(m.itemId, position, m.item, mode)
end sub

' PUT /api/playback/{session}/bitrate: the stream goes on at kbps from the
' segments the Video node hasn't fetched yet, the same size and playlist,
' with no new stream (docs/QUALITY.md, "Players that go black between
' streams").
sub setBitrate(kbps as integer)
    if m.sessionId = "" then return
    path = "/api/playback/" + urlEncode(m.sessionId) + "/bitrate"
    uiRequest({ method: "PUT", path: path, json: { max_bitrate: kbps }, session: m.sessionId, kbps: kbps }, "onBitrate")
end sub

' What the stream goes on at, or that the server refused (one from before,
' a session gone): the watch then moves it as a stream whose bitrate can't
' change.
sub onBitrate(event as object)
    spec = event.getRoSGNode().spec
    if m.watch = invalid or spec.session <> m.sessionId then return
    answer = event.getData()
    kbps = 0
    if answer.ok
        kbps = httpNumber(spec.kbps, 0)
        if GetInterface(answer.data, "ifAssociativeArray") <> invalid then kbps = httpNumber(answer.data.bitrate, kbps)
    end if
    aqAnswered(m.watch, httpNumber(spec.kbps, 0), kbps)
end sub

' The title at the top for 4 s as an item begins to play (the first, the
' next episode, a queue's next), not as the same one opens again; not with
' Playback details, which have its place, as on Android TV.
sub showBanner()
    m.bannerDue = false
    if m.showDetails
        updateBanner()
        return
    end if
    m.bannerUp = true
    m.bannerTimer.control = "stop"
    m.bannerTimer.control = "start"
    updateBanner()
end sub

sub hideBanner()
    m.bannerTimer.control = "stop"
    m.bannerUp = false
    updateBanner()
end sub

' Settings → Playback details: the readout at the top left while nothing
' else shows (the controls, the menu, Up Next, subtitle sync, what went
' wrong), read again as the video plays: Details.brs's detReadout, with
' what the Video node says of the stream.
sub updateReadout()
    shown = m.showDetails and m.info <> invalid and not controlsShown() and not m.menu.visible and not m.upNext.visible and not m.notice.visible and not m.sync.visible
    m.readout.visible = shown
    if not shown then return
    node = { videoFormat: m.video.videoFormat, audioFormat: m.video.audioFormat, streamBps: 0, networkBps: 0, subtitles: subtitleLabel() }
    stream = httpObject(m.video.streamInfo)
    if stream <> invalid
        node.streamBps = httpNumber(stream.streamBitrate, 0)
        node.networkBps = httpNumber(stream.measuredBitrate, 0)
    end if
    typeSet(m.readoutText, detReadout(m.info, playingFile(), m.audioPlaying, node).Join(Chr(10)))
    m.readoutBack.height = m.readoutText.boundingRect().height + 32
end sub

' The subtitles showing, as the menu names them: Off, a track of the file,
' or "" for one it doesn't list.
function subtitleLabel() as string
    tracks = mediaTracks(playingFile()).subtitles
    playing = subtitlePlaying()
    for i = 0 to tracks.values.Count() - 1
        if tracks.values[i] = playing then return tracks.labels[i]
    end for
    if playing < 0 then return "Off"
    return ""
end function

' Roku's bar showing or gone (trick play, a pause, OK pressed).
sub onBar()
    m.barShown = httpTrue(m.video.trickPlayBarVisibilityHint)
    updateBanner()
    updateSkip()
end sub

' Whether the player's controls are up, as Android TV's: paused or Roku's
' bar showing, once the video played, with nothing over it (subtitle sync
' has the top).
function controlsShown() as boolean
    if not m.played or m.ended or m.notice.visible or m.upNext.visible or m.sync.visible then return false
    return m.video.state = "paused" or m.barShown
end function

' The banner as it is now: the title while it is up or the controls are;
' with the controls, how it plays and the clock at the right, and, paused,
' when it ends.
sub updateBanner()
    controls = controlsShown()
    banner = playerBanner(m.item)
    shown = (m.bannerUp or controls) and banner.title <> ""
    m.banner.visible = shown
    typeSet(m.bannerTitle, banner.title)
    typeSet(m.bannerLine, banner.line)
    ticking = shown and controls
    m.bannerRight.visible = ticking
    updateReadout()
    if ticking <> m.clockTicking
        m.clockTicking = ticking
        if ticking
            m.clockTimer.control = "start"
        else
            m.clockTimer.control = "stop"
        end if
    end if
    if not ticking then return
    now = CreateObject("roDateTime").AsSeconds()
    status = []
    if m.info <> invalid then status.Push(detMethod(m.info))
    status.Push(playerClock(now, m.twelve))
    typeSet(m.bannerStatus, status.Join(" · "))
    m.bannerEnds.text = ""
    if m.video.state = "paused" then typeSet(m.bannerEnds, playerEndsAt(now, m.positionMs, m.durationMs, m.twelve))
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
    m.streamSyncMs = m.syncMs
    if stream.previews <> ""
        content.SDBifUrl = stream.previews
        content.HDBifUrl = stream.previews
        content.FHDBifUrl = stream.previews
    end if
    if LCase(Left(stream.url, 8)) = "https://" then nodesFill(content, { HttpCertificatesFile: "common:/certs/ca-bundle.crt" })
    m.video.setHttpAgent(uiHttpAgent())
    m.video.content = content
    if subtitle <> "" then m.video.subtitleTrack = subtitle
end sub

' Where subtitle sync is remembered on this Roku.
function syncSection() as object
    return CreateObject("roRegistrySection", "subtitleSync")
end function

' Pley's menu is offered while a stream plays; subtitle sync in it for a
' file with text subtitles, or picture ones drawn here.
function menuOffered() as boolean
    return (m.info <> invalid or m.local <> invalid) and not m.notice.visible and not m.upNext.visible and not m.sync.visible
end function

sub openSync()
    hideSkip()
    m.syncWas = m.syncMs
    typeSet(m.syncValue, syncLabel(m.syncMs))
    m.sync.visible = true
    updateBanner()
    m.syncButtons.focusButton = 0
    m.syncButtons.setFocus(true)
end sub

sub stepSync(byMs as integer)
    m.syncMs = syncClamp(m.syncMs + byMs)
    typeSet(m.syncValue, syncLabel(m.syncMs))
    ' Pictures and a drive's subtitles drawn here move at once.
    drawPictures()
    drawCue()
end sub

sub onSyncButton(event as object)
    if m.syncButtons.buttons[event.getData()] = "Reset"
        stepSync(-m.syncMs)
    else
        closeSync()
    end if
end sub

' Done or Back: a new offset is remembered for the file, and the stream
' opens again where it was, its text tracks moved by the server. Pictures
' drawn here moved already: the stream plays on.
sub closeSync()
    if not m.sync.visible then return
    m.sync.visible = false
    m.video.setFocus(true)
    updateBanner()
    if m.syncMs = m.syncWas then return
    syncRemember(syncSection(), m.syncKey, m.syncMs)
    if m.pictureIndex < 0 then reopen()
end sub

' The stream again from where it is: the same session and address, the
' subtitles that showed shown again, moved (or the track at subtitle).
sub reopen(subtitle = "" as string)
    if m.item = invalid or m.info = invalid then return
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    showing = subtitle
    if showing = "" then showing = syncBaseUrl(httpString(m.video.currentSubtitleTrack))
    if showing = "" then showing = syncBaseUrl(httpString(m.video.subtitleTrack))
    m.positionMs = position
    showWaiting()
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
    captionsTo("On")
end sub

' Captions on, or off while a drive's subtitles are drawn here, until
' captionsBack.
sub captionsTo(mode as string)
    if m.captionMode = invalid then m.captionMode = m.video.globalCaptionMode
    m.captionSet = mode
    m.video.globalCaptionMode = mode
end sub

sub captionsBack()
    if m.captionMode = invalid then return
    if m.video.globalCaptionMode = m.captionSet then m.video.globalCaptionMode = m.captionMode
    m.captionMode = invalid
end sub

' Captions turned on or off, or another subtitle or audio track, in Roku's
' options: once the stream played, the person's pick, remembered on the
' server for the show or the movie. Before, what shows is pley's choice or
' the Roku's own, and only noted.
sub onTracks()
    if m.info = invalid or m.item = invalid then return
    file = contentFile(m.item, m.info.file_id)
    mode = httpString(m.video.globalCaptionMode)
    subtitle = tracksSubtitleShown(mode, httpString(m.video.currentSubtitleTrack), m.info.subtitles, m.global.session.server)
    settled = m.tracksFrom >= 0 and m.clock.TotalMilliseconds() >= m.tracksFrom
    ' Roku's options can't name pictures drawn here: captions on without a
    ' text track leave them be; turned off by the person, they go.
    turnedOff = settled and LCase(mode) = "off" and LCase(m.captionModeSeen) <> "off"
    m.captionModeSeen = mode
    if m.pictureIndex >= 0 and subtitle = -1 and not turnedOff then subtitle = -2
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
    if not settled then return
    saveTracks(file, subtitlePicked, audioPicked)
end sub

' PUT /api/items/{id}/streams with the tracks playing; kept in the file too,
' so a stream started again shows them. Text subtitles picked over picture
' ones the server draws need a stream without those; over ones drawn here,
' only that those go.
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
        if body.DoesExist("audio_index")
            file.selected_audio = body.audio_index
            file.audio_picked = body.audio_index >= 0
        end if
        file.selected_subtitle = body.subtitle_index
    end if
    if subtitlePicked and m.pictureIndex >= 0 and subtitle <> m.pictureIndex
        stopPictures()
        ' A sync changed meanwhile moved the pictures alone: the text track
        ' moves with the stream opened again.
        if subtitle >= 0 and m.syncMs <> m.streamSyncMs then reopen()
    end if
    if not subtitlePicked or not tracksNeedNewSession(burned, subtitle) then return
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    open(m.itemId, position, m.item, m.mode)
end sub

' A request before playback failed: the server doesn't answer (the player
' reconnects, or says so), the sign-in ended (the scene shows sign-in), or
' a refusal (Try again where asking again may help).
sub failed(answer as object)
    if answer.error = "signed_out" then return
    if answer.error = "network"
        reconnect()
        return
    end if
    buttons = ["Back"]
    if playerMayTryAgain(answer.status) then buttons = ["Try again", "Back"]
    notice("Can't play this item", answer.message, buttons)
end sub

sub onState(event as object)
    state = event.getData()
    watchState(state)
    drawPictures()
    drawCue()
    ' The playhead is read again from now: a pause, or a seek's wait, isn't
    ' time it should have moved in. The last reading stays, so a seek made
    ' here (Skip Intro, Chapters) moves the next one away from it.
    m.seenAt = m.clock.TotalMilliseconds()
    if state <> "paused" then m.pausedTimer.control = "stop"
    if state <> "playing" then hideBanner()
    ' Waiting for data once it played, as for the first: the spinner.
    if state = "buffering" and m.played then showWaiting()
    if state = "paused" or state = "finished" then hideWaiting()
    if state = "playing"
        m.played = true
        m.everPlayed = true
        endReconnect()
        hideWaiting()
        if m.bannerDue
            showBanner()
        else
            updateBanner()
        end if
        ' Played again after the sleep timer's pause: the player stays.
        if m.sleepPausedAt >= 0
            m.sleepPausedAt = -1#
            if m.sleep.minutes = 0 then m.sleepTimer.control = "stop"
        end if
        if m.tracksFrom < 0 then m.tracksFrom = m.clock.TotalMilliseconds() + tracksSettleMs()
        if m.playNotice <> ""
            ' As long as Android TV's: it says where the way back is.
            uiToast(m.playNotice, false, 6)
            m.playNotice = ""
        end if
        if m.paused or m.reportAtPlay
            m.paused = false
            m.reportAtPlay = false
            report("playing")
        end if
        updateSkip()
        return
    end if
    ' Waiting or at the end: the Video node's own. Paused, Skip stays, as
    ' on Android TV, above Roku's bar.
    if state <> "paused" then hideSkip()
    if state = "paused"
        m.paused = true
        report("paused")
        m.pausedTimer.control = "start"
        updateSkip()
    else if state = "finished"
        ' Media that falls short of the title (resumed past the end of what
        ' the file or stream holds) isn't its end. A drive's video has no
        ' title but itself.
        if m.local = invalid and playerEndedShort(m.positionMs, m.startMs, m.durationMs)
            endedShort()
        else
            ended()
        end if
    else if state = "error"
        broken()
    end if
end sub

sub onPosition(event as object)
    seconds = event.getData()
    if seconds = invalid then return
    m.positionMs = Int(seconds * 1000)
    now = m.clock.TotalMilliseconds()
    ' A drive's video: its length as the Video node reads it.
    if m.local <> invalid and m.video.duration <> invalid and m.video.duration > 0 then m.durationMs = Int(m.video.duration * 1000)
    if m.watch <> invalid then aqPosition(m.watch, now, m.positionMs)
    if m.positionMs <> m.stillAt
        m.stillAt = m.positionMs
        m.stillSince = now
    end if
    ' A seek, here or in Roku's own bar, is told at once, as the other apps
    ' tell it a second after: other devices resume from there.
    sought = m.seenMs >= 0 and playerSought(m.seenMs, m.positionMs, now - m.seenAt)
    m.seenMs = m.positionMs
    m.seenAt = now
    if sought
        state = "playing"
        if m.paused then state = "paused"
        report(state)
    else if playerShouldReport(m.video.state, now - m.lastReportAt)
        report("playing")
    end if
    if m.next <> invalid and not m.upNextShown and m.upNextAt > 0 and m.positionMs >= m.upNextAt and not sleepEndsWith(m.sleep, urlValue(m.itemId)) then showUpNext()
    updateSkip()
    followPictures()
    drawPictures()
    if m.readout.visible then updateReadout()
    drawCue()
end sub

' Skip Intro or Skip Credits while the playhead is in one and the video
' plays or is paused; above Roku's bar while that shows.
sub updateSkip()
    marker = playerSkipFor(m.markers, m.positionMs, m.next <> invalid and not m.upNextCancelled)
    key = ""
    if marker <> invalid then key = marker.key
    if key <> m.skipKey
        m.skipKey = key
        m.skipDismissed = false
        if marker <> invalid then typeSet(m.skip, playerSkipLabel(marker))
    end if
    m.skipMarker = marker
    state = m.video.state
    top = 924
    if controlsShown() then top = 760
    m.skip.translation = [1544, top]
    shown = marker <> invalid and not m.skipDismissed and (state = "playing" or state = "paused") and not m.upNext.visible and not m.notice.visible and not m.sync.visible and not m.menu.visible
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

' What play, fast forward, rewind and instant replay do when the skip
' button, Up Next or the menu has the focus instead of the video: pause or
' go on, 30 s further or back, or 10 s back.
sub playerKey(key as string)
    if key = "play"
        if m.video.state = "paused"
            m.video.control = "resume"
        else
            m.video.control = "pause"
        end if
    else if key = "fastforward"
        seekTo(m.positionMs + playerJumpMs())
    else if key = "rewind"
        seekTo(m.positionMs - playerJumpMs())
    else
        seekTo(m.positionMs - playerReplayMs())
    end if
end sub

sub seekTo(targetMs as integer)
    if m.watch <> invalid then aqSeeked(m.watch)
    target = targetMs
    last = Int(m.durationMs) - 1000
    if last > 0 and target > last then target = last
    if target < 0 then target = 0
    m.positionMs = target
    m.video.seek = target / 1000
end sub

' POST /api/items/{id}/progress for the current session. Stopped, the
' screens showing the title follow (Ui.brs's uiNoteChange).
sub report(state as string)
    if m.local <> invalid
        reportLocal(state)
        return
    end if
    if m.sessionId = "" then return
    if state = "stopped" then uiNoteChange({ itemId: m.itemId, action: "played", positionMs: m.positionMs, durationMs: m.durationMs, watched: playerWatched(m.positionMs, m.durationMs) })
    m.lastReportAt = m.clock.TotalMilliseconds()
    path = "/api/items/" + urlValue(m.itemId) + "/progress"
    ' With the tracks playing and the quality picked, which the user's
    ' remotes tick.
    tracks = { audio: m.audioPlaying, subtitle: subtitlePlaying(), quality: qualityPicked() }
    uiRequest({ method: "POST", path: path, json: playerProgress(m.positionMs, m.durationMs, state, m.sessionId, tracks), session: m.sessionId }, "onReported")
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
    m.stallTimer.control = "stop"
    hideBanner()
    if m.durationMs > 0 then m.positionMs = m.durationMs
    report("stopped")
    endSession()
    ' The sleep timer's end: nothing plays after this one.
    if sleepEndsWith(m.sleep, urlValue(m.itemId))
        uiNavigate({ back: true })
        return
    end if
    if m.next = invalid
        uiNavigate({ back: true })
        return
    end if
    ' Up Next at the end, as on the other apps: shown since the credits, it
    ' goes on counting down (or waiting for Play now); else it shows now,
    ' counting down (again, after a Cancel) when the next plays by itself.
    m.ended = true
    if m.upNext.visible then return
    m.upNextCancelled = false
    showUpNext()
end sub

' The media ended moments after it started, a minute or more before the
' title's end, as on Android: a file played as it is is converted, and a
' stream opened again once for the title; falling short after that, the
' player says so. Nothing is reported watched.
sub endedShort()
    m.stallTimer.control = "stop"
    hideBanner()
    if m.info <> invalid and httpString(m.info.method) <> "transcode" and m.mode <> "transcode"
        m.fallback = "unplayable"
        open(m.itemId, m.startMs, m.item, "transcode")
        return
    end if
    if m.shortTries > 0
        endSession()
        notice("Can't play this item", "This video ends before the point it was to play from.", ["Back"])
        return
    end if
    m.shortTries = m.shortTries + 1
    open(m.itemId, m.startMs, m.item, m.mode)
end sub

' Leaving the player: the place is kept and the session closed (or, still
' to come, closed when it does).
sub finish()
    abandonPlayback()
    m.attempt = m.attempt + 1
    m.tracksFrom = -1
    m.reportAtPlay = false
    m.countdownTimer.control = "stop"
    m.sleepTimer.control = "stop"
    m.pausedTimer.control = "stop"
    m.sleepNotice.visible = false
    m.stallTimer.control = "stop"
    m.detailsTimer.control = "stop"
    m.stoppedTimer.control = "stop"
    endReconnect()
    hideWaiting()
    hideBanner()
    m.clockTimer.control = "stop"
    m.clockTicking = false
    newTitle()
    stopPictures()
    keepLocal()
    hideCue()
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

' The server stopped the stream, by its administrator or for another
' device: the player says so itself, "Playback stopped" and why, as
' Android TV's, then leaves 20 s later (or at Back).
sub stopped(data as dynamic)
    m.tracksFrom = -1
    stopPictures()
    m.sessionId = ""
    m.video.control = "stop"
    ' Automatic has nothing left to watch: it would open the title again.
    newTitle()
    notice("Playback stopped", playerStoppedMessage(data), ["Back"])
    m.stoppedTimer.control = "start"
end sub

sub onStoppedTimer()
    if m.notice.visible and m.top.active then uiNavigate({ back: true })
end sub

sub broken()
    ' A drive's video has no server to convert it, or to ask about it.
    if m.local <> invalid
        if m.played
            localFailed("This video stopped. The drive may have been pulled out.")
        else
            localFailed("This Roku can't play this file as it is, and a file on a drive has no server to convert it.")
        end if
        return
    end if
    if playerErrorKind(m.video.errorCode, m.video.errorInfo, m.played) = "media"
        ' The original doesn't play on this Roku after all: converted, once,
        ' which Quality says.
        if m.info <> invalid and httpString(m.info.method) <> "transcode" and m.mode <> "transcode"
            m.fallback = "unplayable"
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
    showWaiting()
    uiRequest({ method: "GET", path: "/api/playback/" + urlEncode(m.sessionId), session: m.sessionId }, "onSessionState")
end sub

sub onSessionState(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId then return
    answer = event.getData()
    meaning = playerSessionState(answer.status)
    if meaning = "stopped"
        stopped(answer.data)
    else if meaning = "unreachable"
        reconnect()
    else if meaning <> "signed_out"
        restart()
    end if
end sub

' The server doesn't answer. Once something played, the player keeps its
' place (and its queue) and asks again every 2 s, saying "Reconnecting…",
' as long as the server keeps a session it doesn't hear from (90 s), as the
' other apps do; before anything played, or after that, it says so in
' Android TV's words, with Try again.
sub reconnect()
    now = m.clock.TotalMilliseconds()
    if m.reconnectFrom < 0 then m.reconnectFrom = now
    if not m.everPlayed or now - m.reconnectFrom >= playerReconnectMs()
        notice("Can't reach your server", "Check that the server is on and that this Roku is on its network.", ["Try again", "Back"])
        return
    end if
    hideBanner()
    showWaiting()
    if m.spinner.visible then m.reconnecting.visible = true
    m.reconnectTimer.control = "start"
end sub

' The session asked about again, or, when none came, asked for again.
sub onReconnectTick()
    if m.reconnectFrom < 0 then return
    if m.sessionId = ""
        open(m.itemId, m.positionMs, m.item, m.mode)
    else
        uiRequest({ method: "GET", path: "/api/playback/" + urlEncode(m.sessionId), session: m.sessionId }, "onSessionState")
    end if
end sub

' The server answers again (the video plays), or the player gave up.
sub endReconnect()
    m.reconnectFrom = -1
    m.reconnectTimer.control = "stop"
    m.reconnecting.visible = false
end sub

' The spinner once the wait has lasted 500 ms, so a quick start or a short
' wait shows none.
sub showWaiting()
    if m.waiting then return
    m.waiting = true
    m.spinnerTimer.control = "start"
end sub

sub onSpinnerTimer()
    if not m.waiting then return
    m.spinner.visible = true
    m.spinner.control = "start"
    m.reconnecting.visible = m.reconnectFrom >= 0
end sub

sub hideWaiting()
    m.waiting = false
    m.spinnerTimer.control = "stop"
    m.spinner.control = "stop"
    m.spinner.visible = false
    m.reconnecting.visible = false
end sub

' --- Stalls ---

' Once a second: whether the playhead stood still 12 s while the video is
' meant to play (not paused, ended, reconnecting or saying why it
' stopped), as the other apps watch for.
sub onStallTick()
    now = m.clock.TotalMilliseconds()
    state = m.video.state
    if m.local <> invalid
        localStallTick(now, state)
        return
    end if
    meant = m.played and not m.ended and m.reconnectFrom < 0 and not m.notice.visible and (state = "playing" or state = "buffering")
    if not meant then m.stillSince = now
    if not meant or now - m.stillSince < playerStallMs() then return
    m.stillSince = now
    starving = false
    if state = "buffering"
        status = m.video.bufferingStatus
        if GetInterface(status, "ifAssociativeArray") <> invalid and GetInterface(status.isUnderrun, "ifBoolean") <> invalid then starving = status.isUnderrun
    end if
    method = httpString(m.info.method)
    ' A file that has its data and doesn't move doesn't play smoothly here:
    ' converted, once a title, as on Android TV, saying so as it plays and
    ' offering Play original in Quality (Automatic's menu has Original).
    if not starving and method = "direct" and m.mode <> "transcode" and not m.struggled
        m.struggled = true
        m.fallback = "struggled"
        if aqChoice(playbackSection()) <> "auto" then m.playNotice = playerTranscodeToast(m.fallback)
        open(m.itemId, m.positionMs, m.item, "transcode")
        return
    end if
    ' A stream waits on the server: how fast it converts says whether a
    ' lower quality would help.
    if method <> "direct" and m.sessionId <> ""
        uiRequest({ method: "GET", path: "/api/playback/" + urlEncode(m.sessionId), session: m.sessionId }, "onStallChecked")
        return
    end if
    restart()
end sub

sub onStallChecked(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId then return
    answer = event.getData()
    meaning = playerSessionState(answer.status)
    if meaning = "stopped"
        stopped(answer.data)
        return
    end if
    if meaning = "unreachable"
        reconnect()
        return
    end if
    if meaning = "signed_out" then return
    speed = 0
    if answer.ok and GetInterface(answer.data, "ifAssociativeArray") <> invalid
        transcode = httpObject(answer.data.transcode)
        if transcode <> invalid then speed = httpNumber(transcode.speed, 0)
    end if
    if speed > 0 and speed < 1
        ' Automatic goes lower by itself; otherwise the person is told, once.
        if m.watch = invalid and not m.slowSaid
            m.slowSaid = true
            uiToast("The server can't keep up with this video. Try a lower quality: Down, then Quality.", true)
        end if
        return
    end if
    restart()
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
    endReconnect()
    hideBanner()
    m.stallTimer.control = "stop"
    hideWaiting()
    closeMenu()
    hideSkip()
    hideUpNext()
    typeSet(m.noticeTitle, title)
    typeSet(m.noticeText, text)
    m.noticeButtons.buttons = buttons
    m.notice.visible = true
    updateBanner()
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
    if item = invalid then return
    m.next = contentUpNext(item, m.global.session.server)
    m.nextCard = playerUpNextCard(item, m.global.session.server)
end sub

' The queue's next entry's details: its card names the episode.
sub onNextItem(event as object)
    if not isCurrent(event) or m.next = invalid then return
    answer = event.getData()
    if not answer.ok or httpObject(answer.data) = invalid then return
    if httpNumber(answer.data.id, 0) = m.next.itemId then m.nextCard = playerUpNextCard(answer.data, m.global.session.server, m.next)
end sub

' The card at the bottom right, as tall as its title: 978 is the bottom of
' Android TV's, 1824 its right edge.
sub showUpNext()
    closeSync()
    closeMenu()
    hideSkip()
    m.upNextShown = true
    card = m.nextCard
    if card = invalid then card = playerUpNextCard(invalid, m.global.session.server, m.next)
    typeSet(m.nextHeading, card.heading)
    typeSet(m.nextTitle, card.title)
    m.nextImage.setHttpAgent(m.top.getScene().getHttpAgent())
    m.nextImage.uri = card.image
    buttonsAt = 404 + m.nextTitle.boundingRect().height + 32
    m.nextButtons.translation = [32, buttonsAt]
    m.countdown.translation = [448, buttonsAt]
    height = buttonsAt + 64 + 32
    m.nextBack.height = height
    m.upNext.translation = [1224, 978 - height]
    m.upNext.visible = true
    updateBanner()
    m.nextButtons.focusButton = 0
    m.nextButtons.setFocus(true)
    ' Turned off in Settings, the next episode waits for Play now.
    m.countdown.text = ""
    if not playerAutoplay(playbackSection()) then return
    m.countdownLeft = playerUpNextSeconds()
    typeSet(m.countdown, playerNextIn(m.countdownLeft))
    m.countdownTimer.control = "start"
end sub

sub hideUpNext()
    m.countdownTimer.control = "stop"
    m.upNext.visible = false
    updateBanner()
end sub

sub onCountdown()
    m.countdownLeft = m.countdownLeft - 1
    if m.countdownLeft <= 0
        playNext()
    else
        typeSet(m.countdown, playerNextIn(m.countdownLeft))
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
    ' At the end, there is nothing left to play.
    if m.ended
        uiNavigate({ back: true })
        return
    end if
    m.video.setFocus(true)
    updateSkip()
end sub

' Up Next shows at the credits: moving on counts the episode as watched.
' The user's remote moves on from anywhere (finished false): the episode
' keeps its place.
sub playNext(finished = true as boolean)
    nextEpisode = m.next
    hideUpNext()
    if nextEpisode = invalid then return
    if m.sessionId <> ""
        if finished and m.durationMs > 0 then m.positionMs = m.durationMs
        report("stopped")
        endSession()
    end if
    m.video.control = "stop"
    m.restarts = []
    newTitle()
    ' A cast's audio was for the title it named.
    m.playAudio = -1
    m.bannerDue = true
    m.shortTries = 0
    m.fallback = ""
    m.struggled = false
    if m.queue <> invalid then m.queueAt = m.queueAt + 1
    open(nextEpisode.itemId, nextEpisode.startMs, invalid, "auto")
end sub

' While subtitle sync shows: left and right (rewind and fast forward too)
' move the subtitles, Back closes it. While Pley's menu shows: Back goes
' back in it, then closes it. While Up Next counts down: Back cancels it,
' and play, fast forward and rewind cancel it and act on the video.
' Otherwise Down brings up the menu, and Back leaves the player (the
' scene), which closes the session.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    ' The sleep timer's Keep watching: Back leaves it to count down, the
    ' video's own keys go on as ever.
    if m.sleepButtons.isInFocusChain()
        if key = "back"
            m.video.setFocus(true)
            return true
        end if
        if key = "play" or key = "fastforward" or key = "rewind"
            playerKey(key)
            return true
        end if
        return false
    end if
    if m.menu.visible
        if key = "back" or key = "left"
            if m.menuKind = "menu"
                closeMenu()
            else
                openMenu()
            end if
            return true
        end if
        ' The video's own keys go on as ever.
        if key = "play" or key = "fastforward" or key = "rewind"
            playerKey(key)
            return true
        end if
        if m.menuKind = "details" and (key = "up" or key = "down")
            scrollDetails(key = "down")
            return true
        end if
        return false
    end if
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
        ' At the end, Play plays the next one.
        if m.ended and key = "play"
            playNext()
            return true
        end if
        if key = "play" or key = "fastforward" or key = "rewind" or key = "replay"
            cancelUpNext()
            playerKey(key)
            return true
        end if
        return false
    end if
    if key = "down" and menuOffered()
        openMenu()
        return true
    end if
    return false
end function

' --- Pley's menu ---

' Its entries: Subtitles and Audio for a file with them, Quality, Subtitle
' sync for a file with text subtitles or pictures drawn here; for a file
' with two or more chapters, Chapters, Previous chapter and, before the
' last, Next chapter; Next episode (or Next in the queue) while something
' comes next; Sleep timer, Find subtitles…, Playback details. Previous and
' Next are those of Android TV's control row, which Roku's remote has no
' keys for.
sub openMenu()
    if m.local <> invalid
        openLocalMenu()
        return
    end if
    hideSkip()
    m.menuKind = "menu"
    m.menuTitle.text = "Playback"
    items = []
    m.menuActions = []
    ' Every track of the file, pictures too, as on the other apps' panels:
    ' Roku's options list only text subtitles, and a conversion's one audio.
    tracks = mediaTracks(playingFile())
    if tracks.subtitles.labels.Count() > 0
        items.Push({ title: "Subtitles: " + tracks.subtitles.labels[tracks.subtitles.current] })
        m.menuActions.Push("subtitles")
    end if
    if tracks.audio.labels.Count() > 0
        items.Push({ title: "Audio: " + tracks.audio.labels[tracks.audio.current] })
        m.menuActions.Push("audio")
    end if
    items.Push({ title: "Quality: " + qualityShown() })
    m.menuActions.Push("quality")
    if m.textTracks > 0 or m.pictureIndex >= 0
        items.Push({ title: "Subtitle sync: " + syncLabel(m.syncMs) })
        m.menuActions.Push("sync")
    end if
    chapters = detChapters(playingFile())
    if chapters.Count() > 1
        items.Push({ title: "Chapters" })
        m.menuActions.Push("chapters")
        items.Push({ title: "Previous chapter" })
        m.menuActions.Push("previousChapter")
        if detNextChapter(chapters, m.positionMs) >= 0
            items.Push({ title: "Next chapter" })
            m.menuActions.Push("nextChapter")
        end if
    end if
    if m.next <> invalid
        items.Push({ title: playerNextLabel(m.queue) })
        m.menuActions.Push("next")
    end if
    items.Push({ title: "Sleep timer: " + sleepLabel(m.sleep, sleepNow(), playingKind()) })
    m.menuActions.Push("sleep")
    items.Push({ title: "Find subtitles…" })
    m.menuActions.Push("find")
    items.Push({ title: "Playback details" })
    m.menuActions.Push("details")
    at = m.menuList.itemFocused
    m.menuList.content = nodesContent(items)
    if at > 0 and at < items.Count() then m.menuList.jumpToItem = at
    m.menuChoices.visible = false
    m.menuText.visible = false
    m.menuList.visible = true
    m.menuHint.text = "Back closes this."
    m.menu.visible = true
    updateReadout()
    m.menuList.setFocus(true)
end sub

sub closeMenu()
    if not m.menu.visible then return
    m.menu.visible = false
    m.menuKind = ""
    m.video.setFocus(true)
    updateSkip()
    updateReadout()
end sub

' The file playing: the session's, else the item's first.
function playingFile() as dynamic
    if m.item = invalid then return invalid
    fileId = invalid
    if m.info <> invalid then fileId = m.info.file_id
    return contentFile(m.item, fileId)
end function

' The quality as the menu says it: "Automatic · 8 Mbps 1080p" while
' Automatic plays, else the one chosen, named for the file.
function qualityShown() as string
    choice = aqChoice(playbackSection())
    labels = {}
    for each quality in aqMenuFor(playingFile())
        labels[quality.id] = quality.label
    end for
    if choice = "auto"
        now = "Original"
        if m.watch <> invalid and aqStepped(m.watch)
            now = aqBitrateLabel(m.watch.playingKbps, m.watch.lines)
        else if m.watch <> invalid and labels.DoesExist(m.watch.quality)
            now = labels[m.watch.quality]
        end if
        return "Automatic · " + now
    end if
    if labels.DoesExist(choice) then return labels[choice]
    return aqLabel(choice)
end function

sub onMenuEntry(event as object)
    if m.menuKind = "found"
        if event.getData() = 0
            openFindLanguages()
        else
            addSubtitles(event.getData() - 1)
        end if
        return
    end if
    action = m.menuActions[event.getData()]
    if action = "find"
        findSubtitles()
    else if action = "subtitles" or action = "audio"
        openTracks(action)
    else if action = "quality"
        openQualities()
    else if action = "sync"
        closeMenu()
        openSync()
    else if action = "chapters"
        openChapters()
    else if action = "previousChapter" or action = "nextChapter"
        goToChapter(action = "nextChapter")
    else if action = "next"
        ' As the user's remote moves on: the episode keeps its place.
        closeMenu()
        playNext(false)
    else if action = "sleep"
        openSleep()
    else
        openDetails()
    end if
end sub

' The file's subtitles (Off first) or audio (Automatic first), as on the
' title's page, the one playing checked.
sub openTracks(kind as string)
    if m.local <> invalid
        openLocalTracks(kind)
        return
    end if
    tracks = mediaTracks(playingFile())
    m.menuKind = kind
    if kind = "audio"
        m.menuTitle.text = "Audio"
        list = tracks.audio
    else
        m.menuTitle.text = "Subtitles"
        list = tracks.subtitles
    end if
    items = []
    for each label in list.labels
        items.Push({ title: label })
    end for
    m.menuValues = list.values
    m.trackCurrent = list.values[list.current]
    showMenuChoices(items, list.current)
end sub

' A track picked in the menu, or by the user's remote, is saved for the
' show or movie, as on the title's page (PUT /api/items/{id}/streams):
' audio picked goes with the subtitles shown, subtitles with the audio as
' it was picked. The stream then opens again where it was with them,
' since a conversion has the audio and the burned-in pictures it was made
' with.
sub pickTrack(kind as string, index as integer)
    file = playingFile()
    if file = invalid then return
    subtitle = Int(httpNumber(file.selected_subtitle, -1))
    audio = -2
    if kind = "audio"
        audio = index
    else
        subtitle = index
    end if
    mediaPick(m.item, file.id, audio, subtitle)
    ' The person's pick takes over from a cast's audio.
    m.playAudio = -1
    body = tracksStreamsBody(file.id, audio, subtitle)
    showWaiting()
    uiRequest({ method: "PUT", path: "/api/items/" + urlValue(m.itemId) + "/streams", jsonText: FormatJson(body), attempt: m.attempt }, "onTrackPicked")
end sub

' Saved or not, the stream opens again with the tracks picked: the item is
' asked for again, for the server's word on them.
sub onTrackPicked(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if answer.error = "signed_out" then return
    if not answer.ok then uiToast("Couldn't save that choice for next time.", true)
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    stopPictures()
    open(m.itemId, position, invalid, m.mode)
end sub

' The qualities this file is offered, the one chosen checked; after the
' player converted a file that didn't play smoothly, Play original above
' them, as on Android TV.
sub openQualities()
    m.menuKind = "quality"
    m.menuTitle.text = "Quality"
    choice = aqChoice(playbackSection())
    items = []
    m.menuValues = []
    if m.fallback = "struggled" and choice <> "auto"
        items.Push({ title: "Play original" })
        m.menuValues.Push("playOriginal")
    end if
    current = items.Count()
    for each quality in aqMenuFor(playingFile())
        if quality.id = choice then current = items.Count()
        items.Push({ title: quality.label })
        m.menuValues.Push(quality.id)
    end for
    showMenuChoices(items, current)
end sub

sub openChapters()
    m.menuKind = "chapters"
    m.menuTitle.text = "Chapters"
    chapters = detChapters(playingFile())
    items = []
    m.menuValues = []
    for each chapter in chapters
        items.Push({ title: chapter.label })
        m.menuValues.Push(chapter.startMs)
    end for
    showMenuChoices(items, detChapterAt(chapters, m.positionMs))
end sub

' Next chapter, or Previous chapter (detPreviousChapterMs).
sub goToChapter(forward as boolean)
    closeMenu()
    chapters = detChapters(playingFile())
    if not forward
        seekTo(detPreviousChapterMs(chapters, m.positionMs))
        return
    end if
    at = detNextChapter(chapters, m.positionMs)
    if at >= 0 then seekTo(chapters[at].startMs)
end sub

' Off, minutes, or the end of what plays; the one set checked.
sub openSleep()
    m.menuKind = "sleep"
    m.menuTitle.text = "Sleep timer"
    items = []
    m.menuValues = []
    current = 0
    for each choice in sleepChoices(playingKind())
        if choice.minutes = m.sleep.minutes then current = items.Count()
        items.Push({ title: choice.label })
        m.menuValues.Push(choice.minutes)
    end for
    showMenuChoices(items, current)
end sub

sub showMenuChoices(items as object, current as integer)
    m.menuChoices.content = nodesContent(items)
    m.menuChoices.checkedItem = current
    m.menuChoices.jumpToItem = current
    m.menuList.visible = false
    m.menuText.visible = false
    m.menuChoices.visible = true
    m.menuHint.text = ""
    onMenuChoiceFocused()
    m.menuChoices.setFocus(true)
end sub

' A quality's hint, under why the player converted the file, if it did.
sub onMenuChoiceFocused()
    if m.menuKind <> "quality" then return
    at = m.menuChoices.itemFocused
    if at < 0 or at >= m.menuValues.Count() then return
    lines = []
    note = playerFallbackNote(m.fallback)
    if note <> "" then lines.Push(note)
    hint = "Try the original again"
    if m.menuValues[at] <> "playOriginal" then hint = aqHint(m.menuValues[at])
    if hint <> "" then lines.Push(hint)
    typeSet(m.menuHint, lines.Join(Chr(10)))
end sub

' A quality is kept as the Roku's (Settings shows it) and the title opens
' again at it where it was; a chapter is gone to.
sub onMenuChoice(event as object)
    at = event.getData()
    if at < 0 or at >= m.menuValues.Count() then return
    value = m.menuValues[at]
    kind = m.menuKind
    ' A language to look in: the menu stays, and looks there.
    if kind = "findLanguage"
        m.findLanguage = value
        findSubtitles()
        return
    end if
    closeMenu()
    if kind = "chapters"
        seekTo(value)
        return
    end if
    if kind = "subtitles" or kind = "audio"
        if value = m.trackCurrent then return
        if m.local <> invalid
            pickLocal(kind, value)
        else
            pickTrack(kind, value)
        end if
        return
    end if
    if kind = "sleep"
        setSleep(value)
        return
    end if
    if value = "playOriginal"
        playOriginal()
        return
    end if
    if value = aqChoice(playbackSection()) then return
    chooseQuality(value)
end sub

' A quality picked, in the menu or by the user's remote: kept as the
' Roku's, and the title opened again at it where it was, as that quality
' plays (the original as it is), not as the player converted it.
sub chooseQuality(value as string)
    aqChoose(playbackSection(), value)
    ' Picked again, Automatic starts afresh at the original.
    if value = "auto" then keepCeiling(0)
    newTitle()
    m.fallback = ""
    open(m.itemId, livePosition(), m.item, "auto")
end sub

' Play original, after the player converted a file that didn't play
' smoothly: the quality chosen, as it plays, from where it was. Should it
' stand still again, it isn't converted again for this title.
sub playOriginal()
    m.fallback = ""
    open(m.itemId, livePosition(), m.item, "auto")
end sub

' Where the video is now: the Video node's position once it has one.
function livePosition() as integer
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    return position
end function

' --- The sleep timer ---

' What plays, as the sleep timer names its end.
function playingKind() as string
    if m.item = invalid then return ""
    return httpString(m.item.kind)
end function

function clockMs() as double
    return m.clock.TotalMilliseconds() * 1#
end function

' What the timer says now.
function sleepNow() as object
    left = -1#
    if m.played and m.durationMs > 0 then left = (m.durationMs - m.positionMs) * 1#
    return sleepState(m.sleep, clockMs(), left, urlValue(m.itemId))
end function

' Set from the menu (minutes; 0 for Off, -1 for the end of what plays).
sub setSleep(minutes as integer)
    m.sleep = sleepNew(minutes, clockMs())
    hideSleepNotice()
    if minutes = 0 and m.sleepPausedAt < 0
        m.sleepTimer.control = "stop"
    else
        m.sleepTimer.control = "start"
    end if
end sub

' Once a second: the notice in the last 30 s, the pause at the end, and
' the player left 10 minutes into that pause.
sub onSleepTick()
    if m.sleepPausedAt >= 0
        if clockMs() - m.sleepPausedAt >= sleepLeaveAfterMs() then uiNavigate({ back: true })
        return
    end if
    state = sleepNow()
    if state.fire
        m.sleep = sleepNew(0, 0#)
        hideSleepNotice()
        m.sleepPausedAt = clockMs()
        m.video.control = "pause"
        return
    end if
    if state.secondsLeft < 0
        hideSleepNotice()
        return
    end if
    typeSet(m.sleepText, "Pausing in " + urlValue(state.secondsLeft) + " s")
    if not m.sleepNotice.visible
        m.sleepNotice.visible = true
        ' The button takes the focus while nothing else is up.
        if m.video.hasFocus() or m.skip.hasFocus() then m.sleepButtons.setFocus(true)
    end if
end sub

sub hideSleepNotice()
    if m.sleepButtons.isInFocusChain() then m.video.setFocus(true)
    m.sleepNotice.visible = false
end sub

' Keep watching: the same time again, or this title plays to its end and
' the next one's end stops.
sub onKeepWatching()
    sleepExtend(m.sleep, urlValue(m.itemId))
    hideSleepNotice()
end sub

' How the stream plays, and for a conversion what the server makes of it
' (GET /api/playback/{session}): "Asking the server…" until it answers,
' as on the native Samsung app.
sub openDetails()
    m.menuKind = "details"
    m.menuTitle.text = "Playback details"
    m.detailTop = 0
    m.detailsWaiting = m.sessionId <> "" and httpString(m.info.method) <> "direct"
    if m.detailsWaiting
        m.detailLines = ["Asking the server…"]
        showDetailLines()
    else
        showDetails(invalid)
    end if
    m.menuList.visible = false
    m.menuChoices.visible = false
    m.menuText.visible = true
    m.menuHint.text = ""
    m.top.setFocus(true)
    onDetailsTick()
    m.detailsTimer.control = "start"
end sub

' While the details show, a conversion's are asked for again every 2 s, as
' on Android, and the position in them moves on.
sub onDetailsTick()
    if m.menuKind <> "details" or not m.menu.visible
        m.detailsTimer.control = "stop"
        return
    end if
    if m.sessionId <> "" and httpString(m.info.method) <> "direct"
        uiRequest({ method: "GET", path: "/api/playback/" + urlEncode(m.sessionId), session: m.sessionId }, "onDetails")
    else
        showDetails(invalid)
    end if
end sub

' The server's answer; without one, what the player knows itself.
sub onDetails(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId or m.menuKind <> "details" then return
    answer = event.getData()
    if answer.ok and answer.data <> invalid
        showDetails(httpObject(answer.data.transcode))
    else if m.detailsWaiting
        showDetails(invalid)
    end if
end sub

sub showDetails(live as dynamic)
    if m.local <> invalid
        showLocalDetails()
        return
    end if
    if m.info = invalid then return
    details = detPlayback(m.info, playingFile(), m.audioPlaying, live, m.positionMs)
    lines = [details.method]
    if details.why <> "" then lines.Push(details.why)
    lines.Push("")
    for each row in details.rows
        lines.Push(row.label + ": " + row.value)
    end for
    if m.pictureIndex >= 0 then lines.Push("Subtitles: Drawn by Pley")
    if m.watch <> invalid and aqLowered(m.watch) then lines.Push("Quality: " + qualityShown() + " · Lowered for your connection")
    m.detailsWaiting = false
    m.detailLines = lines
    showDetailLines()
end sub

' The details from the line scrolled to, as many as the panel holds; asked
' for again, they stay where they were scrolled to.
sub showDetailLines()
    last = m.detailLines.Count() - 1
    if m.detailTop > last then m.detailTop = last
    if m.detailTop < 0 then m.detailTop = 0
    shown = []
    for i = m.detailTop to last
        shown.Push(m.detailLines[i])
    end for
    typeSet(m.menuText, shown.Join(Chr(10)))
end sub

' Up and Down: a line back, or on while the panel cuts the rest off.
sub scrollDetails(down as boolean)
    if down
        more = m.menuText.isTextEllipsized
        if GetInterface(more, "ifBoolean") = invalid then more = m.detailTop < m.detailLines.Count() - 1
        if not more then return
        m.detailTop = m.detailTop + 1
    else
        if m.detailTop = 0 then return
        m.detailTop = m.detailTop - 1
    end if
    showDetailLines()
end sub

' --- Picture subtitles drawn here (Pictures.brs) ---

' The person's picture subtitles, when the app draws them: their list asked
' for at once, from where the playback starts. Without a key to fetch them
' by, the server burns them in instead.
sub startPictures()
    stopPictures()
    file = contentFile(m.item, m.info.file_id)
    index = picTrack(m.info, file, m.picturesOff)
    if index < 0 then return
    m.pictureIndex = index
    m.picturePicture = picPicture(file, 1920, 1080)
    m.pictureFolder = picFolder(m.info.url, m.global.session.server)
    if m.pictureFolder = ""
        picturesFailed()
        return
    end if
    ' What shows is these pictures: Roku's options only list text tracks.
    m.subtitleShown = index
    m.video.notificationInterval = picNotifySeconds()
    askPictures()
end sub

sub stopPictures()
    m.pictureIndex = -1
    m.pictures = invalid
    m.pictureShown = -1
    ' An answer on its way is for pictures no longer drawn.
    m.pictureAsk = m.pictureAsk + 1
    m.pictureWaiting = false
    m.pictureAgain = false
    m.pictureLastMs = -1
    m.picturesTimer.control = "stop"
    for each frame in m.pictureFrames
        frame.opacity = 0
        frame.uri = ""
    end for
    m.pictureFailed = {}
    m.video.notificationInterval = 1
end sub

' The list of the track's pictures, from where the video is: one request at
' a time, the next a few seconds after the last answer while the track
' isn't whole.
sub askPictures()
    if m.pictureIndex < 0 or m.pictureWaiting or m.sessionId = "" then return
    m.picturesTimer.control = "stop"
    m.pictureWaiting = true
    path = picListPath(m.pictureFolder, m.pictureIndex, m.positionMs)
    ' The session's key opens it: no token.
    uiRequest({ method: "GET", path: path, auth: false, build: "pictures", ask: m.pictureAsk }, "onPictures")
end sub

sub onPictures(event as object)
    if event.getRoSGNode().spec.ask <> m.pictureAsk then return
    m.pictureWaiting = false
    answer = event.getData()
    kind = picAnswerKind(answer)
    if kind = "unavailable"
        picturesFailed()
        return
    end if
    ' A session stopped: the player hears of it itself.
    if kind = "stopped" then return
    if kind = "ok"
        m.pictures = picMerge(m.pictures, answer.data)
        m.pictureShown = -2
        drawPictures()
        if m.pictures.complete then return
    end if
    ' A seek while it was asked for: the pictures where the video is now.
    if m.pictureAgain
        m.pictureAgain = false
        askPictures()
        return
    end if
    m.picturesTimer.control = "start"
end sub

' A seek while the track isn't whole: the pictures where the video is now
' are asked for at once, rather than at the next look.
sub followPictures()
    if m.pictureIndex < 0 then return
    now = m.clock.TotalMilliseconds()
    seeked = picSeeked(m.pictureLastMs, m.pictureLastAt, m.positionMs, now)
    m.pictureLastMs = m.positionMs
    m.pictureLastAt = now
    if not seeked or (m.pictures <> invalid and m.pictures.complete) then return
    if m.pictureWaiting
        m.pictureAgain = true
    else
        askPictures()
    end if
end sub

' The server doesn't make pictures (an older server or transcoder): they
' are burned in from now on, the stream opening again where it is, as on
' the other TVs.
sub picturesFailed()
    m.picturesOff = true
    stopPictures()
    if m.info = invalid then return
    position = m.positionMs
    seconds = m.video.position
    if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
    open(m.itemId, position, m.item, m.mode)
end sub

' The picture for now, while the video plays, at its place on the screen;
' the next one loads meanwhile in the other frame, which then comes to the
' front. Paused or seeking, Roku's bar has the bottom of the screen.
sub drawPictures()
    if m.pictureIndex < 0 then return
    plan = { show: -1, preload: -1 }
    if m.pictures <> invalid then plan = picPlan(m.pictures, m.positionMs, m.syncMs)
    shown = plan.show
    if m.video.state <> "playing" then shown = -1
    base = m.global.session.server + m.pictureFolder
    if shown <> m.pictureShown
        m.pictureShown = shown
        if shown >= 0
            cue = m.pictures.cues[shown]
            uri = base + cue.image
            if m.pictureFrames[m.pictureFront].uri <> uri and m.pictureFrames[1 - m.pictureFront].uri = uri then m.pictureFront = 1 - m.pictureFront
            front = m.pictureFrames[m.pictureFront]
            loadPicture(front, uri)
            ' Not loading, as it failed a moment ago: looked at again next time.
            if front.uri <> uri then m.pictureShown = -2
            place = picBox(cue, m.pictures, picPlane(m.pictures, m.picturePicture, 1920, 1080))
            front.translation = [place.x, place.y]
            front.width = place.width
            front.height = place.height
            front.opacity = 1
        else
            m.pictureFrames[m.pictureFront].opacity = 0
        end if
        m.pictureFrames[1 - m.pictureFront].opacity = 0
    end if
    if plan.preload >= 0 then loadPicture(m.pictureFrames[1 - m.pictureFront], base + m.pictures.cues[plan.preload].image)
end sub

' Has frame load uri, unless its load failed a moment ago (picLoadAgain).
sub loadPicture(frame as object, uri as string)
    if frame.uri = uri then return
    if not picLoadAgain(m.pictureFailed[uri], m.clock.TotalMilliseconds()) then return
    frame.uri = uri
end sub

' A picture that didn't load (a timeout, the server busy) loads again a few
' seconds later, when it shows or comes next.
sub onPictureLoad(event as object)
    if event.getData() <> "failed" then return
    frame = event.getRoSGNode()
    uri = httpString(frame.uri)
    if uri = "" then return
    m.pictureFailed[uri] = m.clock.TotalMilliseconds()
    frame.uri = ""
    if frame.opacity > 0 then m.pictureShown = -2
end sub

' --- Find subtitles… (SubtitleSearch.brs) ---

' The server's results for the file playing, in the person's subtitle
' language, listed in the menu.
sub findSubtitles()
    m.menuKind = "finding"
    m.menuTitle.text = "Find subtitles"
    m.found = []
    showMenuText("Looking for subtitles…")
    m.top.setFocus(true)
    fileId = 0
    if m.info <> invalid then fileId = m.info.file_id
    uiRequest({ method: "GET", path: ssSearchPath(m.itemId, fileId, m.findLanguage), timeoutMs: 30000, session: m.sessionId }, "onSubtitlesFound")
end sub

' The languages to look in, the one looked in checked.
sub openFindLanguages()
    m.menuKind = "findLanguage"
    m.menuTitle.text = "Subtitle language"
    choice = ssLanguageChoices(m.findLanguage)
    items = []
    for each label in choice.labels
        items.Push({ title: label })
    end for
    m.menuValues = choice.values
    showMenuChoices(items, choice.current)
end sub

sub showMenuText(text as string)
    typeSet(m.menuText, text)
    m.menuList.visible = false
    m.menuChoices.visible = false
    m.menuText.visible = true
    m.menuHint.text = "Back goes back."
end sub

sub onSubtitlesFound(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId or m.menuKind <> "finding" then return
    answer = event.getData()
    if not answer.ok
        showMenuText(ssProblem(answer, true))
        return
    end if
    m.found = ssResults(answer.data)
    m.menuKind = "found"
    items = [{ title: ssLanguageEntry(m.findLanguage) }]
    for each result in m.found
        items.Push({ title: result.label })
    end for
    m.menuList.content = nodesContent(items)
    ' The first result in focus, the Language entry above it.
    if m.found.Count() > 0
        m.menuList.jumpToItem = 1
    else
        m.menuList.jumpToItem = 0
    end if
    m.menuText.visible = false
    m.menuChoices.visible = false
    m.menuList.visible = true
    onMenuEntryFocused()
    m.menuList.setFocus(true)
end sub

sub onMenuEntryFocused()
    if m.menuKind <> "found" then return
    at = m.menuList.itemFocused - 1
    if at >= 0 and at < m.found.Count()
        typeSet(m.menuHint, m.found[at].hint)
    else if m.found.Count() = 0
        typeSet(m.menuHint, ssNothingFound(m.findLanguage))
    else
        m.menuHint.text = "The language the search looks in."
    end if
end sub

' The result picked is added on the server, then shown.
sub addSubtitles(at as integer)
    if at < 0 or at >= m.found.Count() or m.info = invalid then return
    m.menuKind = "adding"
    showMenuText("Adding " + m.found[at].label + "…")
    m.top.setFocus(true)
    body = { result: m.found[at].id }
    fileId = httpNumber(m.info.file_id, 0)
    if fileId > 0 then body.file_id = fileId
    uiRequest({ method: "POST", path: "/api/items/" + urlValue(m.itemId) + "/subtitles", json: body, timeoutMs: 60000, session: m.sessionId }, "onSubtitlesAdded")
end sub

' The new track joins the stream's text tracks and shows: the stream opens
' again where it was with it (a new session where the server drew picture
' subtitles onto the video), and it is saved as the person's pick for the
' show or the movie, as a track picked in Roku's options is.
sub onSubtitlesAdded(event as object)
    if event.getRoSGNode().spec.session <> m.sessionId or m.menuKind <> "adding" then return
    answer = event.getData()
    track = invalid
    if answer.ok then track = ssAddedTrack(answer.data)
    if track = invalid
        if answer.ok then answer.message = "The server didn't send the subtitles back."
        showMenuText(ssProblem(answer, false))
        return
    end if
    closeMenu()
    m.info.subtitles = ssWithTrack(m.info.subtitles, track)
    index = Int(httpNumber(track.index, -1))
    file = contentFile(m.item, m.info.file_id)
    burned = -1
    if httpTrue(m.info.burn_subtitles) then burned = Int(httpNumber(m.info.subtitle_index, -1))
    m.subtitleShown = index
    stopPictures()
    body = tracksStreamsBody(m.info.file_id, -2, index)
    uiRequest({ method: "PUT", path: "/api/items/" + urlValue(m.itemId) + "/streams", jsonText: FormatJson(body) }, "")
    if file <> invalid then file.selected_subtitle = index
    stream = contentStream(m.info, m.item, m.global.session.server, m.positionMs)
    m.textTracks = stream.subtitleTracks.Count()
    uiToast("Subtitles added: " + contentSubtitleName(track, httpString(track.language)))
    if tracksNeedNewSession(burned, index)
        position = m.positionMs
        seconds = m.video.position
        if seconds <> invalid and seconds > 0 then position = Int(seconds * 1000)
        open(m.itemId, position, m.item, m.mode)
        return
    end if
    url = httpString(track.url)
    if Left(url, 1) = "/" then url = m.global.session.server + url
    captionsOn()
    reopen(syncTrackUrl(url, m.syncMs))
end sub
