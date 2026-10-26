sub test_discovery_reads_answers()
    answer = "{""pley"":1,""id"":""1f2e3d"",""name"":""Living room"",""version"":""0.1.0"",""port"":7539}"
    tkEqual(discoveryAnswer(answer, "192.168.1.20:7539"), { url: "http://192.168.1.20:7539", name: "Living room", id: "1f2e3d", version: "0.1.0" })
    tkEqual(discoveryAnswer(answer, "[fe80::1]:7539").url, "http://[fe80::1]:7539")
    tkEqual(discoveryAnswer("{""pley"":1,""port"":8080}", "10.0.0.5:7539"), { url: "http://10.0.0.5:8080", name: "10.0.0.5", id: "", version: "" })
end sub

sub test_discovery_ignores_other_answers()
    tkInvalid(discoveryAnswer("{""pley"":2,""port"":7539}", "10.0.0.5:7539"), "another protocol version")
    tkInvalid(discoveryAnswer("{""pley"":1,""port"":0}", "10.0.0.5:7539"))
    tkInvalid(discoveryAnswer("{""pley"":1}", "10.0.0.5:7539"))
    tkInvalid(discoveryAnswer("pley?", "10.0.0.5:7539"), "another TV asking")
    tkInvalid(discoveryAnswer("", "10.0.0.5:7539"))
end sub

sub test_discovery_broadcasts_to_the_network_too()
    tkEqual(discoveryTargets("192.168.1.20"), ["255.255.255.255", "192.168.1.255"])
    tkEqual(discoveryTargets(""), ["255.255.255.255"])
    tkEqual(discoveryTargets(invalid), ["255.255.255.255"])
end sub

sub test_discovery_lists_each_server_once_by_name()
    found = []
    found = discoveryAdd(found, { url: "http://10.0.0.5:7539", name: "Office", id: "o", version: "" })
    found = discoveryAdd(found, { url: "http://10.0.0.6:7539", name: "attic", id: "a", version: "" })
    found = discoveryAdd(found, { url: "http://10.0.0.7:7539", name: "Office", id: "o", version: "" })
    tkEqual(found.Count(), 2)
    tkEqual(found[0].name, "attic")
    tkEqual(found[1].url, "http://10.0.0.5:7539")
end sub
