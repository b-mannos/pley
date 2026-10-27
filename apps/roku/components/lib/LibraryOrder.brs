' The profile's own order of its libraries (PUT and DELETE
' /api/me/libraries/order, docs/API.md): every library it may open, music
' ones too, though this app shows none of those, since the order is the
' profile's on all its devices.

' GET /api/libraries as the sort screen lists them: [{ id, name }].
function orderLibraries(libraries as dynamic) as object
    out = []
    for each entry in httpArray(libraries)
        library = httpObject(entry)
        if library <> invalid
            id = httpNumber(library.id, 0)
            if id <> 0 then out.Push({ id: id, name: httpSentence(library.name) })
        end if
    end for
    return out
end function

' The ids of libraries, in their order: the body's ids.
function orderIds(libraries as object) as object
    ids = []
    for each library in libraries
        ids.Push(library.id)
    end for
    return ids
end function

' items with the one at index taken to target (the others keep their order).
function orderMoved(items as object, index as integer, target as integer) as object
    if index < 0 or index >= items.Count() then return items
    rest = []
    for i = 0 to items.Count() - 1
        if i <> index then rest.Push(items[i])
    end for
    if target < 0 then target = 0
    if target > rest.Count() then target = rest.Count()
    moved = []
    for i = 0 to rest.Count() - 1
        if i = target then moved.Push(items[index])
        moved.Push(rest[i])
    end for
    if target = rest.Count() then moved.Push(items[index])
    return moved
end function

' What a refusal of a new order means: "older" (a server from before
' profiles had an order of their own), "changed" (a library came or went
' since they were listed), "" for anything else.
function orderRefusal(status as integer) as string
    if status = 404 or status = 405 then return "older"
    if status = 409 then return "changed"
    return ""
end function
