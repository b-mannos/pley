sub init()
    m.title = m.top.findNode("title")
    m.countLabel = m.top.findNode("count")
    m.grid = m.top.findNode("grid")
    m.message = m.top.findNode("message")
    m.started = false
    m.library = invalid
    ' Items the server listed so far (the next page's offset) and in all.
    m.offset = 0
    m.total = -1
    m.loading = false
    m.content = CreateObject("roSGNode", "ContentNode")
    m.grid.content = m.content
    m.grid.observeField("itemSelected", "onSelected")
    m.grid.observeField("itemFocused", "onFocused")
    m.top.observeField("active", "onActive")
end sub

function pageSize() as integer
    return 70
end function

sub onActive()
    if not m.top.active then return
    if not m.started
        m.started = true
        m.library = m.top.params.library
        m.title.text = m.library.name
        m.countLabel.text = fmtCount(m.library.itemCount, m.library.kind)
    end if
    if m.content.getChildCount() = 0 then loadMore()
    m.grid.setFocus(true)
end sub

' A page of posters; the task makes their ContentNodes.
sub loadMore()
    if m.loading or (m.total >= 0 and m.offset >= m.total) then return
    m.loading = true
    query = { sort: "title", order: "asc", offset: m.offset, limit: pageSize() }
    uiRequest({ method: "GET", path: urlWithQuery("/api/libraries/" + urlValue(m.library.id) + "/items", query), build: "tiles" }, "onPage")
end sub

sub onPage(event as object)
    m.loading = false
    answer = event.getData()
    if not answer.ok
        if m.content.getChildCount() = 0 and not uiFailed(answer)
            m.message.text = answer.message
            m.message.visible = true
        end if
        return
    end if
    page = answer.data
    m.total = page.total
    m.offset = m.offset + page.listed
    ' An empty page ends the listing, whatever the total said.
    if page.listed = 0 then m.total = m.offset
    tiles = event.getRoSGNode().content
    if tiles <> invalid and tiles.getChildCount() > 0 then m.content.appendChildren(tiles.getChildren(-1, 0))
    m.countLabel.text = fmtCount(m.total, m.library.kind)
    if m.content.getChildCount() = 0
        m.message.text = "This library is empty."
        m.message.visible = true
    else
        m.message.visible = false
    end if
end sub

' Two rows before the end, the next page comes.
sub onFocused(event as object)
    if event.getData() >= m.content.getChildCount() - 14 then loadMore()
end sub

sub onSelected(event as object)
    uiOpenTile(m.content.getChild(event.getData()))
end sub
