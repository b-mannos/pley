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
    tkEqual(serversStartScreen("http://nas:7539", true, true), "profiles", "the household's profiles first")
    tkEqual(serversStartScreen("http://nas:7539", false, true), "link")
end sub

' A server chosen leads where a start on it would: "Who's watching?" for a
' household of several, on the PIN pad of the profile kept as Always start
' as when it has a PIN, never straight to Home past it.
sub test_servers_start_on_a_chosen_server()
    scope = "library play playlists profiles"
    household = { count: 2, ids: [1, 2], pin: 1 }
    tkEqual(serversStart("http://nas:7539", false, "ask"), { screen: "link", pin: false }, "not signed in there")
    tkEqual(serversStart("http://nas:7539", true, profAtStart(scope, household, 0)), { screen: "profiles", pin: false }, "a household of two")
    tkEqual(serversStart("http://nas:7539", true, profAtStart(scope, household, 2)), { screen: "profiles", pin: true }, "Always start as, with a PIN")
    tkEqual(serversStart("http://nas:7539", true, profAtStart(scope, { count: 2, ids: [1, 2], pin: 0 }, 2)), { screen: "home", pin: false }, "Always start as, no PIN")
    tkEqual(serversStart("http://nas:7539", true, profAtStart(scope, { count: 1, ids: [1], pin: 1 }, 0)), { screen: "home", pin: false }, "one profile")
    tkEqual(serversStart("http://nas:7539", true, profAtStart("library play", household, 0)), { screen: "home", pin: false }, "a sign-in that can't switch")
    tkEqual(serversStart("", false, ""), { screen: "servers", pin: false })
end sub
