' The live player (docs/API.md, "Live TV"). Tuning is POST
' /api/live/channels/{id}/watch with this Roku's client_id (the API task's),
' which ends what it watched before, so changing channel holds one
' connection; then its live HLS playlist, asked once by the API task
' (the server answers it once the channel's first segments are made, and
' says plainly why when it can't), then given to the Video node as live
' content, from its live edge. Live is live: the Video node's own controls
' are off, and nothing pauses, rewinds or skips. Up and down
' change channel within the category the player was opened from (or the
' one a channel was last chosen in from the list, which can list another
' category or a search; a search's, every channel), a page of it at a time (GET
' /api/live/browse?around=), round its ends. Leaving
' sends DELETE /api/live/sessions/{session}, so the connection goes at
' once, and Live TV opens back at the channel watched. The user's remote,
' on another device, changes channel as up and down do (Remote.brs), and
' watching stopped from elsewhere stops here, saying who stopped it.

sub init()
    m.video = m.top.findNode("video")
    m.waiting = m.top.findNode("waiting")
    m.card = m.top.findNode("card")
    m.cardInitials = m.top.findNode("cardInitials")
    m.cardLogo = m.top.findNode("cardLogo")
    m.tuning = m.top.findNode("tuning")
    m.cardKind = m.top.findNode("cardKind")
    m.cardTitle = m.top.findNode("cardTitle")
    m.cardLine = m.top.findNode("cardLine")
    m.banner = m.top.findNode("banner")
    m.bannerInitials = m.top.findNode("bannerInitials")
    m.bannerLogo = m.top.findNode("bannerLogo")
    m.bannerChannel = m.top.findNode("bannerChannel")
    m.bannerBadge = m.top.findNode("bannerBadge")
    m.bannerTitle = m.top.findNode("bannerTitle")
    m.bannerMeta = m.top.findNode("bannerMeta")
    m.bannerTrack = m.top.findNode("bannerTrack")
    m.bannerFill = m.top.findNode("bannerFill")
    m.bannerNext = m.top.findNode("bannerNext")
    m.controls = m.top.findNode("controls")
    m.star = m.top.findNode("star")
    m.list = m.top.findNode("list")
    m.listTitle = m.top.findNode("listTitle")
    m.listRange = m.top.findNode("listRange")
    m.channelList = m.top.findNode("channelList")
    m.categoryList = m.top.findNode("categoryList")
    m.listEmpty = m.top.findNode("listEmpty")
    m.listHint = m.top.findNode("listHint")
    typeSet(m.listHint, listHintText())
    m.notice = m.top.findNode("notice")
    m.noticeCard = m.top.findNode("noticeCard")
    m.noticeBack = m.top.findNode("noticeBack")
    m.noticeInitials = m.top.findNode("noticeInitials")
    m.noticeLogo = m.top.findNode("noticeLogo")
    m.noticeTitle = m.top.findNode("noticeTitle")
    m.noticeLead = m.top.findNode("noticeLead")
    m.noticeRows = m.top.findNode("noticeRows")
    m.noticeNote = m.top.findNode("noticeNote")
    m.noticeText = m.top.findNode("noticeText")
    m.noticeChoices = m.top.findNode("noticeChoices")
    ' What takes the card's focus, top to bottom (each Watch here instead,
    ' then the buttons), and the one that has it.
    m.troubleStops = []
    m.troubleAt = 0
    m.bannerTimer = m.top.findNode("bannerTimer")
    m.okTimer = m.top.findNode("okTimer")
    m.zapTimer = m.top.findNode("zapTimer")
    m.clock = m.top.findNode("clock")
    m.pictureTimer = m.top.findNode("pictureTimer")
    m.retryTimer = m.top.findNode("retryTimer")
    m.stallTimer = m.top.findNode("stallTimer")
    m.stallTimer.observeField("fire", "onStall")
    ' Waiting for data since the timer started (the spinner shows after it).
    m.stalling = false
    for each spinner in [m.waiting, m.tuning]
        spinner.poster.uri = "pkg:/images/spinner.png"
        spinner.poster.width = 64
        spinner.poster.height = 64
    end for
    m.twelve = CreateObject("roDeviceInfo").GetClockFormat() <> "24h"
    m.started = false
    ' OK went down with nothing over the picture, timed by okTimer: let go
    ' first, it opens the channel list; still down, it was held and shows
    ' the bar, whose OK the rest of that press isn't (okHeld).
    m.okTiming = false
    m.okHeld = false
    ' The channel shown (Live.brs's liveChannel), and the one up and down
    ' lead to before it is tuned.
    m.channel = invalid
    m.zapTo = invalid
    ' Channels picked (the list, Watch again; Live.brs's liveTunePicks), so
    ' a step waiting for a page can tell one was chosen meanwhile, the same
    ' channel again too.
    m.picks = 0
    ' The category up and down go through (the one the player was opened
    ' from, or the one a channel was last chosen in from the list), the
    ' page of it held ({ total, offset, channels }) and its rows, and the
    ' asking an answer must be for.
    m.category = "all"
    m.categoryName = "All channels"
    m.page = invalid
    m.pageContent = invalid
    m.pageAsked = 0
    m.asks = 0
    ' What the list shows: that category, unless another was chosen in it
    ' or a search made there, with its page, the asking it waits for, and
    ' whether it shows a search made there, with the category before it.
    m.listCategory = "all"
    m.listName = "All channels"
    m.listPage = invalid
    m.listAsked = 0
    m.listSearched = false
    m.listBefore = "all"
    ' Choosing another category in the list: the groups (GET
    ' /api/live/groups, once), the entries listed, the one in focus, the
    ' region open.
    m.summary = invalid
    m.groups = []
    m.railEntries = []
    m.railFocused = 0
    m.open = ""
    ' A page that failed ({ offset, around }) and when (m.span's ms): asked
    ' for again once 10 s have passed, as the list moves or Up or Down is
    ' pressed.
    m.failedPage = invalid
    m.failedAt = -1
    m.failedCategory = ""
    m.span = CreateObject("roTimespan")
    ' When the channel playing was last asked for again (m.span's ms).
    m.refreshedAt = -1
    ' Which tuning the answers belong to, the watch asked for last (until
    ' it answers), the session and its playlist.
    m.attempt = 0
    m.watchRequest = invalid
    m.sessionId = ""
    m.playlist = ""
    m.played = false
    ' The session of the channel tuned, kept after it failed (until another
    ' tuning), so a stop from elsewhere that comes after its 410 still says
    ' who stopped it.
    m.watched = ""
    ' The times the channel was tuned again after failing, since it was
    ' tuned or Try again was chosen (liveRetryDelay), and the tuning the
    ' retry waiting belongs to.
    m.retries = 0
    m.retryAttempt = 0
    m.trouble = invalid
    m.video.notificationInterval = 1
    m.video.observeField("state", "onState")
    m.noticeChoices.observeField("chosen", "onNoticeButton")
    m.channelList.observeField("itemSelected", "onListSelected")
    m.categoryList.observeField("itemSelected", "onCategorySelected")
    m.categoryList.observeField("itemFocused", "onCategoryFocused")
    m.bannerTimer.observeField("fire", "onBannerTimer")
    m.okTimer.observeField("fire", "onOkHeld")
    m.controls.observeField("chosen", "onControl")
    ' Moving between them counts as a touch: the bar stays.
    m.controls.observeField("focusIndex", "pokeBanner")
    m.zapTimer.observeField("fire", "onZapTimer")
    m.clock.observeField("fire", "onClock")
    m.pictureTimer.observeField("fire", "onPictureTimer")
    m.retryTimer.observeField("fire", "onRetryTimer")
    m.cardLogo.observeField("loadStatus", "onLogos")
    m.bannerLogo.observeField("loadStatus", "onLogos")
    m.noticeLogo.observeField("loadStatus", "onLogos")
    m.top.observeField("active", "onActive")
    m.top.observeField("liveTakenOver", "onTakenOver")
    m.top.observeField("serverStopped", "onServerStopped")
    m.top.observeField("playerCommand", "onPlayerCommand")
end sub

sub onActive()
    if not m.top.active
        finish()
        return
    end if
    if m.started then return
    m.started = true
    ' A channel takes over from the music.
    m.global.music.control = "suspend"
    ' The playlist and its segments carry the token: a refreshed one goes
    ' to the Video node's agent at once.
    m.global.session.observeFieldScoped("accessToken", "onToken")
    m.clock.control = "start"
    params = m.top.params
    category = httpString(params.category)
    ' From a search (Search's Live TV row, say): every channel.
    if category <> "" then m.category = liveZapCategory(category)
    channel = params.channel
    if GetInterface(channel, "ifAssociativeArray") = invalid then return
    m.categoryName = httpString(params.categoryName)
    if m.categoryName = "" or Left(category, 7) = "search:" then m.categoryName = liveCategoryName(m.category, [])
    m.listCategory = m.category
    m.listName = m.categoryName
    typeSet(m.listTitle, m.categoryName)
    askPage(m.category, 0, aroundIn(m.category, channel), "")
    tune(channel)
end sub

sub onToken()
    m.video.setHttpAgent(uiHttpAgent())
end sub

function nowSeconds() as integer
    return CreateObject("roDateTime").AsSeconds()
end function

' --- The category's channels ---

' Where a category's page is asked around: the channel. Recently watched
' changes with every channel watched: its order is the one it had as it
' was asked for, from its top.
function aroundIn(category as string, channel as dynamic) as integer
    if channel = invalid or category = "recent" then return 0
    return channel.id
end function

' A page of a category (from offset, or around a channel): for up and
' down when it is the category they go through (then pick "first" or
' "last" of it is the channel they lead to), for the list when it is the
' one the list shows (pick "top" or "bottom": where its focus goes).
' Within 10 s of a page of the category that failed, nothing more of it is
' asked: the other categories, the list's or up and down's, go on. from:
' the channel up or down stepped from, for a pick.
sub askPage(category as string, offset as integer, around as integer, pick as string, from = 0 as integer)
    if category = m.failedCategory and not liveMayAskAgain(m.failedAt, m.span.TotalMilliseconds()) then return
    m.asks = m.asks + 1
    if category = m.category then m.pageAsked = m.asks
    if category = m.listCategory then m.listAsked = m.asks
    uiRequest({ op: "livePage", kind: "list", category: category, offset: offset, limit: livePageSize(), around: around, asked: m.asks, pick: pick, from: from, picks: m.picks }, "onPage")
end sub

sub onPage(event as object)
    request = event.getRoSGNode()
    spec = request.spec
    zapping = spec.category = m.category and spec.asked = m.pageAsked
    listed = spec.category = m.listCategory and spec.asked = m.listAsked
    if not zapping and not listed then return
    answer = event.getData()
    if not answer.ok
        ' Watching goes on; the list says why it is empty.
        m.failedPage = { category: spec.category, offset: spec.offset, around: spec.around }
        m.failedAt = m.span.TotalMilliseconds()
        m.failedCategory = spec.category
        if listed and m.listPage = invalid then typeSet(m.listRange, answer.message)
        return
    end if
    if spec.category = m.failedCategory
        m.failedPage = invalid
        m.failedAt = -1
        m.failedCategory = ""
    end if
    page = answer.data
    content = request.content
    if content = invalid then content = CreateObject("roSGNode", "ContentNode")
    if zapping
        m.page = page
        m.pageContent = content
        ' The channel shown, with what's on now.
        at = -1
        if m.channel <> invalid then at = liveIndexOf(page.channels, m.channel.id)
        if at >= 0
            m.channel = page.channels[at]
            if m.banner.visible then drawBanner()
        end if
        ' The step waited for this page: it goes on only from where it
        ' started, and while watching goes on, so it can't undo a channel
        ' chosen meanwhile.
        if stepStillFrom(spec.from, spec.picks)
            if spec.pick = "first" and page.channels.Count() > 0 then zapTo(page.channels[0])
            if spec.pick = "last" and page.channels.Count() > 0 then zapTo(page.channels[page.channels.Count() - 1])
        end if
    end if
    if listed then showListPage(page, content, spec.pick)
end sub

' A page in the list, its focus on the channel playing when it is there,
' else at its top, or at its top or bottom as pick says (paged to).
sub showListPage(page as object, content as object, pick as string)
    m.listPage = page
    m.channelList.content = content
    showListEmpty()
    markCurrent()
    if not m.categoryList.visible then typeSet(m.listRange, liveRange(page.offset, page.channels.Count(), page.total))
    at = -1
    if m.channel <> invalid then at = liveIndexOf(page.channels, m.channel.id)
    if at < 0 or pick = "top" then at = 0
    if pick = "bottom" then at = content.getChildCount() - 1
    if at >= 0 and at < content.getChildCount() then m.channelList.jumpToItem = at
end sub

' A page of the list without channels says so, as on Android: "No
' channels in this group.", "No channels match"…
sub showListEmpty()
    empty = m.listPage <> invalid and m.listPage.channels.Count() = 0 and not m.categoryList.visible
    if empty then typeSet(m.listEmpty, liveEmpty(m.listCategory).title)
    m.listEmpty.visible = empty
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

' The list opens on the category up and down go through, at the channel
' playing, as on Android: another chosen in it last time, or a search,
' isn't kept.
sub openList()
    forgetOk()
    hideBanner()
    m.listSearched = false
    if m.listCategory <> m.category
        m.listCategory = m.category
        m.listAsked = m.pageAsked
        m.listPage = invalid
        m.channelList.content = invalid
        m.listRange.text = ""
        showListEmpty()
        if m.page <> invalid then showListPage(m.page, m.pageContent, "")
    end if
    m.listName = m.categoryName
    typeSet(m.listTitle, m.listName)
    if m.page = invalid then askAgain(m.category)
    m.list.visible = true
    if m.channel <> invalid and m.listPage <> invalid
        at = liveIndexOf(m.listPage.channels, m.channel.id)
        if at >= 0 then m.channelList.jumpToItem = at
    end if
    m.channelList.setFocus(true)
end sub

sub closeList()
    closeCategories()
    m.list.visible = false
    ' The list gives the focus up: its parent taking it may not.
    m.channelList.setFocus(false)
    if m.notice.visible
        focusTrouble()
    else
        m.top.setFocus(true)
    end if
end sub

' Whether channel up or down from the channel with id may still go on
' (Live.brs's liveStepGoesOn has the rule).
function stepStillFrom(id as dynamic, picks as dynamic) as boolean
    from = m.zapTo
    if from = invalid then from = m.channel
    shownId = invalid
    if from <> invalid then shownId = from.id
    return liveStepGoesOn(id, picks, m.picks, shownId, m.trouble <> invalid, m.top.active)
end function

' The page that failed, when it was category's, asked for again.
sub askAgain(category as string)
    if m.failedPage <> invalid and m.failedPage.category = category then askPage(category, m.failedPage.offset, m.failedPage.around, "")
end sub

' Past the list's ends, the page after it (or before), round the ends of
' its category.
sub listPage(by as integer)
    page = m.listPage
    if page = invalid or page.total <= page.channels.Count() then return
    size = livePageSize()
    pages = Int((page.total + size - 1) / size)
    pageNo = Int(page.offset / size)
    pick = "top"
    if by < 0 then pick = "bottom"
    askPage(m.listCategory, ((pageNo + by + pages) mod pages) * size, 0, pick)
end sub

sub onListSelected(event as object)
    page = m.listPage
    if page = invalid then return
    at = event.getData()
    if at < 0 or at >= page.channels.Count() then return
    channel = page.channels[at]
    ' Chosen in another category: up and down go through that one from
    ' now on; chosen in a search, through every channel, as on Android,
    ' around the channel chosen.
    zapping = liveZapCategory(m.listCategory)
    if zapping <> m.listCategory
        m.category = zapping
        m.categoryName = liveCategoryName(zapping, m.groups)
        m.page = invalid
        m.pageContent = invalid
        askPage(zapping, 0, channel.id, "")
    else if m.listCategory <> m.category
        m.category = m.listCategory
        m.categoryName = m.listName
        m.page = page
        m.pageContent = m.channelList.content
        m.pageAsked = m.listAsked
    end if
    closeList()
    if m.channel <> invalid and channel.id = m.channel.id and not m.notice.visible then return
    tune(channel)
end sub

' --- Another category, or a search, in the list ---

' What a category is called in the list.
function listNameOf(category as string) as string
    if category = m.category then return m.categoryName
    return liveCategoryName(category, m.groups)
end function

' The list shows category's channels, from the channel playing when it is
' among them; up and down keep theirs until a channel is chosen there.
sub showListCategory(category as string)
    m.listSearched = false
    m.listName = listNameOf(category)
    closeCategories()
    typeSet(m.listTitle, m.listName)
    if category = m.listCategory and m.listPage <> invalid then return
    m.listCategory = category
    m.listPage = invalid
    m.channelList.content = invalid
    showListEmpty()
    if category = m.category and m.page <> invalid
        m.listAsked = m.pageAsked
        showListPage(m.page, m.pageContent, "")
        return
    end if
    m.listRange.text = "Loading…"
    ' Chosen: asked for at once, whatever failed before.
    m.failedAt = -1
    m.failedCategory = ""
    askPage(category, 0, aroundIn(category, m.channel), "")
end sub

' Left in the list: the categories as Live TV's rail lists them (GET
' /api/live/groups, the first time), the one the list shows in focus.
sub openCategories()
    m.channelList.visible = false
    m.categoryList.visible = true
    showListEmpty()
    m.listTitle.text = "Categories"
    m.listRange.text = ""
    m.listHint.text = "OK: its channels   ·   Back: the channels"
    m.categoryList.setFocus(true)
    if m.summary = invalid
        m.listRange.text = "Loading…"
        uiRequest({ method: "GET", path: "/api/live/groups" }, "onGroups")
        return
    end if
    buildCategories(false)
end sub

sub onGroups(event as object)
    answer = event.getData()
    if not answer.ok
        if m.categoryList.visible then typeSet(m.listRange, answer.message)
        return
    end if
    m.summary = liveSummary(answer.data)
    m.groups = liveGroups(answer.data)
    region = liveRegionOf(m.listCategory, m.groups)
    if region <> "" then m.open = region
    if m.categoryList.visible then buildCategories(false)
end sub

' The categories, focus on the one the list shows (or, with keep, on the
' one in focus).
sub buildCategories(keep as boolean)
    m.listRange.text = ""
    wanted = m.listCategory
    at = m.categoryList.itemFocused
    if keep and at >= 0 and at < m.railEntries.Count() then wanted = m.railEntries[at].key
    m.railEntries = liveRail(m.summary, m.groups, m.open, "")
    at = 0
    for i = 0 to m.railEntries.Count() - 1
        if m.railEntries[i].kind <> "heading" and m.railEntries[i].key = wanted then at = i
    end for
    m.railFocused = at
    m.categoryList.content = nodesContent(liveRailItems(m.railEntries, m.open, m.listCategory))
    m.categoryList.jumpToItem = at
end sub

sub onCategoryFocused(event as object)
    at = event.getData()
    target = liveRailPast(m.railEntries, at, m.railFocused)
    m.railFocused = target
    if target <> at then m.categoryList.jumpToItem = target
end sub

' OK on a category shows its channels; on a region, its groups open (and
' another's close) or close.
sub onCategorySelected(event as object)
    at = event.getData()
    if at < 0 or at >= m.railEntries.Count() then return
    entry = m.railEntries[at]
    if entry.kind = "heading" then return
    if entry.kind = "region"
        if m.open = entry.key
            m.open = ""
        else
            m.open = entry.key
        end if
        buildCategories(true)
        return
    end if
    showListCategory(entry.key)
end sub

' Back from the categories: the list's channels again.
sub closeCategories()
    if not m.categoryList.visible then return
    m.categoryList.visible = false
    m.channelList.visible = true
    typeSet(m.listTitle, m.listName)
    typeSet(m.listHint, listHintText())
    m.listRange.text = ""
    if m.listPage <> invalid then typeSet(m.listRange, liveRange(m.listPage.offset, m.listPage.channels.Count(), m.listPage.total))
    showListEmpty()
    if m.list.visible then m.channelList.setFocus(true)
end sub

function listHintText() as string
    return "OK: watch   ·   Options (*): favourite   ·   Back: close" + Chr(10) + "Left: categories   ·   Replay: search"
end function

' Replay in the list: Roku's keyboard, every channel whose name, number or
' programme on now has the words in it.
sub askListSearch()
    dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
    dialog.title = "Search channels"
    dialog.message = ["Channels and what's on"]
    if Left(m.listCategory, 7) = "search:" then dialog.text = Mid(m.listCategory, 8)
    dialog.buttons = ["Search", "Cancel"]
    dialog.observeField("buttonSelected", "onListSearch")
    m.top.getScene().dialog = dialog
end sub

sub onListSearch(event as object)
    dialog = m.top.getScene().dialog
    text = ""
    labels = []
    if dialog <> invalid
        text = dialog.text.Trim()
        labels = dialog.buttons
    end if
    uiCloseDialog()
    chosen = event.getData()
    if chosen < 0 or chosen >= labels.Count() or labels[chosen] <> "Search" then return
    query = httpSentence(text)
    if query = "" then return
    ' The category shown as the first of the searches made here began.
    if not m.listSearched then m.listBefore = m.listCategory
    showListCategory("search:" + query)
    m.listSearched = true
end sub

' The Options key in the list: the channel in focus in or out of the
' favourites.
sub toggleListed()
    if m.listPage = invalid then return
    at = m.channelList.itemFocused
    if at < 0 or at >= m.listPage.channels.Count() then return
    toggleFavorite(m.listPage.channels[at])
end sub

' A channel in or out of the favourites (PUT or DELETE
' /api/live/channels/{id}/favorite): its star changes at once wherever it
' shows, and goes back if the server refuses, saying so, as on Android.
sub toggleFavorite(channel as object)
    favorite = not channel.favorite
    markFavorite(channel.id, favorite)
    method = "DELETE"
    if favorite then method = "PUT"
    uiRequest({ method: method, path: "/api/live/channels/" + urlValue(channel.id) + "/favorite", channelId: channel.id, favorite: favorite }, "onFavorite")
end sub

' The channel's star: on the channel playing (or the one Up or Down leads
' to), in the pages held and their rows, and on the bar.
sub markFavorite(id as integer, favorite as boolean)
    for each channel in [m.channel, m.zapTo]
        if channel <> invalid and channel.id = id then channel.favorite = favorite
    end for
    for each held in [{ page: m.page, rows: m.pageContent }, { page: m.listPage, rows: m.channelList.content }]
        if held.page <> invalid
            at = liveIndexOf(held.page.channels, id)
            if at >= 0
                held.page.channels[at].favorite = favorite
                item = invalid
                if held.rows <> invalid then item = held.rows.getChild(at)
                if item <> invalid then item.favorite = favorite
            end if
        end if
    end for
    if m.banner.visible then drawBanner()
end sub

sub onFavorite(event as object)
    answer = event.getData()
    if answer.ok then return
    spec = event.getRoSGNode().spec
    markFavorite(spec.channelId, not spec.favorite)
    if not uiFailed(answer) then uiToast(liveFavoriteFailed(answer.message), true)
end sub

' --- Tuning ---

' Up and down: the next channel of the category (round its ends), the
' page after or before it fetched at its ends.
sub zap(by as integer)
    from = m.zapTo
    if from = invalid then from = m.channel
    if m.page = invalid then askAgain(m.category)
    if from = invalid or m.page = invalid then return
    move = liveZap(m.page, from.id, by)
    if move = invalid then return
    if move.channel <> invalid
        zapTo(move.channel)
        return
    end if
    which = "first"
    if move.last then which = "last"
    askPage(m.category, move.offset, 0, which, from.id)
end sub

' The channel's logo and name show at once, and it is tuned once the
' presses stop. What's on shows only when asked for.
sub zapTo(channel as object)
    m.zapTo = channel
    hideBanner()
    showCard(channel, "")
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
        if m.played then m.card.visible = false
        return
    end if
    tune(target, 0, false, true)
end sub

' Watches channel: the session before it ends (the server ends it too,
' by the client_id), its logo shows while it starts. again: tuned again
' after a failure, which counts against the retries. stepped: led to by up
' and down, which a step still waiting goes on from. Neither is a pick.
sub tune(channel as object, takeOver = 0 as integer, again = false as boolean, stepped = false as boolean)
    m.attempt = m.attempt + 1
    m.picks = liveTunePicks(m.picks, again, stepped)
    abandonWatch()
    leave()
    if not again then m.retries = 0
    m.retryTimer.control = "stop"
    m.zapTo = invalid
    m.zapTimer.control = "stop"
    m.channel = channel
    m.watched = ""
    ' Kept for the server and profile: Live TV opens at it, after a
    ' restart too.
    session = m.global.session
    liveSaveChannel(CreateObject("roRegistrySection", "liveViews"), liveViewKey(session.server, session.profileId), channel.id)
    m.played = false
    m.trouble = invalid
    m.pictureTimer.control = "stop"
    m.video.control = "stop"
    m.notice.visible = false
    showWaiting(false)
    showCard(channel, "")
    hideBanner()
    markCurrent()
    if not m.list.visible then m.top.setFocus(true)
    ' The user's remote changes channel through the category, as up and
    ' down do.
    m.watchRequest = uiRequest({ op: "liveWatch", channelId: channel.id, takeOver: takeOver, attempt: m.attempt, controls: remoteLiveControls(true) }, "onWatch")
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
            uiUnreachable({ to: "livePlayer", params: { channel: m.channel, category: m.category } })
            return
        end if
        trouble(liveTrouble(answer, m.channel.name))
        return
    end if
    m.sessionId = httpString(answer.data.session_id)
    m.watched = m.sessionId
    m.playlist = httpString(answer.data.url)
    fresh = liveChannel(answer.data.channel, m.global.session.server, liveLogoWidth(), m.category)
    if fresh <> invalid and fresh.id = m.channel.id then m.channel = fresh
    showCard(m.channel, "")
    if m.banner.visible then drawBanner()
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
            uiUnreachable({ to: "livePlayer", params: { channel: m.channel, category: m.category } })
            return
        end if
        failed(liveTrouble(answer, m.channel.name))
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
end sub

sub onState(event as object)
    state = event.getData()
    ' A stall shows the spinner only once it has lasted a moment: the
    ' briefest flicker of one doesn't.
    if state = "buffering" and m.played
        if not m.stalling
            m.stalling = true
            m.stallTimer.control = "start"
        end if
    else
        m.stalling = false
        m.stallTimer.control = "stop"
        showWaiting(false)
    end if
    if state = "playing"
            if not m.played
            m.played = true
            m.card.visible = false
            m.tuning.control = "stop"
            ' A channel without pictures (radio) keeps its card.
            m.pictureTimer.control = "start"
        end if
        return
    end if
    if state = "error" or state = "finished"
        broken()
    end if
end sub

sub onStall()
    if m.stalling then showWaiting(true)
end sub

' Someone took over the connection of the channel playing (the server's
' live_taken_over, sent before the stream breaks off, one for each viewer
' of the channel; matched on the channel): it stops at once with the
' server's words, said once, and isn't tuned again by itself. Said already
' from the playlist's 410 without words, the event's words show instead.
' Changing channel already (Up or Down pressed), the change goes on to the
' channel chosen, with no notice of the one left.
sub onTakenOver(event as object)
    data = event.getData()
    if m.channel = invalid or not m.started then return
    if Int(httpNumber(data.channel_id, 0)) <> m.channel.id then return
    stopped = liveStopped(data.message)
    if m.trouble <> invalid
        if m.trouble.kind = "stopped" and httpSentence(data.message) <> "" and m.trouble.message <> stopped.message then trouble(stopped)
        return
    end if
    endWatching(stopped)
end sub

' The watching's session stopped from another device or by the server's
' administrator (session_stopped): it stops as for a connection taken
' over, saying who stopped it and their message. Said already from the
' playlist's 410, these words show instead. Unlike a connection taken
' over, a channel change on its way (Up or Down pressed) ends too: the
' person stopped watching, not one channel.
sub onServerStopped(event as object)
    data = event.getData()
    session = httpString(data.session_id)
    if not m.started or session = "" or (session <> m.sessionId and session <> m.watched) then return
    stopped = liveStoppedBy(data)
    m.zapTo = invalid
    m.zapTimer.control = "stop"
    if m.trouble <> invalid
        if m.trouble.kind = "stopped" and m.trouble.message <> stopped.message then trouble(stopped)
        return
    end if
    endWatching(stopped)
end sub

' A command of the user's remote, on another device, for the watching's
' session (Remote.brs has the rule): next and previous are channel up and
' down, through the same category, the new channel's watch telling the
' server. Watching that stopped, or says what went wrong, obeys nothing.
sub onPlayerCommand(event as object)
    if not m.top.active or m.sessionId = "" or m.trouble <> invalid or m.notice.visible then return
    action = remoteAction({ kind: "live", session: m.sessionId, hasOrder: true }, event.getData())
    if action.act = "channel" then zap(action.by)
end sub

' Watching ended on the server: what is on its way belongs to it, and its
' session is gone there, so nothing is left; changing channel already (Up
' or Down pressed), the change goes on to the channel chosen.
sub endWatching(stopped as object)
    m.attempt = m.attempt + 1
    abandonWatch()
    m.sessionId = ""
    m.playlist = ""
    if m.zapTo <> invalid
        m.video.control = "stop"
        return
    end if
    trouble(stopped)
end sub

sub showWaiting(shown as boolean)
    if shown = m.waiting.visible then return
    m.waiting.visible = shown
    if shown
        m.waiting.control = "start"
    else
        m.waiting.control = "stop"
    end if
end sub

sub onPictureTimer()
    if not m.played or m.channel = invalid then return
    video = httpString(m.video.videoFormat)
    audio = httpString(m.video.audioFormat)
    if video = "" and audio <> "" then showCard(m.channel, "radio")
end sub

' The stream broke: what the server says of its playlist says what to do.
' Gone or refused, the player says so; otherwise the channel is tuned
' again (failed).
sub broken()
    if m.sessionId = "" or m.trouble <> invalid then return
    m.video.control = "stop"
    showCard(m.channel, "")
    uiRequest({ method: "GET", path: m.playlist, timeoutMs: 50000, attempt: m.attempt }, "onChecked")
end sub

sub onChecked(event as object)
    if not isCurrent(event) then return
    answer = event.getData()
    if answer.error = "signed_out" then return
    if answer.error = "network"
        uiUnreachable({ to: "livePlayer", params: { channel: m.channel, category: m.category } })
        return
    end if
    if not answer.ok
        failed(liveTrouble(answer, m.channel.name))
        return
    end if
    ' Its playlist is there: the stream itself broke off.
    message = "It stopped and couldn't get going again."
    failed({ kind: "other", title: m.channel.name + " stopped", message: message, lines: message, retry: true, connections: [] })
end sub

' Watching failed: the channel is tuned again where that may help (the
' server let it go, it wasn't ready, the stream broke), twice at most,
' after 1 s and then 3 s; after that, or when the server refused it, the
' player says why.
sub failed(problem as object)
    leave()
    delayMs = -1
    if liveRetried(problem) then delayMs = liveRetryDelay(m.retries)
    if delayMs < 0
        trouble(problem)
        return
    end if
    m.retries = m.retries + 1
    m.played = false
    m.video.control = "stop"
    showCard(m.channel, "")
    m.retryAttempt = m.attempt
    m.retryTimer.duration = delayMs / 1000
    m.retryTimer.control = "start"
end sub

sub onRetryTimer()
    if m.retryAttempt <> m.attempt or m.trouble <> invalid or m.channel = invalid then return
    tune(m.channel, 0, true)
end sub

' --- What shows ---

' The channel's logo (or its name) in the middle, over black, the spinner
' under it while it tunes; over a channel without pictures ("radio"),
' "Sound only" and what's on instead (Live.brs's liveTuningCard).
sub showCard(channel as object, kind as string)
    words = liveTuningCard(channel, kind = "radio", nowSeconds())
    typeSet(m.cardKind, words.kind)
    m.cardKind.visible = words.kind <> ""
    typeSet(m.cardTitle, words.title)
    typeSet(m.cardLine, words.line)
    typeSet(m.cardInitials, channel.name)
    ' The scene's agent carries the token; it changes with each refresh.
    m.cardLogo.setHttpAgent(m.top.getScene().getHttpAgent())
    m.cardLogo.uri = channel.logo
    onLogos()
    m.tuning.visible = kind <> "radio"
    if kind = "radio"
        m.tuning.control = "stop"
    else
        m.tuning.control = "start"
    end if
    m.card.visible = true
end sub

' Names show where no logo does (logos are often see-through).
sub onLogos()
    m.cardInitials.visible = m.cardLogo.uri = "" or m.cardLogo.loadStatus = "failed"
    m.bannerInitials.visible = m.bannerLogo.uri = "" or m.bannerLogo.loadStatus = "failed"
    m.noticeInitials.visible = m.noticeLogo.uri = "" or m.noticeLogo.loadStatus = "failed"
end sub

' The bar: the channel, what's on (LIVE or NEW before it, as on Android)
' with when and how long it has left, how far along, what's next, the
' Live chip, and Channels and the favourite.
sub drawBanner()
    channel = bannerChannel()
    if channel = invalid then return
    now = nowSeconds()
    on = liveOnNow(channel, now)
    coming = liveComingUp(channel, now)
    typeSet(m.bannerChannel, liveTitle(channel))
    typeSet(m.bannerInitials, channel.name)
    m.bannerLogo.setHttpAgent(m.top.getScene().getHttpAgent())
    m.bannerLogo.uri = channel.logo
    onLogos()
    placeBadge(m.bannerBadge, m.bannerTitle, liveBadge(on), 1100)
    if on <> invalid
        typeSet(m.bannerTitle, on.title)
        meta = [liveSpan(on, m.twelve), liveTimeLeft(on, now)]
        if on.line <> "" then meta.Push(on.line)
        if on.rating <> "" then meta.Push(on.rating)
        typeSet(m.bannerMeta, meta.Join("   ·   "))
        m.bannerFill.width = Int(1100 * liveProgress(on, now))
        m.bannerTrack.visible = true
        m.bannerFill.visible = true
    else
        typeSet(m.bannerTitle, channel.name)
        m.bannerMeta.text = "Nothing in the guide now"
        if not channel.hasGuide then m.bannerMeta.text = "No guide for this channel"
        m.bannerTrack.visible = false
        m.bannerFill.visible = false
    end if
    m.bannerNext.text = ""
    if coming <> invalid then typeSet(m.bannerNext, "Next   " + liveClock(coming.start, m.twelve) + "   " + coming.title)
    typeSet(m.star, "")
    if channel.favorite then typeSet(m.star, "★ Favourite")
    favorite = "Add to favourites"
    if channel.favorite then favorite = "Remove from favourites"
    ' Drawn again only when the favourite's words change: the chips are
    ' made anew each time.
    chips = m.controls.chips
    if chips = invalid or chips.Count() <> 2 or chips[1].label <> favorite then m.controls.chips = [{ label: "Channels", width: 0 }, { label: favorite, width: 0 }]
end sub

' A programme's LIVE or NEW (text, "" for neither) in badge, a small
' label of its own before its title, which takes the rest of width.
sub placeBadge(badge as object, title as object, text as string, width as integer)
    x = badge.translation[0]
    y = title.translation[1]
    typeSet(badge, text)
    badge.visible = text <> ""
    if text = ""
        title.translation = [x, y]
        title.width = width
        return
    end if
    badge.color = "0x7DD3FCFF"
    if text = "LIVE" then badge.color = "0xEB6B70FF"
    room = Int(badge.boundingRect().width) + 16
    title.translation = [x + room, y]
    title.width = width - room
end sub

' The channel the bar is of: the one Up or Down leads to, else the one
' playing.
function bannerChannel() as dynamic
    if m.zapTo <> invalid then return m.zapTo
    return m.channel
end function

' OK held: the bar, Channels in focus, as Android's long press shows its
' controls.
sub showControls()
    if bannerChannel() = invalid or m.list.visible or m.notice.visible then return
    drawBanner()
    m.controls.focusIndex = 0
    m.banner.visible = true
    m.controls.setFocus(true)
    pokeBanner()
end sub

sub hideBanner()
    m.bannerTimer.control = "stop"
    if not m.banner.visible then return
    m.banner.visible = false
    ' The bar gives the focus back to the picture.
    if m.controls.isInFocusChain() then m.top.setFocus(true)
end sub

' A touch: the bar goes 6 s after the last.
sub pokeBanner()
    if not m.banner.visible then return
    m.bannerTimer.control = "stop"
    m.bannerTimer.control = "start"
end sub

' Left alone, the bar goes once the channel plays; while it tunes, it
' waits.
sub onBannerTimer()
    if m.played and m.zapTo = invalid
        hideBanner()
    else
        pokeBanner()
    end if
end sub

' Channels opens the list; the favourite changes in place, the bar
' staying.
sub onControl(event as object)
    if event.getData() = 0
        openList()
        return
    end if
    channel = bannerChannel()
    if channel <> invalid then toggleFavorite(channel)
    pokeBanner()
end sub

' What covers the picture now (the list, what went wrong) takes the OK
' going down: let go, it opens nothing more.
sub forgetOk()
    m.okTiming = false
    m.okTimer.control = "stop"
end sub

' OK still down: it was held. The bar shows, and the rest of the press is
' its own.
sub onOkHeld()
    if not m.okTiming then return
    m.okTiming = false
    m.okHeld = true
    showControls()
end sub

' Each 15 s: what's on moves on, on the banner and in the list; once the
' programme listed as on now has ended, the channel playing is asked for
' again (GET /api/live/channels/{id}), at most once a minute.
sub onClock()
    if m.banner.visible then drawBanner()
    if m.list.visible then redrawList()
    if m.channel = invalid or not liveNowEnded([m.channel], 0, 1, nowSeconds()) then return
    ms = m.span.TotalMilliseconds()
    if not liveMayRefresh(m.refreshedAt, ms) then return
    m.refreshedAt = ms
    uiRequest({ method: "GET", path: "/api/live/channels/" + urlValue(m.channel.id), channelId: m.channel.id }, "onChannelFresh")
end sub

sub onChannelFresh(event as object)
    answer = event.getData()
    if not answer.ok or m.channel = invalid then return
    fresh = liveChannel(answer.data, m.global.session.server, liveLogoWidth(), m.category)
    if fresh = invalid or fresh.id <> m.channel.id then return
    m.channel = fresh
    for each page in [m.page, m.listPage]
        if page <> invalid
            at = liveIndexOf(page.channels, fresh.id)
            if at >= 0 then page.channels[at] = fresh
        end if
    end for
    if m.banner.visible then drawBanner()
    if m.list.visible then redrawList()
    ' Radio shows what's on in its card.
    if m.played and m.card.visible and m.zapTo = invalid then showCard(m.channel, "radio")
end sub

' The list's channels, what's on each and how far along.
sub redrawList()
    content = m.channelList.content
    if m.listPage = invalid or content = invalid then return
    now = nowSeconds()
    for i = 0 to m.listPage.channels.Count() - 1
        item = content.getChild(i)
        if item <> invalid
            row = liveListItem(m.listPage.channels[i], now, m.twelve)
            if item.line <> row.line then item.line = row.line
            if Abs(item.progress - row.progress) > 0.001 then item.progress = row.progress
        end if
    end for
end sub

' A watching that didn't start or stopped: a card in the middle, as
' Android TV's (Live.brs's liveTroubleCard), with Try again where it may
' help (Watch again when it stopped), Other channels, and Back to the
' guide. Every connection in use, a row for each with what holds it, and
' Watch here instead where this account may take it over, which has the
' focus first.
sub trouble(problem as object)
    m.trouble = problem
    m.retryTimer.control = "stop"
    m.video.control = "stop"
    m.card.visible = false
    m.tuning.control = "stop"
    showWaiting(false)
    forgetOk()
    hideBanner()
    card = liveTroubleCard(problem)
    channel = m.channel
    m.noticeInitials.text = ""
    m.noticeLogo.uri = ""
    if channel <> invalid
        typeSet(m.noticeInitials, channel.name)
        m.noticeLogo.setHttpAgent(m.top.getScene().getHttpAgent())
        m.noticeLogo.uri = channel.logo
    end if
    onLogos()
    typeSet(m.noticeTitle, card.title)
    y = 236 + m.noticeTitle.boundingRect().height
    typeSet(m.noticeLead, card.lead)
    m.noticeLead.visible = card.lead <> ""
    if m.noticeLead.visible
        m.noticeLead.translation = [64, y + 16]
        y = y + 16 + m.noticeLead.boundingRect().height + 8
    end if
    ' A row for each connection; the focus goes through their Watch here
    ' instead, then the buttons under them (focusTrouble).
    m.noticeRows.removeChildrenIndex(m.noticeRows.getChildCount(), 0)
    m.troubleStops = []
    for each c in card.rows
        y = y + 12
        troubleRow(c, y)
        y = y + 88
    end for
    typeSet(m.noticeNote, card.note)
    m.noticeNote.visible = card.note <> ""
    if m.noticeNote.visible
        m.noticeNote.translation = [64, y + 12]
        y = y + 12 + m.noticeNote.boundingRect().height
    end if
    typeSet(m.noticeText, card.text)
    m.noticeText.visible = card.text <> ""
    if m.noticeText.visible
        m.noticeText.translation = [64, y + 16]
        y = y + 16 + m.noticeText.boundingRect().height
    end if
    m.noticeActions = liveTroubleButtons(problem)
    chips = []
    for each button in m.noticeActions
        chips.Push({ label: button.label, width: 0 })
    end for
    m.noticeChoices.chips = chips
    m.noticeChoices.focusIndex = 0
    m.noticeChoices.translation = [Int((1200 - m.noticeChoices.rowWidth) / 2), y + 40]
    m.troubleStops.Push(m.noticeChoices)
    height = y + 40 + 56 + 64
    m.noticeBack.height = height
    m.noticeCard.translation = [360, Int((1080 - height) / 2)]
    m.troubleAt = 0
    m.notice.visible = true
    if not m.list.visible then focusTrouble()
end sub

' A connection's row on the card: its channel, who holds it, and Watch
' here instead where it may be taken over.
sub troubleRow(c as object, y as integer)
    row = m.noticeRows.createChild("Group")
    row.translation = [64, y]
    back = row.createChild("Rectangle")
    back.width = 1072
    back.height = 88
    back.color = "0x2E3440FF"
    take = invalid
    room = 1072 - 48
    if c.canTakeOver
        take = row.createChild("ChipRow")
        take.chips = [{ label: "Watch here instead", width: 0 }]
        take.translation = [1072 - 16 - take.rowWidth, 16]
        ' The connection it takes over, read back as it is chosen.
        take.id = c.channelId.ToStr()
        take.observeField("chosen", "onTakeOver")
        room = room - take.rowWidth - 16
        m.troubleStops.Push(take)
    end if
    name = row.createChild("Label")
    name.translation = [24, 10]
    name.width = room
    name.font = typeFont(700, 32)
    name.color = "0xEBEBEBFF"
    typeSet(name, c.channelName)
    holders = row.createChild("Label")
    holders.translation = [24, 48]
    holders.width = room
    holders.font = typeFont(500, 26)
    holders.color = "0xA1A9B6FF"
    typeSet(holders, c.holders)
end sub

' The card's focus: on its stop at m.troubleAt (a Watch here instead, or
' the buttons under them).
sub focusTrouble()
    if m.troubleStops.Count() = 0 then return
    if m.troubleAt >= m.troubleStops.Count() then m.troubleAt = m.troubleStops.Count() - 1
    m.troubleStops[m.troubleAt].setFocus(true)
end sub

sub onTakeOver(event as object)
    channelId = event.getRoSGNode().id.ToInt()
    if channelId > 0 then tune(m.channel, channelId)
end sub

sub onNoticeButton(event as object)
    at = event.getData()
    if m.noticeActions = invalid or at < 0 or at >= m.noticeActions.Count() then return
    button = m.noticeActions[at]
    if button.action = "retry"
        tune(m.channel)
    else if button.action = "list"
        openList()
    else
        uiNavigate({ back: true })
    end if
end sub

' Leaving: the session ends, the token's observer goes, and Live TV opens
' back at this channel.
sub finish()
    m.attempt = m.attempt + 1
    abandonWatch()
    leave()
    m.video.control = "stop"
    for each timer in [m.bannerTimer, m.okTimer, m.zapTimer, m.clock, m.pictureTimer, m.stallTimer, m.retryTimer]
        timer.control = "stop"
    end for
    m.okTiming = false
    if m.channel <> invalid then m.top.getScene().liveChannel = m.channel.id
    if m.started then m.global.session.unobserveFieldScoped("accessToken")
end sub

' With nothing over the picture, as on Android TV: up and down change
' channel (Up: the next number); OK, as it is let go, and the Options key
' (*), Roku's menu key, open the channel list at the right; OK held shows
' the bar of what's on with Channels and the favourite, which goes 6 s
' after it was last touched (Back hides it sooner). Left and Right do
' nothing, nor do play, rewind, fast forward and Replay: live is live.
' What's on never shows by itself as a channel starts, and Roku remotes
' have no Info key: it shows in that bar. In the list: the Options key
' puts the channel in focus in the favourites or takes it out, up and down
' past its ends go to the page before or after, Left opens the categories
' (Back and Right close them), Replay searches, Back leaves a search for
' the category before, and Back and Right close it. Over what went wrong,
' Up and Down go between the card's rows and its buttons, and the Options
' key opens the list too. Otherwise Back leaves (the scene).
function onKeyEvent(key as string, press as boolean) as boolean
    if key = "OK" and not press
        ' The rest of an OK held is the bar's.
        if m.okHeld
            m.okHeld = false
            return true
        end if
        if m.okTiming
            m.okTiming = false
            m.okTimer.control = "stop"
            openList()
            return true
        end if
        return false
    end if
    if not press then return false
    if m.list.visible and m.categoryList.visible
        if key = "back" or key = "right" then closeCategories()
        return true
    end if
    if m.list.visible
        if key = "back" and m.listSearched
            showListCategory(m.listBefore)
            return true
        end if
        if key = "back" or key = "right"
            closeList()
            return true
        end if
        if key = "left"
            openCategories()
            return true
        end if
        if key = "replay"
            askListSearch()
            return true
        end if
        if key = "options"
            toggleListed()
            return true
        end if
        if key = "up" and m.channelList.itemFocused = 0
            listPage(-1)
            return true
        end if
        if key = "down" and m.channelList.content <> invalid and m.channelList.itemFocused = m.channelList.content.getChildCount() - 1
            listPage(1)
            return true
        end if
        return true
    end if
    if key = "options"
        openList()
        return true
    end if
    ' The card: Up and Down between its Watch here instead and the buttons
    ' under them, which take Left, Right and OK.
    if m.notice.visible
        if key = "up" or key = "down"
            if key = "up" and m.troubleAt > 0 then m.troubleAt = m.troubleAt - 1
            if key = "down" and m.troubleAt < m.troubleStops.Count() - 1 then m.troubleAt = m.troubleAt + 1
            focusTrouble()
            return true
        end if
        return false
    end if
    ' The bar has the focus: its chips take Left, Right and OK.
    if m.banner.visible
        if key = "back"
            hideBanner()
        else
            pokeBanner()
        end if
        return true
    end if
    if key = "up"
        zap(1)
        return true
    end if
    if key = "down"
        zap(-1)
        return true
    end if
    if key = "OK"
        if not m.okTiming
            m.okTiming = true
            m.okTimer.control = "start"
        end if
        return true
    end if
    ' Live is live: no pause, rewind or skip.
    if key = "left" or key = "right" or key = "play" or key = "rewind" or key = "fastforward" or key = "replay" then return true
    return false
end function
