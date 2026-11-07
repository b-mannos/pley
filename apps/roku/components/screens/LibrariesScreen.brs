' Every library, from Home's menu once it has more than six (Home.brs's
' homeMenu), as Android TV's Libraries page: "Movies · 412 movies", in the
' order the menu has them. params: { libraries ([{ id, name, kind,
' itemCount }], Home's) }.

sub init()
    m.list = m.top.findNode("list")
    typeSet(m.top.findNode("title"), t("browse.menu.libraries"))
    m.started = false
    m.libraries = []
    m.list.observeField("itemSelected", "onLibrary")
    m.top.observeField("active", "onActive")
end sub

sub onActive()
    if not m.top.active then return
    if not m.started
        m.started = true
        items = []
        for each library in httpArray(m.top.params.libraries)
            if GetInterface(library, "ifAssociativeArray") <> invalid
                m.libraries.Push(library)
                items.Push({ title: library.name + " · " + fmtCount(library.itemCount, library.kind) })
            end if
        end for
        m.list.content = nodesContent(items)
    end if
    m.list.setFocus(true)
end sub

sub onLibrary(event as object)
    at = event.getData()
    if at >= 0 and at < m.libraries.Count() then uiNavigate({ to: "library", params: { library: m.libraries[at] } })
end sub
