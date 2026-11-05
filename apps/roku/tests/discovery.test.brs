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

sub test_discovery_sweeps_the_tvs_network()
    urls = discoverySweep("192.168.1.20")
    tkEqual(urls.Count(), 253, "every other address of the /24")
    tkEqual(urls[0], "http://192.168.1.1:7539")
    tkEqual(urls[18], "http://192.168.1.19:7539")
    tkEqual(urls[19], "http://192.168.1.21:7539", "not the TV itself")
    tkEqual(urls[252], "http://192.168.1.254:7539")
    tkEqual(discoverySweepRange("192.168.1.20"), "192.168.1.1-254")
    tkEqual(discoverySweep("").Count(), 0, "no IPv4 address")
    tkEqual(discoverySweep(invalid).Count(), 0)
    tkEqual(discoverySweep("fe80::1").Count(), 0)
    tkEqual(discoverySweepRange("1234.1.1.1"), "")
end sub

sub test_discovery_reads_what_a_sweep_found()
    answer = "{""id"":""1f2e3d"",""name"":""Living room"",""version"":""0.1.0"",""setup_required"":false}"
    tkEqual(discoverySwept("http://192.168.1.20:7539", 200, answer), { url: "http://192.168.1.20:7539", name: "Living room", id: "1f2e3d", version: "0.1.0" })
    tkEqual(discoverySwept("http://192.168.1.20:7539", 200, "{""version"":""0.1.0""}").name, "192.168.1.20", "no name: its address")
    tkInvalid(discoverySwept("http://192.168.1.20:7539", 200, "{""name"":""Router""}"), "not Pley")
    tkInvalid(discoverySwept("http://192.168.1.20:7539", 200, "<html></html>"))
    tkInvalid(discoverySwept("http://192.168.1.20:7539", 404, answer))
    tkInvalid(discoverySwept("http://192.168.1.20:7539", -28, ""), "no answer in time")
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
