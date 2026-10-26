sub test_player_reports_every_ten_seconds_while_playing()
    tkTrue(playerShouldReport("playing", 10000))
    tkFalse(playerShouldReport("playing", 9999))
    tkFalse(playerShouldReport("paused", 60000))
    tkFalse(playerShouldReport("buffering", 60000))
end sub

sub test_player_progress_bodies()
    tkEqual(playerProgress(61500.7, 6960000, "playing", "s1"), { position_ms: 61500, state: "playing", duration_ms: 6960000, session_id: "s1" })
    tkEqual(playerProgress(-3, 0, "stopped", ""), { position_ms: 0, state: "stopped" })
end sub

sub test_player_tells_media_errors_from_stream_errors()
    tkEqual(playerErrorKind(-1, { category: "http", errcode: 401, dbgmsg: "" }), "stream")
    tkEqual(playerErrorKind(-5, { category: "mediaerror" }), "media")
    tkEqual(playerErrorKind(-5, invalid), "media")
    tkEqual(playerErrorKind(-6, invalid), "media")
    tkEqual(playerErrorKind(-1, invalid), "stream")
    tkEqual(playerErrorKind(-3, {}), "stream")
end sub

sub test_player_reads_the_sessions_status()
    tkEqual(playerSessionState(410), "stopped")
    tkEqual(playerSessionState(404), "expired")
    tkEqual(playerSessionState(200), "alive")
    tkEqual(playerSessionState(401), "signed_out")
    tkEqual(playerSessionState(-7), "unreachable")
end sub

sub test_player_says_who_stopped_the_stream()
    tkEqual(playerStoppedMessage("{""code"":""session_stopped"",""error"":""session_stopped"",""message"":""Server maintenance at 10""}"), "Playback was stopped by the server's administrator." + Chr(10) + "Server maintenance at 10")
    tkEqual(playerStoppedMessage("{""code"":""session_stopped"",""error"":""session_stopped"",""message"":null}"), "Playback was stopped by the server's administrator.")
    tkEqual(playerStoppedMessage({ message: "Disk full" }), "Playback was stopped by the server's administrator." + Chr(10) + "Disk full")
    tkEqual(playerStoppedMessage("<html>"), "Playback was stopped by the server's administrator.")
end sub

sub test_player_up_next_at_the_credits()
    item = {
        files: [{ id: 7 }, { id: 8 }],
        markers: [
            { file_id: 7, kind: "intro", start_ms: 30000, end_ms: 90000, source: "fingerprint" },
            { file_id: 7, kind: "credits", start_ms: 3100000, end_ms: 3300000, source: "fingerprint" },
            { file_id: 8, kind: "credits", start_ms: 3200000, end_ms: 3300000, source: "chapter" }
        ]
    }
    tkEqual(playerUpNextAt(3300000, playerFileMarkers(item, 7)), 3100000)
    tkEqual(playerUpNextAt(3300000, playerFileMarkers(item, 8)), 3200000)
    tkEqual(playerUpNextAt(3300000, playerFileMarkers(item, 9)), 3270000, "no credits for this file: 30 s before the end")
    tkEqual(playerUpNextAt(3300000, []), 3270000)
    tkEqual(playerUpNextAt(45000, []), -1, "too short")
end sub

sub test_player_takes_the_markers_of_the_file_played()
    item = {
        files: [{ id: 1 }, { id: 2 }],
        markers: [
            { file_id: 1, kind: "intro", start_ms: 10000, end_ms: 70000, source: "fingerprint" },
            { file_id: 2, kind: "intro", start_ms: 0, end_ms: 5000, source: "chapter" },
            { file_id: 1, kind: "recap", start_ms: 0, end_ms: 5000, source: "chapter" },
            { file_id: 1, kind: "credits", start_ms: 9000, end_ms: 9000, source: "frames" },
            "credits"
        ]
    }
    tkEqual(playerFileMarkers(item, 2), [{ kind: "intro", startMs: 0, endMs: 5000, key: "intro:0" }])
    tkEqual(playerFileMarkers(item, invalid), [{ kind: "intro", startMs: 10000, endMs: 70000, key: "intro:10000" }], "the first file's, without other kinds or empty parts")
    tkEqual(playerFileMarkers({ files: [{ id: 1 }], markers: invalid }, 1), [])
    tkEqual(playerFileMarkers({ files: [], markers: item.markers }, invalid), [], "no file: no markers")
    tkEqual(playerFileMarkers(invalid, 1), [])
end sub

function skipMarkers() as object
    return playerFileMarkers({
        files: [{ id: 1 }],
        markers: [
            { file_id: 1, kind: "intro", start_ms: 10000, end_ms: 70000, source: "fingerprint" },
            { file_id: 1, kind: "credits", start_ms: 1200000, end_ms: 1300000, source: "fingerprint" }
        ]
    }, 1)
end function

sub test_player_shows_a_skip_button_until_a_second_before_the_end()
    markers = skipMarkers()
    tkInvalid(playerActiveMarker(markers, 9999))
    tkEqual(playerActiveMarker(markers, 10000).key, "intro:10000")
    tkEqual(playerActiveMarker(markers, 68999).key, "intro:10000")
    tkInvalid(playerActiveMarker(markers, 69000), "a second before the intro ends")
    tkEqual(playerActiveMarker(markers, 1250000).key, "credits:1200000")
    tkEqual(playerSkipLabel(markers[0]), "Skip Intro")
    tkEqual(playerSkipLabel(markers[1]), "Skip Credits")
end sub

sub test_player_leaves_an_episodes_credits_to_up_next()
    markers = skipMarkers()
    tkEqual(playerSkipFor(markers, 1250000, false).kind, "credits", "a movie, or an episode without a next one")
    tkInvalid(playerSkipFor(markers, 1250000, true), "Up Next comes up instead")
    tkEqual(playerSkipFor(markers, 20000, true).kind, "intro")
    tkInvalid(playerSkipFor(markers, 80000, false))
end sub

sub test_player_skips_to_the_end_of_an_intro_or_credits()
    markers = skipMarkers()
    tkEqual(playerSkipTarget(markers[0], 1500000), 69250, "just before the intro ends, so its last moment plays")
    tkEqual(playerSkipTarget({ kind: "intro", startMs: 0, endMs: 1000 }, 1500000), 500, "inside a very short intro")
    tkEqual(playerSkipTarget(markers[1], 1500000), 1300000)
    tkEqual(playerSkipTarget(markers[1], 1300000), -1, "credits to the end: the title ends")
    tkEqual(playerSkipTarget(markers[1], 1300800), -1, "less than a second left")
    tkEqual(playerSkipTarget(markers[1], invalid), 1300000, "no duration known")
end sub

sub test_player_limits_restarts()
    tkTrue(playerMayRestart([], 100000))
    tkTrue(playerMayRestart([50000, 90000], 100000))
    tkFalse(playerMayRestart([50000, 60000, 90000], 100000))
    tkTrue(playerMayRestart([1000, 2000, 90000], 100000), "older than a minute")
end sub

sub test_player_starts_where_the_title_was_left()
    tkEqual(playerStartOf(600000, 6000000), 600000)
    tkEqual(playerStartOf(5900000, 6000000), 0, "near the end: from the start")
    tkEqual(playerStartOf(0, 6000000), 0)
end sub
