' Collections (GET /api/libraries/{id}/collections for each library of
' movies or shows): a row each, named after its library, in the libraries'
' order, once every library answered; a library without collections has no
' row. When none could be loaded, the page says so, with Retry, as on
' Android TV. params: { libraries ([{ id, name, kind }], Home's menu's) }.

sub init()
    m.rows = m.top.findNode("rows")
    m.message = m.top.findNode("message")
    m.started = false
    m.rows.observeField("rowItemSelected", "onTile")
    m.message.observeField("chosen", "load")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active then return
    if m.started
        if m.rows.visible
            m.rows.setFocus(true)
        else if m.message.action <> ""
            m.message.setFocus(true)
        else
            m.top.setFocus(true)
        end if
        return
    end if
    m.started = true
    m.top.setFocus(true)
    m.libraries = []
    for each library in httpArray(m.top.params.libraries)
        if GetInterface(library, "ifAssociativeArray") <> invalid and (library.kind = "movies" or library.kind = "shows") then m.libraries.Push(library)
    end for
    load()
end sub

' Every library's collections, asked together.
sub load()
    ' Each library's collections as they come ({ id: ContentNode }), how
    ' many answers are still to come, and why one failed.
    m.found = {}
    m.waiting = m.libraries.Count()
    m.failed = invalid
    m.message.title = ""
    m.message.action = ""
    if m.message.isInFocusChain() then m.top.setFocus(true)
    if m.waiting = 0
        m.message.text = "No collections yet."
        return
    end if
    m.message.text = "Loading…"
    for each library in m.libraries
        uiRequest({ method: "GET", path: "/api/libraries/" + urlValue(library.id) + "/collections", build: "collections", libraryId: library.id }, "onCollections")
    end for
end sub

sub onCollections(event as object)
    request = event.getRoSGNode()
    answer = event.getData()
    m.waiting = m.waiting - 1
    if not answer.ok
        ' Handled (the page goes for Unreachable or sign-in) unless the page
        ' was covered meanwhile: then it says why once every answer is in.
        if uiFailed(answer) and m.top.active then return
        if m.failed = invalid then m.failed = answer.message
    end if
    if answer.ok and request.content <> invalid and request.content.getChildCount() > 0 then m.found[urlValue(request.spec.libraryId)] = request.content
    if m.waiting > 0 then return
    content = CreateObject("roSGNode", "ContentNode")
    for each library in m.libraries
        tiles = m.found[urlValue(library.id)]
        if tiles <> invalid
            row = content.createChild("ContentNode")
            row.title = library.name
            row.appendChildren(tiles.getChildren(-1, 0))
        end if
    end for
    if content.getChildCount() = 0
        if m.failed <> invalid
            m.message.title = "Couldn't load collections"
            typeSet(m.message, m.failed)
            m.message.action = "Retry"
            if m.top.hasFocus() or m.message.isInFocusChain() then m.message.setFocus(true)
        else
            m.message.text = "No collections yet."
        end if
        return
    end if
    m.message.visible = false
    m.rows.content = content
    typeRowLabels(m.rows)
    m.rows.visible = true
    ' Only while this screen has the focus: a dialog over it keeps it.
    if m.top.hasFocus() or m.message.isInFocusChain() then m.rows.setFocus(true)
end sub

sub onTile(event as object)
    position = event.getData()
    row = m.rows.content.getChild(position[0])
    if row <> invalid then uiOpenTile(row.getChild(position[1]))
end sub
