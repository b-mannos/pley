' What can be done with a title from the screens that show it: the
' Options key (*) on a tile, and the buttons of a title's page. On the
' render thread, with Ui.brs; changes go to the server through the API
' task, then to the scene's list of changes (uiNoteChange). A screen that
' includes this file has onTitleChanged(change), called for a change made
' while it shows; one covered meanwhile reads them as it shows again
' (uiChangesSince).

' The Options key (*) on a tile: what can be done with its title, in a
' dialog, as on the other TVs (pageTileOptions: Play, or Resume and From
' start, the watched mark, Remove from Continue Watching, My List, Add to
' playlist, Go to show, Go to season; music's Open). My List is as Home
' last saw it.
sub uiTileOptions(tile as dynamic, continueWatching as boolean)
    if tile = invalid then return
    options = pageTileOptions(tile, continueWatching, pageInList(m.top.getScene().myList, tile.itemId))
    if options.Count() = 0 then return
    labels = []
    for each option in options
        labels.Push(option.label)
    end for
    labels.Push("Cancel")
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = tile.title
    dialog.buttons = labels
    dialog.observeField("buttonSelected", "uiTileOptionChosen")
    kept = {}
    for each key in ["itemId", "kind", "title", "parentId", "grandparentId", "positionMs", "durationMs"]
        value = invalid
        if GetInterface(tile, "ifAssociativeArray") <> invalid
            value = tile[key]
        else if tile.hasField(key)
            value = tile.getField(key)
        end if
        kept[key] = value
    end for
    m.tileOptions = { tile: kept, options: options }
    m.top.getScene().dialog = dialog
end sub

sub uiTileOptionChosen(event as object)
    chosen = event.getData()
    state = m.tileOptions
    m.tileOptions = invalid
    uiCloseDialog()
    if state = invalid or chosen < 0 or chosen >= state.options.Count() then return
    uiTitleAction(state.tile, state.options[chosen].action)
end sub

' Does what an action of the Options key or a title's page says, for tile
' ({ itemId, kind, title, parentId, grandparentId, positionMs, durationMs
' }): plays it (a show's or a season's next episode), opens music's page,
' goes to an episode's show or season or a season's show (their pages),
' as on the other apps, or asks the server.
sub uiTitleAction(tile as object, action as string)
    if action = "playlist"
        uiAddToPlaylist(tile)
        return
    end if
    kind = httpString(tile.kind)
    if action = "play" or action = "resume" or action = "start"
        uiPlayTitle(tile, action)
        return
    end if
    if action = "open"
        uiOpenTile(tile)
        return
    end if
    showId = httpNumber(tile.grandparentId, 0)
    if kind = "season" then showId = httpNumber(tile.parentId, 0)
    if action = "show"
        if showId > 0 then uiNavigate({ to: "details", params: { itemId: showId } })
        return
    end if
    if action = "season"
        if showId > 0 then uiNavigate({ to: "show", params: { showId: showId, seasonId: httpNumber(tile.parentId, 0) } })
        return
    end if
    request = pageActionRequest(action, tile.itemId)
    if request = invalid then return
    request.action = action
    request.itemId = tile.itemId
    uiRequest(request, "uiTitleActionDone")
end sub

' Play plays a movie or an episode from its place (or its start), Resume
' from its place, From start from 0; a show or a season plays its next
' episode, as its page's Play does.
sub uiPlayTitle(tile as object, action as string)
    kind = httpString(tile.kind)
    if kind = "show" or kind = "season"
        uiRequest({ method: "GET", path: "/api/items/" + urlValue(tile.itemId) + "/on-deck" }, "uiTitleOnDeck")
        return
    end if
    startMs = 0
    if action <> "start" and fmtResumable(tile.positionMs, tile.durationMs) then startMs = Int(httpNumber(tile.positionMs, 0))
    uiNavigate({ to: "player", params: { itemId: tile.itemId, startMs: startMs } })
end sub

sub uiTitleOnDeck(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then uiToast(answer.message, true)
        return
    end if
    episode = invalid
    if answer.data <> invalid then episode = httpObject(answer.data.item)
    if episode = invalid
        uiToast("Nothing to play")
        return
    end if
    state = httpObject(episode.user_state)
    startMs = 0
    if state <> invalid and fmtResumable(state.position_ms, episode.duration_ms) then startMs = Int(httpNumber(state.position_ms, 0))
    uiNavigate({ to: "player", params: { itemId: httpNumber(episode.id, 0), startMs: startMs } })
end sub

sub uiTitleActionDone(event as object)
    answer = event.getData()
    spec = event.getRoSGNode().spec
    if not answer.ok
        if not uiFailed(answer) then uiToast(answer.message, true)
        return
    end if
    scene = m.top.getScene()
    if spec.action = "listAdd" or spec.action = "listRemove" then scene.myList = pageListWith(scene.myList, spec.itemId, spec.action = "listAdd")
    change = uiNoteChange({ itemId: spec.itemId, action: spec.action })
    m.changesSeen = change.count
    onTitleChanged(change)
end sub

' --- Add to playlist ---

' The profile's playlists of the title's kind, to add it to one, or to a
' new one named on the keyboard. What it came to shows in a toast.
sub uiAddToPlaylist(tile as object)
    m.playlistTile = tile
    uiRequest({ method: "GET", path: "/api/playlists" }, "uiPlaylistsListed")
end sub

sub uiPlaylistsListed(event as object)
    answer = event.getData()
    if not answer.ok or answer.data = invalid
        if not uiFailed(answer) then uiToast(answer.message, true)
        return
    end if
    m.playlistChoices = pagePlaylistChoices(answer.data.playlists, pagePlaylistKind(httpString(m.playlistTile.kind)))
    labels = []
    for each playlist in m.playlistChoices
        labels.Push(playlist.title)
    end for
    labels.Push("New playlist…")
    labels.Push("Cancel")
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = "Add to playlist"
    dialog.buttons = labels
    dialog.observeField("buttonSelected", "uiPlaylistChosen")
    m.top.getScene().dialog = dialog
end sub

sub uiPlaylistChosen(event as object)
    chosen = event.getData()
    uiCloseDialog()
    count = m.playlistChoices.Count()
    if chosen < count
        playlist = m.playlistChoices[chosen]
        uiRequest({ method: "POST", path: "/api/playlists/" + urlValue(playlist.id) + "/items", json: { item_ids: [m.playlistTile.itemId] }, playlist: playlist.title }, "uiPlaylistAdded")
    else if chosen = count
        dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
        dialog.title = "New playlist"
        dialog.buttons = ["Make it", "Cancel"]
        dialog.observeField("buttonSelected", "uiPlaylistNamed")
        m.top.getScene().dialog = dialog
    end if
end sub

sub uiPlaylistNamed(event as object)
    dialog = m.top.getScene().dialog
    name = ""
    if dialog <> invalid then name = dialog.text.Trim()
    uiCloseDialog()
    if event.getData() <> 0 or name = "" then return
    uiRequest({ method: "POST", path: "/api/playlists", json: { title: name, item_ids: [m.playlistTile.itemId] }, playlist: name }, "uiPlaylistAdded")
end sub

sub uiPlaylistAdded(event as object)
    answer = event.getData()
    if answer.ok
        uiToast("Added to " + event.getRoSGNode().spec.playlist)
    else if not uiFailed(answer)
        uiToast(pagePlaylistRefusal(answer), true)
    end if
end sub

' A title marked watched or unwatched, or played to some place, on this
' screen or another: its tiles in content (a list's, or rows of lists)
' show it. Each is replaced by a copy, which the list draws again.
sub uiPatchTiles(content as dynamic, change as object)
    if content = invalid then return
    action = httpString(change.action)
    if action <> "watched" and action <> "unwatched" and action <> "played" then return
    for i = 0 to content.getChildCount() - 1
        node = content.getChild(i)
        if node.getChildCount() > 0
            uiPatchTiles(node, change)
        else if node.hasField("itemId") and node.itemId = change.itemId and node.hasField("watched")
            fields = {}
            for each key in ["title", "subtitle", "image", "wide", "square", "progress", "watched", "unwatched", "leafCount", "itemId", "kind", "parentId", "grandparentId", "positionMs", "durationMs", "summary", "entryId", "historyId", "qualityBadges"]
                if node.hasField(key) then fields[key] = node.getField(key)
            end for
            fields.positionMs = 0
            fields.progress = 0.0
            ' A show or a season marked: none of its episodes left to watch,
            ' or all of them.
            if fields.DoesExist("leafCount")
                if action = "watched" then fields.unwatched = 0
                if action = "unwatched" then fields.unwatched = fields.leafCount
            end if
            if action = "played"
                fields.watched = change.watched
                if not change.watched
                    fields.positionMs = change.positionMs
                    fields.progress = fmtProgress(change.positionMs, change.durationMs)
                end if
            else
                fields.watched = action = "watched"
            end if
            fresh = CreateObject("roSGNode", "ContentNode")
            nodesFill(fresh, fields)
            content.replaceChild(fresh, i)
        end if
    end for
end sub
