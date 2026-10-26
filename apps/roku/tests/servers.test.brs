sub test_servers_list_round_trips()
    list = [{ url: "http://nas:7539", name: "Living room", id: "abc", lastUsed: 100 }, { url: "https://pley.example.com", name: "Away", id: "", lastUsed: 50 }]
    tkEqual(serversParse(serversSerialize(list)), list)
    tkEqual(serversParse("garbage"), [])
    tkEqual(serversParse(""), [])
    tkEqual(serversParse("[{""url"":""ftp://x""},{""url"":""http://NAS:7539/"",""name"":""""}]"), [{ url: "http://nas:7539", name: "nas", id: "", lastUsed: 0 }])
end sub

sub test_servers_with_replaces_the_same_server()
    list = [{ url: "http://nas:7539", name: "Living room", id: "abc", lastUsed: 100 }, { url: "http://box:7539", name: "Box", id: "", lastUsed: 50 }]
    moved = serversWith(list, { url: "http://192.168.1.30:7539", name: "Living room", id: "abc", lastUsed: 200 })
    tkEqual(moved.Count(), 2, "found at a new address")
    tkEqual(moved[1].url, "http://192.168.1.30:7539")
    again = serversWith(list, { url: "http://box:7539", name: "Box", id: "", lastUsed: 300 })
    tkEqual(again.Count(), 2)
    tkEqual(again[1].lastUsed, 300)
    tkEqual(serversWithout(list, "http://box:7539").Count(), 1)
end sub

sub test_servers_merge_saved_first_then_found()
    saved = [{ url: "http://box:7539", name: "Box", id: "", lastUsed: 50 }, { url: "http://nas:7539", name: "Living room", id: "abc", lastUsed: 100 }, { url: "http://pley.example.com", name: "Away", id: "zzz", lastUsed: 10 }]
    found = [{ url: "http://192.168.1.30:7539", name: "Living room", id: "abc", version: "0.1.0" }, { url: "http://192.168.1.40:7539", name: "Box", id: "other", version: "0.1.0" }, { url: "http://192.168.1.50:7539", name: "Attic", id: "att", version: "0.1.0" }]
    rows = serversMerge(saved, found)
    tkEqual(rows.Count(), 5)
    tkEqual(rows[0].name, "Living room", "most recently used first")
    tkTrue(rows[0].saved)
    tkTrue(rows[0].found, "the same id answered")
    tkEqual(rows[0].address, "nas")
    tkEqual(rows[1].name, "Box")
    tkFalse(rows[1].found)
    tkTrue(rows[2].unencrypted, "plain http beyond the home network")
    tkEqual(rows[3].url, "http://192.168.1.40:7539")
    tkTrue(rows[3].lookalike, "named like a saved server it isn't")
    tkFalse(rows[3].saved)
    tkEqual(rows[4].name, "Attic")
    tkFalse(rows[4].lookalike)
end sub

sub test_servers_choose_the_first_screen()
    tkEqual(serversStartScreen("", false), "servers")
    tkEqual(serversStartScreen("http://nas:7539", false), "link")
    tkEqual(serversStartScreen("http://nas:7539", true), "home")
end sub
