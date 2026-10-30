' The live player (docs/API.md, "Live TV"). Tuning is POST
' /api/live/channels/{id}/watch with this Roku's client_id (the API task's),
' which ends what it watched before, so changing channel holds one
' connection; then its live HLS playlist, asked once by the API task
' (the server answers it once the channel's first segments are made, and
' says plainly why when it can't), then given to the Video node as live
' content. Pause and rewinding within the server's timeshift are the Video
' node's own. Leaving sends DELETE /api/live/sessions/{session}, so the
' connection goes at once.

sub init()
    m.video = m.top.findNode("video")
    m.card = m.top.findNode("card")
    m.cardBack = m.top.findNode("cardBack")
    m.cardInitials = m.top.findNode("cardInitials")
    m.cardLogo = m.top.findNode("cardLogo")
    m.cardTitle = m.top.findNode("cardTitle")
    m.cardLine = m.top.findNode("cardLine")
    m.banner = m.top.findNode("banner")
    m.bannerInitials = m.top.findNode("bannerInitials")
    m.bannerLogo = m.top.findNode("bannerLogo")
    m.bannerChannel = m.top.findNode("bannerChannel")
    m.bannerTitle = m.top.findNode("bannerTitle")
    m.bannerMeta = m.top.findNode("bannerMeta")
    m.bannerTrack = m.top.findNode("bannerTrack")
    m.bannerFill = m.top.findNode("bannerFill")
    m.bannerNext = m.top.findNode("bannerNext")
    m.bannerHint = m.top.findNode("bannerHint")
    m.badge = m.top.findNode("badge")
    m.badgeText = m.top.findNode("badgeText")
    m.star = m.top.findNode("star")
    m.list = m.top.findNode("list")
    m.listChips = m.top.findNode("listChips")
    m.channelList = m.top.findNode("channelList")
    m.listEmpty = m.top.findNode("listEmpty")
    m.notice = m.top.findNode("notice")
    m.noticeTitle = m.top.findNode("noticeTitle")
    m.noticeText = m.top.findNode("noticeText")
    m.noticeButtons = m.top.findNode("noticeButtons")
    m.toast = m.top.findNode("toast")
    m.toastText = m.top.findNode("toastText")
    m.bannerTimer = m.top.findNode("bannerTimer")
    m.zapTimer = m.top.findNode("zapTimer")
    m.keepAlive = m.top.findNode("keepAlive")
    m.clock = m.top.findNode("clock")
    m.listTimer = m.top.findNode("listTimer")
    m.pictureTimer = m.top.findNode("pictureTimer")
    m.toastTimer = m.top.findNode("toastTimer")
    m.twelve = CreateObject("roDeviceInfo").GetClockFormat() <> "24h"
    m.started = false
    ' The channel shown (Live.brs's liveChannel), and the one up and down
    ' lead to before it is tuned.
    m.channel = invalid
    m.zapTo = invalid
    ' The list's channels, by number (up and down go through them): every
    ' channel where the lineup is small, else the group of the channel
    ' first watched; or the favourites. The list's filter ("" until the
    ' API task chose it), the one it chose, and the lineup's size.
    m.channels = []
    m.listFilter = ""
    m.mainFilter = ""
    m.hasFavorites = false
    m.total = 0
    ' Which tuning the answers belong to, the watch asked for last (until
    ' it answers), the session and its playlist.
    m.attempt = 0
    m.watchRequest = invalid
    m.sessionId = ""
    m.playlist = ""
    m.played = false
    m.paused = false
    ' When the stream was tuned again after breaking (playerMayRestart).
    m.restarts = []
    m.span = CreateObject("roTimespan")
    m.trouble = invalid
    m.video.notificationInterval = 1
    m.video.observeField("state", "onState")
    m.video.observeField("position", "onPosition")
    m.noticeButtons.observeField("buttonSelected", "onNoticeButton")
    m.channelList.observeField("itemSelected", "onListSelected")
    m.listChips.observeField("chosen", "onListChip")
    m.bannerTimer.observeField("fire", "onBannerTimer")
    m.zapTimer.observeField("fire", "onZapTimer")
    m.keepAlive.observeField("fire", "onKeepAlive")
    m.clock.observeField("fire", "onClock")
    m.listTimer.observeField("fire", "onListTimer")
    m.pictureTimer.observeField("fire", "onPictureTimer")
    m.toastTimer.observeField("fire", "onToastTimer")
    m.cardLogo.observeField("loadStatus", "onLogos")
    m.bannerLogo.observeField("loadStatus", "onLogos")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        finish()
        return
    end if
    if m.started then return
    m.started = true
    ' A channel takes over from the music.
    m.global.music.control = "stop"
    ' The playlist and its segments carry the token: a refreshed one goes
    ' to the Video node's agent at once.
    m.global.session.observeFieldScoped("accessToken", "onToken")
    m.clock.control = "start"
    m.listTimer.control = "start"
    channel = m.top.params.channel
    if GetInterface(channel, "ifAssociativeArray") = invalid then return
    askChannels(false, channel.id)
    tune(channel)
end sub

sub onToken()
    m.video.setHttpAgent(uiHttpAgent())
end sub

function nowSeconds() as integer
    return CreateObject("roDateTime").AsSeconds()
end function

' --- Channels ---

' The list's channels with what's on, made by the API task from the
' lineup it keeps (fetched again with refresh): on opening, and each
' minute where the lineup is small.
sub askChannels(refresh as boolean, around as dynamic)
    uiRequest({ op: "liveList", refresh: refresh, filter: m.listFilter, around: around }, "onChannels")
end sub

sub onListTimer()
    if m.total <= liveMany() then askChannels(true, invalid)
end sub

sub onChannels(event as object)
    answer = event.getData()
    if not answer.ok then return
    request = event.getRoSGNode()
    ' The list as asked for: the filter may have changed since.
    if request.spec.filter <> m.listFilter then return
    data = answer.data
    m.channels = data.channels
    m.total = data.total
    m.listFilter = data.filter
    if m.mainFilter = "" and data.filter <> "favorites" then m.mainFilter = data.filter
    m.hasFavorites = false
    for each choice in data.choices
        if choice.value = "favorites" then m.hasFavorites = true
    end for
    focusedId = invalid
    if m.channelList.content <> invalid
        item = m.channelList.content.getChild(m.channelList.itemFocused)
        if item <> invalid then focusedId = item.channelId
    end if
    content = request.content
    if content = invalid then content = CreateObject("roSGNode", "ContentNode")
    m.channelList.content = content
    markCurrent()
    ' The channel shown, with what's on now.
    if m.channel <> invalid
        fresh = liveStep(m.channels, m.channel.id, 0)
        if fresh <> invalid and fresh.id = m.channel.id
            m.channel = fresh
            showBanner(false)
        end if
    end if
    showListChips()
    m.listEmpty.visible = content.getChildCount() = 0
    m.listEmpty.text = "No favourites yet: the Options key (*) on a channel adds it."
    if focusedId = invalid and m.channel <> invalid then focusedId = m.channel.id
    at = liveIndexOf(m.channels, focusedId)
    if at >= 0 then m.channelList.jumpToItem = at
end sub

' The channel playing stands out in the list.
sub markCurrent()
    content = m.channelList.content
    if content = invalid then return
    for i = 0 to content.getChildCount() - 1
        item = content.getChild(i)
        shown = m.channel <> invalid and item.channelId = m.channel.id
        if item.current <> shown then item.current = shown
    end for
end sub

' The list's filters as chips: every channel (or the group), and the
' favourites when there are some.
sub showListChips()
    main = m.mainFilter
    if main = "" then main = "all"
    m.listFilters = [main]
    chips = [{ label: liveFilterLabel(main), width: 120 + 15 * Len(liveFilterLabel(main)) }]
    if m.hasFavorites or m.listFilter = "favorites"
        m.listFilters.Push("favorites")
        chips.Push({ label: "Favourites", width: 240 })
    end if
    if chips[0].width > 420 then chips[0].width = 420
    if chips.Count() < 2
        m.listChips.visible = false
        return
    end if
    m.listChips.chips = chips
    m.listChips.current = 0
    if m.listFilter = "favorites" then m.listChips.current = 1
    m.listChips.visible = true
end sub

sub onListChip(event as object)
    at = event.getData()
    if at < 0 or at >= m.listFilters.Count() then return
    wanted = m.listFilters[at]
    if wanted = m.listFilter then return
    m.listFilter = wanted
    m.listChips.current = at
    askChannels(false, invalid)
end sub

sub openList()
    hideBanner()
    m.list.visible = true
    if m.channel <> invalid
        at = liveIndexOf(m.channels, m.channel.id)
        if at >= 0 then m.channelList.jumpToItem = at
    end if
    if m.channelList.content <> invalid and m.channelList.content.getChildCount() > 0
        m.channelList.setFocus(true)
    else if m.listChips.visible
        m.listChips.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

sub closeList()
    m.list.visible = false
    if m.notice.visible
        m.noticeButtons.setFocus(true)
    else
        m.video.setFocus(true)
    end if
end sub

sub onListSelected(event as object)
    item = m.channelList.content.getChild(event.getData())
    if item = invalid then return
    channel = liveStep(m.channels, item.channelId, 0)
    closeList()
    if channel = invalid or channel.id <> item.channelId then return
    if m.channel <> invalid and channel.id = m.channel.id and not m.notice.visible
        showBanner(true)
        return
    end if
    tune(channel)
end sub

' The Options key in the list: the channel in focus in or out of the
' favourites (PUT or DELETE /api/live/channels/{id}/favorite).
sub toggleFavorite()
    item = m.channelList.content.getChild(m.channelList.itemFocused)
    if item = invalid then return
    favorite = not item.favorite
    method = "DELETE"
    if favorite then method = "PUT"
    item.favorite = favorite
    uiRequest({ method: method, path: "/api/live/channels/" + urlValue(item.channelId) + "/favorite", favorite: favorite, name: item.title, channelId: item.channelId }, "onFavorite")
end sub

sub onFavorite(event as object)
    answer = event.getData()
    spec = event.getRoSGNode().spec
    if not answer.ok
        if not uiFailed(answer) then showToast(answer.message)
        askChannels(false, invalid)
        return
    end if
    uiRequest({ op: "liveFavorite", channelId: spec.channelId, favorite: spec.favorite }, "")
    if spec.favorite
        showToast(spec.name + " is in your favourites")
    else
        showToast(spec.name + " is out of your favourites")
    end if
    askChannels(false, invalid)
end sub

' --- Tuning ---

' Up and down: the channel's banner shows at once, over the one playing,
' and it is tuned once the presses stop.
sub zap(by as integer)
    from = m.zapTo
    if from = invalid then from = m.channel
    if from = invalid then return
    target = liveStep(m.channels, from.id, by)
    if target = invalid then return
    m.zapTo = target
    showBanner(true)
    m.bannerTimer.control = "stop"
    m.zapTimer.control = "stop"
    m.zapTimer.control = "start"
end sub

sub onZapTimer()
    target = m.zapTo
    m.zapTo = invalid
    if target = invalid then return
    if m.channel <> invalid and target.id = m.channel.id and m.sessionId <> ""
        ' Back where it started.
        showBanner(true)
        return
    end if
    tune(target)
end sub

' Watches channel: the session before it ends (the server ends it too,
' by the client_id), its logo shows while it starts.
sub tune(channel as object)
    m.attempt = m.attempt + 1
    abandonWatch()
    leave()
    m.zapTo = invalid
    m.zapTimer.control = "stop"
    m.channel = channel
    m.played = false
    m.paused = false
    m.trouble = invalid
    m.keepAlive.control = "stop"
    m.pictureTimer.control = "stop"
    m.video.control = "stop"
    m.notice.visible = false
    showCard(channel, "")
    showBanner(true)
    markCurrent()
    if not m.list.visible then m.top.setFocus(true)
    m.watchRequest = uiRequest({ op: "liveWatch", channelId: channel.id, attempt: m.attempt }, "onWatch")
end sub

' The watch asked for isn't wanted any more: the API task ends it if it
' comes.
sub abandonWatch()
    if m.watchRequest = invalid then return
    m.watchRequest.abandoned = true
    m.watchRequest = invalid
end sub

' DELETE /api/live/sessions/{session}: the connection goes at once.
sub leave()
    m.keepAlive.control = "stop"
    if m.sessionId = "" then return
    uiRequest({ method: "DELETE", path: "/api/live/sessions/" + urlEncode(m.sessionId) }, "")
    m.sessionId = ""
    m.playlist = ""
end sub

function isCurrent(event as object) as boolean
    return event.getRoSGNode().spec.attempt = m.attempt
end function

sub onWatch(event as object)
    if not isCurrent(event) then return
    m.watchRequest = invalid
    answer = event.getData()
    if not answer.ok or httpObject(answer.data) = invalid
        if answer.error = "signed_out" then return
        if answer.error = "network"
            uiUnreachable({ to: "livePlayer", params: { channel: m.channel } })
            return
        end if
        trouble(liveTrouble(answer, m.channel.name))
        return
    end if
    m.sessionId = httpString(answer.data.session_id)
    m.playlist = httpString(answer.data.url)
    fresh = liveChannel(answer.data.channel, m.global.session.server, liveLogoWidth())
    if fresh <> invalid and fresh.id = m.channel.id then m.channel = fresh
    showCard(m.channel, "")
    showBanner(true)
    ' The playlist once, waited for: the server answers it once the
    ' channel's first segments are made, or says why not.
    uiRequest({ method: "GET", path: m.playlist, timeoutMs: 50000, attempt: m.attempt }, "onPlaylist")
end sub

sub onPlaylist(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if not answer.ok
        if answer.error = "signed_out" then return
        if answer.error = "network"
            uiUnreachable({ to: "livePlayer", params: { channel: m.channel } })
            return
        end if
        leave()
        trouble(liveTrouble(answer, m.channel.name))
        return
    end if
    play()
end sub

' The live stream in the Video node, its token in the agent's header.
sub play()
    content = CreateObject("roSGNode", "ContentNode")
    content.url = m.global.session.server + m.playlist
    content.streamFormat = "hls"
    content.title = liveTitle(m.channel)
    content.live = true
    if LCase(Left(content.url, 8)) = "https://" then nodesFill(content, { HttpCertificatesFile: "common:/certs/ca-bundle.crt" })
    m.video.setHttpAgent(uiHttpAgent())
    m.video.content = content
    m.video.control = "play"
    if not m.list.visible then m.video.setFocus(true)
end sub

sub onState(event as object)
    state = event.getData()
    if state = "playing"
        m.keepAlive.control = "stop"
        if not m.played
            m.played = true
            m.card.visible = false
            showBanner(true)
            ' A channel without pictures (radio) keeps its card.
            m.pictureTimer.control = "start"
        else if m.paused
            showBanner(true)
        end if
        m.paused = false
        updateBadge()
        return
    end if
    if state = "paused"
        m.paused = true
        ' Paused, the Video node asks for nothing: the server is told the
        ' channel is still watched.
        m.keepAlive.control = "start"
        showBanner(false)
        m.bannerTimer.control = "stop"
        updateBadge()
    else if state = "error" or state = "finished"
        broken()
    end if
end sub

' Paused for 20 s: the playlist again, so the server keeps the channel.
sub onKeepAlive()
    if m.playlist = "" or not m.paused
        m.keepAlive.control = "stop"
        return
    end if
    uiRequest({ method: "GET", path: m.playlist, timeoutMs: 10000 }, "")
end sub

sub onPictureTimer()
    if not m.played or m.channel = invalid then return
    video = httpString(m.video.videoFormat)
    audio = httpString(m.video.audioFormat)
    if video = "" and audio <> ""
        showCard(m.channel, "radio")
    end if
end sub

sub onPosition()
    if m.banner.visible then updateBadge()
end sub

' The stream broke: what the server says of its playlist says what to do.
' Gone or refused, the player says so; otherwise the channel is tuned
' again, a few times at most.
sub broken()
    if m.sessionId = "" or m.trouble <> invalid then return
    m.video.control = "stop"
    showCard(m.channel, "")
    uiRequest({ method: "GET", path: m.playlist, timeoutMs: 50000, attempt: m.attempt, check: true }, "onChecked")
end sub

sub onChecked(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if answer.error = "signed_out" then return
    if answer.error = "network"
        uiUnreachable({ to: "livePlayer", params: { channel: m.channel } })
        return
    end if
    if answer.status = 410 or answer.status = 403 or answer.status = 503
        leave()
        trouble(liveTrouble(answer, m.channel.name))
        return
    end if
    nowMs = m.span.TotalMilliseconds()
    if not playerMayRestart(m.restarts, nowMs)
        leave()
        trouble({ kind: "other", title: m.channel.name + " stopped", message: "It stopped and couldn't get going again.", retry: true })
        return
    end if
    m.restarts.Push(nowMs)
    tune(m.channel)
end sub

' --- What shows ---

' The channel's logo (or its name) in the middle, over black: while it
' tunes, and over a channel without pictures ("radio").
sub showCard(channel as object, kind as string)
    m.cardTitle.text = liveTitle(channel)
    on = liveOnNow(channel, nowSeconds())
    line = "Tuning…"
    if kind = "radio"
        line = "Radio"
        if on <> invalid then line = on.title
    else if on <> invalid
        line = on.title + "   ·   Tuning…"
    end if
    m.cardLine.text = line
    m.cardInitials.text = channel.name
    m.cardLogo.uri = channel.logo
    onLogos()
    m.card.visible = true
end sub

' Names show where no logo does (logos are often see-through).
sub onLogos()
    m.cardInitials.visible = m.cardLogo.uri = "" or m.cardLogo.loadStatus = "failed"
    m.bannerInitials.visible = m.bannerLogo.uri = "" or m.bannerLogo.loadStatus = "failed"
end sub

' The banner: the channel, what's on with when and how long it has left,
' how far along, what's next, and the Live badge. With hides, it goes
' after 5 s (not while paused).
sub showBanner(hides as boolean)
    channel = m.zapTo
    if channel = invalid then channel = m.channel
    if channel = invalid then return
    now = nowSeconds()
    on = liveOnNow(channel, now)
    coming = liveComingUp(channel, now)
    m.bannerChannel.text = liveTitle(channel)
    m.bannerInitials.text = channel.name
    m.bannerLogo.uri = channel.logo
    onLogos()
    if on <> invalid
        m.bannerTitle.text = on.title
        meta = [liveSpan(on, m.twelve), liveTimeLeft(on, now)]
        if on.line <> "" then meta.Push(on.line)
        if on.badges <> "" then meta.Push(on.badges)
        m.bannerMeta.text = meta.Join("   ·   ")
        m.bannerFill.width = Int(1100 * liveProgress(on, now))
        m.bannerTrack.visible = true
        m.bannerFill.visible = true
    else
        m.bannerTitle.text = channel.name
        m.bannerMeta.text = "Nothing in the guide now"
        if not channel.hasGuide then m.bannerMeta.text = "No guide for this channel"
        m.bannerTrack.visible = false
        m.bannerFill.visible = false
    end if
    m.bannerNext.text = ""
    if coming <> invalid then m.bannerNext.text = "Next   " + liveClock(coming.start, m.twelve) + "   " + coming.title
    m.star.text = ""
    if channel.favorite then m.star.text = "★ Favourite"
    m.bannerHint.text = "Up and down: channels   ·   Left: channel list   ·   Play: pause   ·   Rewind and fast forward: within what was recorded"
    updateBadge()
    if m.list.visible or m.notice.visible then return
    m.banner.visible = true
    m.bannerTimer.control = "stop"
    if hides and not m.paused then m.bannerTimer.control = "start"
end sub

sub hideBanner()
    m.bannerTimer.control = "stop"
    m.banner.visible = false
end sub

sub onBannerTimer()
    if not m.paused and m.zapTo = invalid then hideBanner()
end sub

' LIVE in red at the edge, how far behind in grey otherwise.
sub updateBadge()
    badge = liveBadge(m.video.position, m.video.duration, m.paused)
    if not m.played then badge = { live: true, text: "LIVE" }
    m.badgeText.text = badge.text
    if badge.live
        m.badge.color = "0xEB6B70FF"
    else
        m.badge.color = "0x2E3440FF"
    end if
end sub

' Each 15 s: what's on moves on.
sub onClock()
    if m.banner.visible then showBanner(false)
end sub

sub showToast(text as string)
    m.toastText.text = text
    m.toast.visible = true
    m.toastTimer.control = "stop"
    m.toastTimer.control = "start"
end sub

sub onToastTimer()
    m.toast.visible = false
end sub

' A watching that didn't start or stopped: what the server said, with Try
' again where it may help, Other channels, and Back.
sub trouble(problem as object)
    m.trouble = problem
    m.video.control = "stop"
    m.card.visible = false
    hideBanner()
    m.noticeTitle.text = problem.title
    m.noticeText.text = problem.message
    buttons = []
    if problem.retry then buttons.Push("Try again")
    buttons.Push("Other channels")
    buttons.Push("Back")
    m.noticeButtons.buttons = buttons
    m.notice.visible = true
    if not m.list.visible then m.noticeButtons.setFocus(true)
end sub

sub onNoticeButton(event as object)
    label = m.noticeButtons.buttons[event.getData()]
    if label = "Try again"
        m.restarts = []
        tune(m.channel)
    else if label = "Other channels"
        openList()
    else
        uiNavigate({ back: true })
    end if
end sub

' Leaving: the session ends, the token's observer goes.
sub finish()
    m.attempt = m.attempt + 1
    abandonWatch()
    leave()
    m.video.control = "stop"
    for each timer in [m.bannerTimer, m.zapTimer, m.keepAlive, m.clock, m.listTimer, m.pictureTimer, m.toastTimer]
        timer.control = "stop"
    end for
    if m.started then m.global.session.unobserveFieldScoped("accessToken")
end sub

' Up and down change channel (Up: the next number); Left opens the channel
' list, Right and OK show what's on. In the list: the Options key adds to
' or takes out of the favourites, Back and Right close it. Back with the
' banner showing hides it; otherwise it leaves (the scene).
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.list.visible
        if key = "back" or key = "right"
            closeList()
            return true
        end if
        if key = "options" and m.channelList.isInFocusChain()
            toggleFavorite()
            return true
        end if
        if key = "up" and m.channelList.isInFocusChain() and m.listChips.visible
            m.listChips.focusIndex = m.listChips.current
            m.listChips.setFocus(true)
            return true
        end if
        if key = "down" and m.listChips.isInFocusChain() and m.channelList.content <> invalid and m.channelList.content.getChildCount() > 0
            m.channelList.setFocus(true)
            return true
        end if
        return true
    end if
    if m.notice.visible then return false
    if key = "up"
        zap(1)
        return true
    end if
    if key = "down"
        zap(-1)
        return true
    end if
    if key = "left"
        openList()
        return true
    end if
    if key = "right" or key = "OK"
        if m.banner.visible and not m.paused
            hideBanner()
        else
            showBanner(true)
        end if
        return true
    end if
    if key = "back" and m.banner.visible and not m.paused and m.zapTo = invalid
        hideBanner()
        return true
    end if
    return false
end function
