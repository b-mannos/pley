' An artist (GET /api/items/{id}, their albums /children), as Android
' TV's page: Play and Shuffle gather every track of theirs, album by album
' as the server lists them, hand them to the music player and open Now
' Playing; Now playing goes back to it while music plays. Their albums are
' a row under the buttons; OK on one opens it.
' params: { id, title (shown until the server answers) }.

sub init()
    m.backdrop = m.top.findNode("backdrop")
    m.initials = m.top.findNode("initials")
    m.picture = m.top.findNode("picture")
    m.text = m.top.findNode("text")
    m.kindLabel = m.top.findNode("kindLabel")
    m.name = m.top.findNode("name")
    m.line = m.top.findNode("line")
    m.summary = m.top.findNode("summary")
    m.buttons = m.top.findNode("buttons")
    m.albumsLabel = m.top.findNode("albumsLabel")
    m.albums = m.top.findNode("albums")
    m.message = m.top.findNode("message")
    m.message.text = t("common.states.loading")
    typeSet(m.kindLabel, t("titles.music.artist"))
    typeSet(m.albumsLabel, t("titles.music.albums"))
    m.started = false
    ' The artist's details once they came; their albums' ids in the
    ' server's order, invalid until they came.
    m.artist = invalid
    m.albumIds = invalid
    ' What the buttons say and do, in their order: "play", "shuffle",
    ' "nowPlaying".
    m.labels = []
    m.actions = []
    ' Play's tracks being gathered: { shuffled, lists (each album's, in
    ' order), next (the album to ask for next), left, ask }; invalid when
    ' not. ask tells this Play's answers from an earlier one's.
    m.gathering = invalid
    m.asks = 0
    m.lastFocus = m.buttons
    m.buttons.observeField("chosen", "onButton")
    m.albums.observeField("rowItemSelected", "onAlbum")
    m.message.observeField("chosen", "onRetry")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active
        m.lastFocus = m.buttons
        if m.albums.isInFocusChain() then m.lastFocus = m.albums
        return
    end if
    uiChangesSince()
    if m.started
        ' Now playing comes and goes with the music.
        showButtons()
        if m.buttons.visible
            m.lastFocus.setFocus(true)
        else if m.message.action <> ""
            m.message.setFocus(true)
        end if
        return
    end if
    m.started = true
    typeSet(m.name, httpSentence(m.top.params.title))
    typeSet(m.initials, pageInitials(m.name.text))
    layoutText()
    m.top.setFocus(true)
    ask()
end sub

' What hasn't come: the artist's details, their albums.
sub ask()
    path = "/api/items/" + urlValue(m.top.params.id)
    if m.artist = invalid then uiGet(path, "onArtist")
    if m.albumIds = invalid then uiRequest({ method: "GET", path: path + "/children", build: "albums" }, "onAlbums")
end sub

' What didn't load, the server's sentence, and Retry, as on Android TV.
' With the albums showing, they stay, and a toast says why the rest
' didn't come.
sub showFailure(answer as object)
    if m.albums.content <> invalid and m.albums.content.getChildCount() > 0
        uiToast(answer.message, true)
        return
    end if
    m.message.title = t("browse.music.artistFailed")
    typeSet(m.message, answer.message)
    m.message.action = t("common.actions.retry")
    m.message.visible = true
    if m.top.hasFocus() or m.message.isInFocusChain() then m.message.setFocus(true)
end sub

sub onRetry()
    m.message.title = ""
    m.message.text = t("common.states.loading")
    m.message.action = ""
    m.top.setFocus(true)
    ask()
end sub

' Albums have no watched marks or My List: only Add to playlist is
' offered here, which changes nothing shown.
sub onTitleChanged(change as object)
    m.lastChange = change
end sub

sub onArtist(event as object)
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        if not uiFailed(answer) then showFailure(answer)
        return
    end if
    m.artist = answer.data
    server = m.global.session.server
    typeSet(m.name, httpSentence(m.artist.title))
    typeSet(m.initials, pageInitials(m.name.text))
    typeSet(m.summary, httpClean(m.artist.summary, 400, false))
    showLine()
    agent = m.top.getScene().getHttpAgent()
    m.picture.setHttpAgent(agent)
    m.picture.uri = contentTileImage(m.artist, server, false, 320)
    m.backdrop.setHttpAgent(agent)
    m.backdrop.uri = contentTileImage(m.artist, server, true, contentBackdropWidth())
    ' Retried with the albums already in: the page is whole again, and the
    ' buttons take the focus Retry gave the page.
    if m.albumIds <> invalid and m.albums.content <> invalid
        m.message.visible = false
        if (m.top.hasFocus() or m.message.isInFocusChain()) and m.buttons.visible then m.buttons.setFocus(true)
    end if
end sub

sub onAlbums(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then showFailure(answer)
        return
    end if
    m.albumIds = answer.data.order
    showLine()
    showButtons()
    if answer.data.shown = 0
        m.message.title = ""
        m.message.action = ""
        m.message.text = t("titles.music.noAlbums")
        if (m.top.hasFocus() or m.message.isInFocusChain()) and m.buttons.visible then m.buttons.setFocus(true)
        return
    end if
    ' The task's tiles, as the row's.
    content = CreateObject("roSGNode", "ContentNode")
    row = content.createChild("ContentNode")
    row.appendChildren(event.getRoSGNode().content.getChildren(-1, 0))
    m.albums.content = content
    m.message.visible = false
    m.albumsLabel.visible = true
    if (m.top.hasFocus() or m.message.isInFocusChain()) and m.buttons.visible then m.buttons.setFocus(true)
end sub

' Their albums and tracks, and genres, once the details came.
sub showLine()
    if m.artist = invalid then return
    albums = 0
    if m.albumIds <> invalid then albums = m.albumIds.Count()
    typeSet(m.line, musicArtistLine(m.artist, albums))
    layoutText()
end sub

' The labels with text, one under the other.
sub layoutText()
    shown = [m.kindLabel, m.name]
    spacings = [4]
    for each part in [{ label: m.line, gap: 12 }, { label: m.summary, gap: 12 }]
        if part.label.text <> ""
            spacings.Push(part.gap)
            shown.Push(part.label)
        end if
    end for
    m.text.removeChildrenIndex(m.text.getChildCount(), 0)
    m.text.appendChildren(shown)
    m.text.itemSpacings = spacings
end sub

' Play and Shuffle once their albums came, with Now playing while music
' plays; the focus stays on the button it was on.
sub showButtons()
    actions = []
    labels = []
    if m.albumIds <> invalid and m.albumIds.Count() > 0
        actions.Append(["play", "shuffle"])
        labels.Append([t("common.actions.play"), t("titles.actions.shuffle")])
    end if
    if m.global.music.state <> "stopped"
        actions.Push("nowPlaying")
        labels.Push(t("titles.actions.nowPlaying"))
    end if
    m.actions = actions
    if FormatJson(labels) <> FormatJson(m.labels)
        m.labels = labels
        focused = m.buttons.focusIndex
        chips = []
        for each label in labels
            chips.Push({ label: label, width: 0 })
        end for
        m.buttons.chips = chips
        if focused > 0 and focused < labels.Count() then m.buttons.focusIndex = focused
    end if
    m.buttons.visible = labels.Count() > 0
end sub

sub onButton(event as object)
    at = event.getData()
    if at < 0 or at >= m.actions.Count() then return
    action = m.actions[at]
    if action = "nowPlaying"
        uiNavigate({ to: "nowPlaying" })
    else
        play(action = "shuffle")
    end if
end sub

' Every track of theirs: each album's asked for, four at a time.
sub play(shuffled as boolean)
    if m.gathering <> invalid or m.albumIds = invalid or m.albumIds.Count() = 0 then return
    m.asks = m.asks + 1
    lists = []
    for each id in m.albumIds
        lists.Push(invalid)
    end for
    m.gathering = { shuffled: shuffled, lists: lists, next: 0, left: lists.Count(), ask: m.asks }
    for i = 1 to 4
        askAlbumTracks()
    end for
end sub

sub askAlbumTracks()
    gathering = m.gathering
    at = gathering.next
    if at >= m.albumIds.Count() then return
    gathering.next = at + 1
    uiRequest({ method: "GET", path: "/api/items/" + urlValue(m.albumIds[at]) + "/children", build: "trackQueue", at: at, ask: gathering.ask }, "onAlbumTracks")
end sub

' An album's tracks came: the next album is asked for, or, the last in,
' the music player plays them all (shuffled: from any track, the others
' after it in a random order; the music player keeps their order for
' Previous and Stop shuffling).
sub onAlbumTracks(event as object)
    gathering = m.gathering
    spec = event.getRoSGNode().spec
    if gathering = invalid or spec.ask <> gathering.ask then return
    answer = event.getData()
    if not answer.ok
        m.gathering = invalid
        if not uiFailed(answer) then uiToast(answer.message, true)
        return
    end if
    gathering.lists[spec.at] = answer.data.queue
    gathering.left = gathering.left - 1
    if gathering.left > 0
        askAlbumTracks()
        return
    end if
    m.gathering = invalid
    tracks = []
    for each list in gathering.lists
        tracks.Append(list)
    end for
    ' Left meanwhile: nothing starts behind the screen now on top.
    if not m.top.active then return
    if tracks.Count() = 0
        uiToast(t("titles.nothingToPlay"))
        return
    end if
    at = 0
    if gathering.shuffled then at = Rnd(tracks.Count()) - 1
    m.global.music.play = { queue: tracks, at: at, shuffled: gathering.shuffled }
    uiNavigate({ to: "nowPlaying" })
end sub

' The album in focus in the row, or invalid.
function albumAt(position as dynamic) as dynamic
    if m.albums.content = invalid or GetInterface(position, "ifArray") = invalid or position.Count() < 2 then return invalid
    row = m.albums.content.getChild(position[0])
    if row = invalid then return invalid
    return row.getChild(position[1])
end function

sub onAlbum(event as object)
    uiOpenTile(albumAt(event.getData()))
end sub

' Down from the buttons goes to the albums, Up back; the Options key adds
' the album in focus, or the artist, to a playlist.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    hasAlbums = m.albums.content <> invalid and m.albums.content.getChildCount() > 0
    if key = "down" and m.buttons.isInFocusChain() and hasAlbums
        m.albums.setFocus(true)
        return true
    end if
    if key = "up" and m.albums.isInFocusChain() and m.buttons.visible
        m.buttons.setFocus(true)
        return true
    end if
    if key = "options"
        if m.albums.isInFocusChain() and hasAlbums
            uiTileOptions(albumAt(m.albums.rowItemFocused), false)
        else if m.artist <> invalid
            uiTileOptions({ itemId: m.artist.id, kind: "artist", title: httpSentence(m.artist.title) }, false)
        end if
        return true
    end if
    return false
end function
