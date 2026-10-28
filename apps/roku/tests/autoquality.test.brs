sub test_aq_the_menu_in_its_order()
    ids = []
    labels = []
    for each quality in aqMenu()
        ids.Push(quality.id)
        labels.Push(quality.label)
    end for
    tkEqual(ids, ["original", "auto", "30m", "15m", "20m", "12m", "8m", "4m", "2m"], "the two 4K qualities first")
    tkEqual(labels, ["Original", "Automatic", "30 Mbps 4K", "15 Mbps 4K", "20 Mbps 1080p", "12 Mbps 1080p", "8 Mbps 1080p", "4 Mbps 720p", "2 Mbps 480p"])
    tkEqual(aqKbps("15m"), 15000)
    tkEqual(aqLines("15m"), 2160)
    tkEqual(aqKbps("auto"), 0)
    tkEqual(aqLabel("nonsense"), "Original")
end sub

sub test_aq_automatic_unless_a_quality_was_chosen()
    section = tkRegistrySection()
    tkEqual(aqChoice(section), "auto", "nothing chosen")
    aqChoose(section, "original")
    tkEqual(aqChoice(section), "original", "Original chosen is kept")
    aqChoose(section, "8m")
    tkEqual(aqChoice(section), "8m")
    tkEqual(section.flushes, 2)
    section.Write("quality", "junk")
    tkEqual(aqChoice(section), "auto")
end sub

sub test_aq_a_file_is_offered_the_qualities_that_make_it_smaller()
    tkEqual(aqOffered(10000, 1080), ["8m", "4m", "2m"], "at most 80 % of its bitrate")
    tkEqual(aqOffered(18000, 2160), ["20m", "12m", "8m", "4m", "2m"], "15 Mbps 4K is no lighter than 18")
    tkEqual(aqOffered(20000, 1080), ["15m", "12m", "8m", "4m", "2m"], "15 Mbps 4K as 15 Mbps 1080p, above 12")
    tkEqual(aqOffered(40000, 2160), ["30m", "15m", "20m", "12m", "8m", "4m", "2m"], "a 4K file: the menu's order")
    tkEqual(aqOffered(0, 0), ["30m", "15m", "20m", "12m", "8m", "4m", "2m"], "unknown: every one")
    tkEqual(aqOffered(1500, 480), [])
end sub

sub test_aq_the_players_menu_names_qualities_by_the_files_lines()
    labels = function(menu as object) as object
        out = []
        for each quality in menu
            out.Push(quality.label)
        end for
        return out
    end function
    tkEqual(labels(aqMenuFor({ bitrate: 12000000, width: 1280, height: 720 })), ["Original", "Automatic", "8 Mbps 720p", "4 Mbps 720p", "2 Mbps 480p"])
    tkEqual(labels(aqMenuFor({ bitrate: 10000000, width: 1920, height: 800 })), ["Original", "Automatic", "8 Mbps 1080p", "4 Mbps 720p", "2 Mbps 480p"], "a wide film is 1080p")
    tkEqual(labels(aqMenuFor(invalid)).Count(), 9, "an unknown file: the whole menu")
    tkEqual(aqLadder(40000, 2160, ""), ["30m", "15m", "12m", "8m", "4m", "2m"], "each lower in bitrate than the one above")
    tkEqual(aqLadder(40000, 2160, "15m"), ["15m", "12m", "8m", "4m", "2m"], "never above the server's quality")
end sub

sub test_aq_how_each_choice_is_asked_for()
    big = { bitrate: 24000000, height: 2160 }
    small = { bitrate: 6000000, streams: [{ kind: "video", height: 1080 }] }
    tkEqual(aqAsk("original", "", 0, big), { automatic: false, quality: "original", lowered: false, mode: "auto", maxBitrate: 0, maxHeight: 0 })
    tkEqual(aqAsk("12m", "", 0, big), { automatic: false, quality: "12m", lowered: false, mode: "transcode", maxBitrate: 12000, maxHeight: 1080 })
    tkEqual(aqAsk("12m", "", 0, small).quality, "original", "a file already under it plays as it is")
    tkEqual(aqAsk("auto", "", 0, big), { automatic: true, quality: "original", lowered: false, mode: "auto", maxBitrate: 0, maxHeight: 0 })
    ' Under a ceiling remembered for a slow network.
    tkEqual(aqAsk("auto", "", 9450, big), { automatic: true, quality: "8m", lowered: true, mode: "transcode", maxBitrate: 8000, maxHeight: 1080 })
    tkEqual(aqAsk("auto", "", 9450, small).quality, "original", "a file under the ceiling plays as it is")
    ' Automatic's restarts.
    tkEqual(aqAsk("auto", "4m", 0, big), { automatic: true, quality: "4m", lowered: false, mode: "transcode", maxBitrate: 4000, maxHeight: 720 })
    tkEqual(aqAsk("auto", "original", 9450, big).mode, "auto")
end sub

sub test_aq_steps_down_by_what_the_network_carried()
    ' A 40 Mbps file, segments arriving at 20 Mbps: 15 with the margin.
    tkEqual(aqStepDown("original", 40000, 20000, 40000, 2160, ""), { quality: "15m", ceiling: 15000 })
    ' A file played as it is, no segments: half the bitrate, then the margin.
    tkEqual(aqStepDown("original", 20000, 0, 20000, 1080, ""), { quality: "4m", ceiling: 7500 })
    tkEqual(aqStepDown("original", 60000, 50000, 60000, 2160, "15m").quality, "15m", "never above the server's own quality")
    tkEqual(aqStepDown("8m", 8000, 100000, 20000, 1080, "").quality, "4m", "always a quality lower")
    tkEqual(aqStepDown("4m", 4000, 100, 20000, 1080, "").quality, "2m", "never below 2 Mbps")
    tkInvalid(aqStepDown("2m", 2000, 100, 20000, 1080, ""))
    tkEqual(aqStepDown("original", 0, 0, 0, 0, "").quality, "8m", "nothing known")
end sub

sub test_aq_steps_up_one_at_a_time_then_the_original()
    tkEqual(aqStepUp("4m", 25000, 2160, ""), { quality: "8m", kbps: 8000 })
    tkEqual(aqStepUp("20m", 25000, 2160, ""), { quality: "original", kbps: 25000 })
    tkEqual(aqStepUp("15m", 60000, 2160, "15m"), { quality: "original", kbps: 15000 }, "the original converted is the server's quality")
    tkInvalid(aqStepUp("original", 25000, 2160, ""))
    tkInvalid(aqStepUp("30m", 0, 0, ""), "an unknown file can't be measured against")
end sub

' A Roku playing: the state at each second, segments as they come.
function aqPlay(watch as object, fromMs as integer, seconds as integer, segmentKbps as float) as object
    said = []
    now = fromMs
    for i = 1 to seconds
        now = now + 1000
        if segmentKbps > 0 and i MOD 2 = 0 then aqSegment(watch, now, 4000 * 250, 4000 * 250 * 8 / segmentKbps)
        quality = aqTick(watch, now)
        if quality <> "" then said.Push({ at: now, quality: quality })
    end for
    return { now: now, said: said }
end function

sub test_aq_a_long_wait_steps_down()
    watch = aqWatch(0, "original", 20000, 20000, 1080, "", false)
    tkEqual(aqState(watch, 1000, "playing", invalid), "")
    played = aqPlay(watch, 1000, 30, 12000)
    tkEqual(aqState(watch, played.now, "buffering", true), "")
    tkEqual(aqTick(watch, played.now + 1000), "")
    ' 12 Mbps carried, 9 with the margin.
    tkEqual(aqTick(watch, played.now + 2000), "8m")
    tkEqual(watch.ceiling, 9000)
    tkEqual(aqTick(watch, played.now + 3000), "", "one wait, one step")
end sub

sub test_aq_two_short_waits_within_a_minute_step_down()
    watch = aqWatch(0, "original", 20000, 20000, 1080, "", false)
    aqState(watch, 1000, "playing", invalid)
    aqPlay(watch, 1000, 30, 0)
    aqState(watch, 31000, "buffering", invalid)
    tkEqual(aqState(watch, 31500, "playing", invalid), "")
    aqPlay(watch, 31500, 20, 0)
    aqState(watch, 52000, "buffering", invalid)
    tkEqual(aqState(watch, 52800, "playing", invalid), "4m", "half, then the margin, without segments")
end sub

sub test_aq_waits_that_arent_the_network()
    watch = aqWatch(0, "original", 20000, 20000, 1080, "", false)
    ' The start.
    aqState(watch, 0, "buffering", invalid)
    tkEqual(aqPlay(watch, 0, 5, 0).said, [])
    aqState(watch, 5000, "playing", invalid)
    aqPlay(watch, 5000, 30, 0)
    ' A seek pley made.
    aqSeeked(watch)
    aqState(watch, 35000, "buffering", invalid)
    tkEqual(aqPlay(watch, 35000, 5, 0).said, [])
    aqState(watch, 40000, "playing", invalid)
    aqPlay(watch, 40000, 30, 0)
    ' The Roku says this wait isn't an underrun (a seek in its own controls).
    aqState(watch, 70000, "buffering", false)
    tkEqual(aqPlay(watch, 70000, 5, 0).said, [])
    ' Paused, then buffering on the way back.
    aqState(watch, 75000, "playing", invalid)
    aqPlay(watch, 75000, 10, 0)
    aqState(watch, 85000, "paused", invalid)
    aqState(watch, 95000, "buffering", invalid)
    tkEqual(aqPlay(watch, 95000, 5, 0).said, [])
end sub

sub test_aq_not_again_within_20_seconds()
    watch = aqWatch(0, "original", 20000, 20000, 1080, "", false)
    aqState(watch, 0, "playing", invalid)
    aqPlay(watch, 0, 30, 12000)
    aqState(watch, 30000, "buffering", true)
    tkEqual(aqTick(watch, 32000), "8m")
    aqRestarted(watch, 34000, "8m", 8000)
    aqState(watch, 35000, "playing", invalid)
    aqPlay(watch, 35000, 5, 0)
    aqState(watch, 40000, "buffering", true)
    tkEqual(aqTick(watch, 43000), "", "within 20 s of the change")
    aqState(watch, 44000, "playing", invalid)
    aqPlay(watch, 44000, 20, 100000)
    aqState(watch, 64000, "buffering", true)
    tkEqual(aqTick(watch, 66000), "4m", "a quality lower at least")
end sub

sub test_aq_back_up_when_segments_carry_it()
    watch = aqWatch(0, "4m", 4000, 20000, 1080, "", true)
    aqState(watch, 0, "playing", invalid)
    played = aqPlay(watch, 0, 299, 13000)
    tkEqual(played.said, [], "not within 5 minutes of a step down")
    tkEqual(aqTick(watch, 300000), "8m", "13 Mbps carries 1.6 times 8")
    aqRestarted(watch, 302000, "8m", 8000)
    aqState(watch, 303000, "playing", invalid)
    played = aqPlay(watch, 303000, 400, 13000)
    tkEqual(played.said, [], "12 Mbps needs 19.2")
    played = aqPlay(watch, played.now, 200, 40000)
    tkEqual(played.said[0].quality, "12m")
end sub

sub test_aq_a_file_played_as_it_is_stays_down()
    watch = aqWatch(0, "8m", 8000, 20000, 1080, "", true)
    aqState(watch, 0, "playing", invalid)
    tkEqual(aqPlay(watch, 0, 1200, 0).said, [], "no segments, no measure")
end sub

sub test_aq_remembers_each_server_for_30_minutes()
    section = tkRegistrySection()
    tkEqual(aqRemembered(section, "http://nas:7539", 1000000), 0)
    aqRemember(section, "http://nas:7539", 9450.5, 1000000)
    tkEqual(aqRemembered(section, "http://nas:7539", 1000000 + 29 * 60), 9450)
    tkEqual(aqRemembered(section, "http://other:7539", 1000000), 0, "another server's network")
    tkEqual(aqRemembered(section, "http://nas:7539", 1000000 + 31 * 60), 0)
    aqRemember(section, "http://nas:7539", 0, 1000060)
    tkEqual(aqRemembered(section, "http://nas:7539", 1000060), 0, "back at the original, forgotten")
end sub

sub test_aq_a_seek_in_the_rokus_own_controls_isnt_a_wait()
    watch = aqWatch(0, "original", 20000, 20000, 1080, "", false)
    aqState(watch, 0, "playing", invalid)
    for second = 1 to 30
        aqPosition(watch, second * 1000, second * 1000)
    end for
    ' Its wait came first, the jump ahead after.
    aqState(watch, 31000, "buffering", invalid)
    aqPosition(watch, 32000, 600000)
    tkEqual(aqTick(watch, 34000), "", "the wait was the seek's")
    aqState(watch, 34000, "playing", invalid)
    ' Playing on, a real wait counts again.
    for second = 35 to 40
        aqPosition(watch, second * 1000, 600000 + (second - 34) * 1000)
    end for
    aqState(watch, 40500, "buffering", invalid)
    tkEqual(aqTick(watch, 42600), "4m")
end sub
