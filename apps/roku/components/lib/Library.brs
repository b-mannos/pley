' A library's page as the web app has it (apps/web/src/pages/LibraryPage.tsx):
' its tabs (Recommended, Library, and Collections for movies and shows),
' and the Library tab's sort and filters. Which tab, sort and filters a
' library was left on is kept on the Roku for each server, profile and
' library, and it opens on them again.

' The tabs of a library of kind, in order.
function libTabs(kind as string) as object
    if kind = "music" then return ["recommended", "library"]
    return ["recommended", "library", "collections"]
end function

function libTabLabel(which as string) as string
    if which = "recommended" then return "Recommended"
    if which = "collections" then return "Collections"
    return "Library"
end function

' The sorts a library of kind offers, in order: [{ id, label }].
function libSorts(kind as string) as object
    all = [
        { id: "title", label: "Title" }, { id: "year", label: "Year" }, { id: "added", label: "Date added" },
        { id: "rating", label: "Rating" }, { id: "user_rating", label: "Your rating" }, { id: "release", label: "Release date" },
        { id: "duration", label: "Duration" }, { id: "last_played", label: "Last played" }, { id: "bitrate", label: "Bitrate" }
    ]
    if kind = "music" then return [all[0], all[2], all[4], all[7]]
    if kind = "shows"
        out = []
        for each sort in all
            if sort.id <> "duration" then out.Push(sort)
        end for
        return out
    end if
    return all
end function

' A sort's own order: A to Z for titles, newest or highest first for the rest.
function libDefaultOrder(sort as string) as string
    if sort = "title" then return "asc"
    return "desc"
end function

' How the Library tab is sorted, in words: "Title, A to Z", "Year, newest
' first".
function libSortLabel(sort as string, order as string) as string
    notes = {
        title: ["A to Z", "Z to A"],
        year: ["oldest first", "newest first"],
        added: ["first added first", "recently added first"],
        rating: ["lowest rated first", "best rated first"],
        user_rating: ["lowest rated first", "best rated first"],
        release: ["earliest first", "latest first"],
        duration: ["shortest first", "longest first"],
        last_played: ["least recent first", "most recent first"],
        bitrate: ["lowest first", "highest first"]
    }
    label = "Title"
    for each known in libSorts("movies")
        if known.id = sort then label = known.label
    end for
    pair = notes[sort]
    if pair = invalid then pair = notes.title
    if order = "desc" then return label + ", " + pair[1]
    return label + ", " + pair[0]
end function

' The view of a library a fresh Roku opens on: the Library tab by title,
' nothing filtered. watched: "" for every title, "false" unwatched only,
' "true" watched only; genre "" and year 0 for any.
function libDefaultView() as object
    return { tab: "library", sort: "title", order: "asc", watched: "", genre: "", year: 0 }
end function

' The query of the Library tab's page at offset.
function libQuery(view as object, offset as integer, limit as integer) as object
    query = { sort: view.sort, order: view.order, offset: offset, limit: limit }
    if view.watched <> "" then query.watched = view.watched
    if view.genre <> "" then query.genre = view.genre
    if view.year > 0 then query.year = view.year
    return query
end function

' What the filters keep, in words: "All titles", "Unwatched, Comedy, 1999".
function libFilterLabel(view as object) as string
    parts = []
    if view.watched = "false" then parts.Push("Unwatched")
    if view.watched = "true" then parts.Push("Watched")
    if view.genre <> "" then parts.Push(view.genre)
    if view.year > 0 then parts.Push(urlValue(view.year))
    if parts.Count() = 0 then return "All titles"
    return parts.Join(", ")
end function

function libFiltered(view as object) as boolean
    return view.watched <> "" or view.genre <> "" or view.year > 0
end function

' Where a library's view is kept: the server, the profile and the library.
function libViewKey(server as string, profileId as dynamic, libraryId as dynamic) as string
    return server + "|" + urlValue(profileId) + "|" + urlValue(libraryId)
end function

' The view kept for key, checked against what a library of kind offers;
' the default for anything else.
function libLoadView(section as object, key as string, kind as string) as object
    view = libDefaultView()
    kept = httpObject(httpJson(section.Read(key)))
    if kept = invalid then return view
    shownTab = httpString(kept.tab)
    for each known in libTabs(kind)
        if known = shownTab then view.tab = shownTab
    end for
    sort = httpString(kept.sort)
    for each known in libSorts(kind)
        if known.id = sort then view.sort = sort
    end for
    view.order = libDefaultOrder(view.sort)
    order = httpString(kept.order)
    if order = "asc" or order = "desc" then view.order = order
    watched = httpString(kept.watched)
    if watched = "true" or watched = "false" then view.watched = watched
    view.genre = httpSentence(kept.genre)
    year = httpNumber(kept.year, 0)
    if year > 0 then view.year = Int(year)
    return view
end function

sub libSaveView(section as object, key as string, view as object)
    section.Write(key, FormatJson(view))
    section.Flush()
end sub
