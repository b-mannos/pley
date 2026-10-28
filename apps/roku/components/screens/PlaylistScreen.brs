' A playlist (GET /api/playlists/{id}): Play and Shuffle play its entries
' one after the other, Up Next bringing on the next; Rename and Delete
' change it (PATCH, DELETE), which needs the sign-in to allow playlists.
' params: { id, title (shown until the server answers) }.

sub init()
    m.title = m.top.findNode("title")
    m.line = m.top.findNode("line")
    m.buttons = m.top.findNode("buttons")
    m.entries = m.top.findNode("entries")
    m.message = m.top.findNode("message")
    m.started = false
    m.playlist = invalid
    m.dialog = invalid
    m.entryOptions = invalid
    m.lastFocus = m.buttons
    m.buttons.buttons = ["Play", "Shuffle", "Rename", "Delete"]
    m.buttons.observeField("buttonSelected", "onButton")
    m.entries.observeField("itemSelected", "onEntry")
    m.top.observeField("active", "onActive")
end sub

' Loaded each time it shows: back from playing, the places have changed.
sub onActive()
    if not m.top.active
        m.lastFocus = m.buttons
        if m.entries.isInFocusChain() then m.lastFocus = m.entries
        return
    end if
    if not m.started
        m.started = true
        m.title.text = httpSentence(m.top.params.title)
        uiChangesSince()
        m.top.setFocus(true)
    else
        uiChangesSince()
        if m.buttons.visible then m.lastFocus.setFocus(true)
    end if
    load()
end sub

sub load()
    uiRequest({ method: "GET", path: "/api/playlists/" + urlValue(m.top.params.id), build: "playlist" }, "onPlaylist")
end sub

sub onTitleChanged(change as object)
    uiPatchTiles(m.entries.content, change)
end sub

sub onPlaylist(event as object)
    answer = event.getData()
    if not answer.ok
        if m.playlist = invalid and not uiFailed(answer) then m.message.text = answer.message
        return
    end if
    m.playlist = answer.data
    m.title.text = m.playlist.title
    m.line.text = m.playlist.line
    keep = m.entries.itemFocused
    m.entries.content = event.getRoSGNode().content
    if keep > 0 and keep < m.playlist.shown then m.entries.jumpToItem = keep
    if m.playlist.shown = 0
        m.message.text = "This playlist is empty. Add titles to it with the Options key on them."
        m.message.visible = true
    else
        m.message.visible = false
    end if
    first = not m.buttons.visible
    m.buttons.visible = true
    if first and m.top.hasFocus() then m.buttons.setFocus(true)
end sub

sub onButton(event as object)
    if m.playlist = invalid then return
    chosen = m.buttons.buttons[event.getData()]
    if chosen = "Play"
        play(0, false)
    else if chosen = "Shuffle"
        play(0, true)
    else if chosen = "Rename"
        dialog = CreateObject("roSGNode", "StandardKeyboardDialog")
        dialog.title = "Rename playlist"
        dialog.text = m.playlist.title
        dialog.buttons = ["Rename", "Cancel"]
        dialog.observeField("buttonSelected", "onRename")
        m.dialog = dialog
        m.top.getScene().dialog = dialog
    else
        dialog = CreateObject("roSGNode", "StandardMessageDialog")
        dialog.title = "Delete " + m.playlist.title + "?"
        dialog.message = ["The playlist goes; its titles stay in your libraries."]
        dialog.buttons = ["Delete", "Cancel"]
        dialog.observeField("buttonSelected", "onDelete")
        m.dialog = dialog
        m.top.getScene().dialog = dialog
    end if
end sub

' Plays the playlist from its entry at (shuffled: in any order, that one
' first), Up Next bringing on each next one.
sub play(at as integer, shuffled as boolean)
    queue = pageQueueFrom(m.playlist.queue, at, shuffled)
    if queue.Count() = 0 then return
    if m.playlist.kind = "audio"
        ' Tracks: the music player, and Now Playing.
        m.global.music.play = { queue: queue, at: 0 }
        uiNavigate({ to: "nowPlaying" })
        return
    end if
    uiNavigate({ to: "player", params: { itemId: queue[0].itemId, startMs: queue[0].startMs, queue: queue } })
end sub

sub onEntry(event as object)
    if m.playlist <> invalid then play(event.getData(), false)
end sub

sub closeDialog()
    if m.dialog <> invalid then m.dialog.close = true
    m.dialog = invalid
end sub

sub onRename(event as object)
    name = ""
    if m.dialog <> invalid then name = m.dialog.text.Trim()
    closeDialog()
    if event.getData() <> 0 or name = "" or name = m.playlist.title then return
    uiRequest({ method: "PATCH", path: "/api/playlists/" + urlValue(m.top.params.id), json: { title: name } }, "onChanged")
end sub

sub onDelete(event as object)
    closeDialog()
    if event.getData() <> 0 then return
    uiRequest({ method: "DELETE", path: "/api/playlists/" + urlValue(m.top.params.id) }, "onDeleted")
end sub

sub onDeleted(event as object)
    answer = event.getData()
    if answer.ok
        uiNavigate({ back: true })
    else if not uiFailed(answer)
        uiMessage("The playlist wasn't deleted", [pagePlaylistRefusal(answer)])
    end if
end sub

' A change made: the playlist again, as the server has it now.
sub onChanged(event as object)
    answer = event.getData()
    if answer.ok
        load()
    else if not uiFailed(answer)
        uiMessage("That didn't work", [pagePlaylistRefusal(answer)])
    end if
end sub

' The Options key on an entry: Remove from playlist (not from a smart one,
' which its filter fills), then what can be done with its title.
sub entryOptions()
    entry = m.entries.content.getChild(m.entries.itemFocused)
    if entry = invalid then return
    tile = { itemId: entry.itemId, kind: entry.kind, title: entry.title, watched: entry.watched, parentId: entry.parentId, grandparentId: entry.grandparentId }
    options = []
    if not m.playlist.smart then options.Push({ label: "Remove from playlist", action: "remove" })
    for each option in pageTileOptions(tile, false, invalid)
        if option.action <> "playlist" then options.Push(option)
    end for
    labels = []
    for each option in options
        labels.Push(option.label)
    end for
    labels.Push("Cancel")
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = entry.title
    dialog.buttons = labels
    dialog.observeField("buttonSelected", "onEntryOption")
    m.entryOptions = { tile: tile, entryId: entry.entryId, options: options }
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

sub onEntryOption(event as object)
    closeDialog()
    state = m.entryOptions
    m.entryOptions = invalid
    chosen = event.getData()
    if state = invalid or chosen >= state.options.Count() then return
    action = state.options[chosen].action
    if action = "remove"
        uiRequest({ method: "DELETE", path: "/api/playlists/" + urlValue(m.top.params.id) + "/items/" + urlValue(state.entryId) }, "onChanged")
    else
        uiTitleAction(state.tile, action)
    end if
end sub

' Right from the buttons goes to the entries, left back.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    hasEntries = m.entries.content <> invalid and m.entries.content.getChildCount() > 0
    if key = "right" and m.buttons.isInFocusChain() and hasEntries
        m.entries.setFocus(true)
        return true
    end if
    if key = "left" and m.entries.isInFocusChain()
        m.buttons.setFocus(true)
        return true
    end if
    if key = "options" and m.entries.isInFocusChain() and hasEntries
        entryOptions()
        return true
    end if
    return false
end function
