' An album (GET /api/items/{id}, its tracks /children): Play, Shuffle and
' OK on a track hand its tracks to the music player and open Now Playing;
' as on Android TV, the track playing is marked, and OK on it pauses or
' goes on instead. As on the other TVs, Go to artist opens its artist's
' page, and Now playing goes back to the music while it plays.
' On several discs, each disc's tracks are under its heading, a row of
' the list that the focus passes over (lib/Music.brs, musicDiscRows).
' params: { id, title (shown until the server answers), trackId (the
' track to focus, from a search) }.

sub init()
    m.backdrop = m.top.findNode("backdrop")
    m.cover = m.top.findNode("cover")
    m.title = m.top.findNode("title")
    m.line = m.top.findNode("line")
    m.rating = m.top.findNode("rating")
    m.buttons = m.top.findNode("buttons")
    m.tracks = m.top.findNode("tracks")
    m.message = m.top.findNode("message")
    m.started = false
    m.album = invalid
    m.queue = []
    ' The list's rows' fields, the one playing marked (musicMarkedRows),
    ' and whether the music player is followed (while this screen shows).
    m.rowFields = []
    m.watchingMusic = false
    ' The row of the track that had the focus last, which says which way
    ' it was going when it lands on a heading.
    m.trackRow = 0
    m.lastFocus = m.buttons
    ' What the buttons do, in their order: "play", "shuffle", "artist",
    ' "nowPlaying".
    m.actions = []
    showButtons()
    m.buttons.observeField("buttonSelected", "onButton")
    m.tracks.observeField("itemSelected", "onTrack")
    m.tracks.observeField("itemFocused", "onTrackFocused")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    music = m.global.music
    if not m.top.active
        m.lastFocus = m.buttons
        if m.tracks.isInFocusChain() then m.lastFocus = m.tracks
        if m.watchingMusic
            music.unobserveFieldScoped("track")
            music.unobserveFieldScoped("state")
            m.watchingMusic = false
        end if
        return
    end if
    if not m.watchingMusic
        music.observeFieldScoped("track", "markPlaying")
        music.observeFieldScoped("state", "onMusicState")
        m.watchingMusic = true
    end if
    markPlaying()
    uiChangesSince()
    if m.started
        ' Now playing comes and goes with the music.
        showButtons()
        if m.lastFocus.isSameNode(m.tracks)
            focusTracks()
        else if m.buttons.visible
            m.buttons.setFocus(true)
        end if
        return
    end if
    m.started = true
    m.title.text = httpSentence(m.top.params.title)
    m.top.setFocus(true)
    path = "/api/items/" + urlValue(m.top.params.id)
    uiGet(path, "onAlbum")
    uiRequest({ method: "GET", path: path + "/children", build: "tracks" }, "onTracks")
end sub

' Tracks have no watched marks or My List: only Add to playlist is
' offered here, which changes nothing shown.
sub onTitleChanged(change as object)
    m.lastChange = change
end sub

sub onAlbum(event as object)
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        if not uiFailed(answer) then m.message.text = answer.message
        return
    end if
    m.album = answer.data
    m.title.text = httpSentence(m.album.title)
    m.rating.text = musicRatingLine(m.album)
    showLine()
    showButtons()
    image = contentTileImage(m.album, m.global.session.server, false, 420)
    agent = m.top.getScene().getHttpAgent()
    m.cover.setHttpAgent(agent)
    m.cover.uri = image
    m.backdrop.setHttpAgent(agent)
    m.backdrop.uri = image
end sub

sub showLine()
    if m.album <> invalid then m.line.text = musicAlbumLine(m.album, m.queue)
end sub

sub onTracks(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then m.message.text = answer.message
        return
    end if
    m.queue = answer.data.queue
    m.tracks.content = event.getRoSGNode().content
    m.rowFields = []
    for each node in m.tracks.content.getChildren(-1, 0)
        m.rowFields.Push({ title: node.title, subtitle: node.subtitle, duration: node.duration, itemId: httpNumber(node.itemId, 0), kind: node.kind, at: node.at })
    end for
    markPlaying()
    showLine()
    if m.queue.Count() = 0
        m.message.text = "This album has no tracks."
        return
    end if
    m.message.visible = false
    m.buttons.visible = true
    wanted = httpNumber(m.top.params.trackId, 0)
    row = -1
    for i = 0 to m.tracks.content.getChildCount() - 1
        at = trackAt(i)
        if at >= 0 and m.queue[at].itemId = wanted then row = i
    end for
    if not m.top.isInFocusChain() then return
    if row >= 0
        m.tracks.jumpToItem = row
        m.tracks.setFocus(true)
    else
        m.buttons.setFocus(true)
    end if
end sub

' The place among the album's tracks of the list's row; -1 for a disc's
' heading.
function trackAt(row as integer) as integer
    node = m.tracks.content.getChild(row)
    if node = invalid then return -1
    return node.at
end function

' A disc's heading passes the focus on: to the track beyond it, the way
' the focus was going, or back to the first track from the top.
sub onTrackFocused(event as object)
    row = event.getData()
    if row < 0 or m.tracks.content = invalid then return
    if trackAt(row) >= 0
        m.trackRow = row
        return
    end if
    beyond = row + 1
    if row < m.trackRow and row > 0 then beyond = row - 1
    if beyond >= m.tracks.content.getChildCount() then beyond = row - 1
    if beyond >= 0 then m.tracks.jumpToItem = beyond
end sub

' The tracks take the focus, never on a heading.
sub focusTracks()
    m.tracks.setFocus(true)
    if trackAt(m.tracks.itemFocused) < 0 then m.tracks.jumpToItem = m.tracks.itemFocused + 1
end sub

' The album from its track at, or shuffled: from any track, the others
' after it in a random order. The music player keeps the album's order
' for Previous and Stop shuffling.
sub play(at as integer, shuffled as boolean)
    if m.queue.Count() = 0 then return
    if shuffled then at = Rnd(m.queue.Count()) - 1
    m.global.music.play = { queue: m.queue, at: at, shuffled: shuffled }
    uiNavigate({ to: "nowPlaying" })
end sub

' Play and Shuffle, Go to artist once the album says whose it is, and Now
' playing while music plays; the focus stays on the button it was on.
sub showButtons()
    actions = ["play", "shuffle"]
    labels = ["Play", "Shuffle"]
    if m.album <> invalid and httpNumber(m.album.parent_id, 0) > 0
        actions.Push("artist")
        labels.Push("Go to artist")
    end if
    if m.global.music.state <> "stopped"
        actions.Push("nowPlaying")
        labels.Push("Now playing")
    end if
    m.actions = actions
    if FormatJson(labels) <> FormatJson(m.buttons.buttons)
        focused = m.buttons.buttonFocused
        m.buttons.buttons = labels
        if focused > 0 and focused < labels.Count() then m.buttons.focusButton = focused
    end if
end sub

sub onButton(event as object)
    action = m.actions[event.getData()]
    if action = "artist"
        uiNavigate({ to: "artist", params: { id: httpNumber(m.album.parent_id, 0), title: httpSentence(m.album.parent_title) } })
    else if action = "nowPlaying"
        uiNavigate({ to: "nowPlaying" })
    else
        play(0, action = "shuffle")
    end if
end sub

' OK on the track playing pauses or goes on (playOrToggle, as on Android
' TV); on another, the album plays from it.
sub onTrack(event as object)
    at = trackAt(event.getData())
    if at < 0 then return
    if m.queue[at].itemId = playingId()
        m.global.music.control = "toggle"
    else
        play(at, false)
    end if
end sub

' The track the music player has, 0 for none.
function playingId() as dynamic
    music = m.global.music
    if music.state = "stopped" or music.track = invalid then return 0
    return httpNumber(music.track.itemId, 0)
end function

' The row of the track playing is marked, as on Android TV: a new node,
' which the list draws again, for each row that changes.
sub markPlaying()
    content = m.tracks.content
    if content = invalid then return
    for each change in musicMarkedRows(m.rowFields, playingId())
        m.rowFields[change.at] = change.fields
        fresh = CreateObject("roSGNode", "ContentNode")
        nodesFill(fresh, change.fields)
        content.replaceChild(fresh, change.at)
    end for
end sub

' The music stopped or started: the mark, and Now playing, follow.
sub onMusicState()
    markPlaying()
    showButtons()
end sub

' Right from the buttons goes to the tracks, left back; the Options key on
' a track adds it to a playlist.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    hasTracks = m.queue.Count() > 0
    if key = "right" and m.buttons.isInFocusChain() and hasTracks
        focusTracks()
        return true
    end if
    if key = "left" and m.tracks.isInFocusChain()
        m.buttons.setFocus(true)
        return true
    end if
    if key = "options" and hasTracks
        if m.tracks.isInFocusChain()
            at = trackAt(m.tracks.itemFocused)
            if at < 0 then return true
            track = m.queue[at]
            uiTileOptions({ itemId: track.itemId, kind: "track", title: track.title }, false)
        else if m.album <> invalid
            uiTileOptions({ itemId: m.album.id, kind: "album", title: httpSentence(m.album.title) }, false)
        end if
        return true
    end if
    return false
end function
