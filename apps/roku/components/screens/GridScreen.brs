' A grid of titles under a heading: a person's movies and shows (GET
' /api/people/{id}), or a collection's titles (GET /api/collections/{id}).
' params: { kind ("person", "collection"), id, title (shown until the
' server answers) }.

sub init()
    m.photoFrame = m.top.findNode("photoFrame")
    m.photo = m.top.findNode("photo")
    m.title = m.top.findNode("title")
    m.line = m.top.findNode("line")
    m.summary = m.top.findNode("summary")
    m.grid = m.top.findNode("grid")
    m.message = m.top.findNode("message")
    m.started = false
    m.grid.observeField("itemSelected", "onSelected")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active then return
    m.grid.setFocus(true)
    if m.started
        for each change in uiChangesSince()
            onTitleChanged(change)
        end for
        return
    end if
    m.started = true
    uiChangesSince()
    params = m.top.params
    m.title.text = httpSentence(params.title)
    if params.kind = "person"
        uiRequest({ method: "GET", path: "/api/people/" + urlValue(params.id), build: "person" }, "onGrid")
    else
        uiRequest({ method: "GET", path: "/api/collections/" + urlValue(params.id), build: "collection" }, "onGrid")
    end if
end sub

sub onGrid(event as object)
    answer = event.getData()
    if not answer.ok
        if not uiFailed(answer) then m.message.text = answer.message
        return
    end if
    page = answer.data
    m.title.text = page.title
    m.line.text = page.line
    m.summary.text = page.summary
    if page.image <> ""
        ' A photo at the left, the text beside it.
        for each label in [m.title, m.line, m.summary]
            label.translation = [248, label.translation[1]]
        end for
        m.photoFrame.visible = true
        m.photo.setHttpAgent(m.top.getScene().getHttpAgent())
        m.photo.uri = page.image
    end if
    m.grid.content = event.getRoSGNode().content
    if page.shown = 0
        m.message.text = "Nothing to show here."
    else
        m.message.visible = false
    end if
end sub

sub onTitleChanged(change as object)
    uiPatchTiles(m.grid.content, change)
end sub

sub onSelected(event as object)
    uiOpenTile(m.grid.content.getChild(event.getData()))
end sub

function onKeyEvent(key as string, press as boolean) as boolean
    if press and key = "options" and m.grid.content <> invalid
        uiTileOptions(m.grid.content.getChild(m.grid.itemFocused), false)
        return true
    end if
    return false
end function
