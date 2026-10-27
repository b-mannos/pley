sub test_order_lists_every_library()
    listed = orderLibraries([
        { id: 1, name: "Movies", kind: "movies" },
        { id: 3, name: "Music", kind: "music" },
        { id: 0, name: "Broken" },
        "not a library"
    ])
    tkEqual(listed, [{ id: 1, name: "Movies" }, { id: 3, name: "Music" }], "music too: the order is the profile's everywhere")
    tkEqual(orderIds(listed), [1, 3])
    tkEqual(orderLibraries(invalid), [])
end sub

sub test_order_moves_one()
    tkEqual(orderMoved([1, 2, 3], 0, 1), [2, 1, 3])
    tkEqual(orderMoved([1, 2, 3], 2, 0), [3, 1, 2])
    tkEqual(orderMoved([1, 2, 3], 1, 2), [1, 3, 2])
    tkEqual(orderMoved([1, 2, 3], 1, 1), [1, 2, 3])
    tkEqual(orderMoved([1, 2, 3], 0, 9), [2, 3, 1], "past the end: last")
    tkEqual(orderMoved([1, 2, 3], 2, -1), [3, 1, 2], "before the start: first")
end sub

sub test_order_refusals()
    tkEqual(orderRefusal(404), "older")
    tkEqual(orderRefusal(405), "older")
    tkEqual(orderRefusal(409), "changed")
    tkEqual(orderRefusal(500), "")
end sub
