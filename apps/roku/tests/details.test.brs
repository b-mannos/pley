' The player's menu: the playback's details and the file's chapters.

function detailsTestFile() as object
    return ParseJson("{""id"":7,""bitrate"":24000000,""width"":3840,""height"":2160,""streams"":[{""index"":0,""kind"":""video"",""codec"":""hevc"",""width"":3840,""height"":2160,""dynamic_range"":""dolby_vision"",""dv_profile"":8,""dv_bl_compat"":1},{""index"":1,""kind"":""audio"",""codec"":""truehd"",""channels"":8,""default"":true},{""index"":2,""kind"":""audio"",""codec"":""ac3"",""channels"":6}],""chapters"":[{""start_ms"":0,""end_ms"":60000,""title"":""Opening""},{""start_ms"":60000,""end_ms"":120000,""title"":null}]}")
end function

sub test_details_name_streams_as_the_other_apps()
    file = detailsTestFile()
    tkEqual(detVideo(file.streams[0]), "HEVC 4K Dolby Vision P8.1")
    tkEqual(detAudio(file.streams[1]), "TrueHD 7.1")
    tkEqual(detAudio({ codec: "aac", channels: 5 }), "AAC 5ch")
    tkEqual(detRange({ dynamic_range: "dolby_vision", dv_profile: 7, dv_el: "fel" }), "Dolby Vision P7 FEL")
    tkEqual(detRange({ dynamic_range: "hdr10plus" }), "HDR10+")
    tkEqual(detRange({ dynamic_range: "sdr" }), "")
    tkEqual(detCodec("hdmv_pgs_subtitle"), "PGS")
    tkEqual(detCodec("xyz"), "XYZ")
end sub

sub test_details_of_a_direct_play_and_a_conversion()
    file = detailsTestFile()
    direct = detPlayback({ method: "direct", audio_index: 2 }, file, -1, invalid, 0)
    tkEqual(direct.method, "Direct play")
    tkEqual(direct.why, "", "direct play goes without saying")
    tkEqual(direct.rows, [{ label: "Video", value: "HEVC 4K Dolby Vision P8.1" }, { label: "Audio", value: "AC3 5.1" }, { label: "Bitrate", value: "24 Mbps" }])
    live = { video: "transcode", audio: "transcode", video_in: "HEVC 3840×2160 Dolby Vision P8.1", video_out: "H.264 1920×1080", audio_in: "TrueHD 7.1", audio_out: "AAC 2.0", tonemap: "libplacebo", speed: 1.5, transcoded_ms: 70000 }
    info = { method: "transcode", audio_index: 1, reasons: ["This player doesn't decode Dolby Vision."], burn_subtitles: true, video_range: "SDR (tone-mapped)" }
    converted = detPlayback(info, file, -1, live, 40000)
    tkEqual(converted.method, "Transcoding")
    tkEqual(converted.why, "This player doesn't decode Dolby Vision.")
    tkEqual(converted.rows[0], { label: "Video", value: "HEVC 4K Dolby Vision P8.1 → H.264 1080p SDR" })
    tkEqual(converted.rows[1], { label: "Audio", value: "TrueHD 7.1 → AAC 2.0" })
    tkEqual(converted.rows[3], { label: "Shown as", value: "SDR (tone-mapped with libplacebo)" })
    tkEqual(converted.rows[4], { label: "Subtitles", value: "Burned in" })
    tkEqual(converted.rows[5], { label: "Speed", value: "1.5× real time · 30 s ready" })
    remux = detPlayback({ method: "remux", audio_converted: true }, file, -1, invalid, 0)
    tkEqual(remux.method, "Direct stream · Audio transcoding")
    tkEqual(remux.why, "", "the method says it")
    tkEqual(remux.rows[0].value, "HEVC 4K Dolby Vision P8.1", "the video copied")
    tkEqual(remux.rows[1].value, "TrueHD 7.1 → …", "the audio converted, its output not known yet")
end sub

sub test_details_readout_over_the_video()
    file = detailsTestFile()
    node = { videoFormat: "", audioFormat: "", streamBps: 0, networkBps: 0, subtitles: "Off" }
    tkEqual(detReadout({ method: "direct", audio_index: 2 }, file, -1, node), ["Playback: Direct play", "Video: HEVC 4K Dolby Vision P8.1", "Audio: AC3 5.1", "Subtitles: Off", "Bitrate: 24 Mbps"])
    ' Converted: what the Roku decodes, the stream's bitrate and the network.
    node = { videoFormat: "mpeg4 avc", audioFormat: "aac", streamBps: 8000000, networkBps: 42000000, subtitles: "English (PGS)" }
    converted = detReadout({ method: "transcode", audio_index: 1, burn_subtitles: true }, file, -1, node)
    tkEqual(converted, ["Playback: Transcoding", "Video: HEVC 4K Dolby Vision P8.1 → H.264", "Audio: TrueHD 7.1 → AAC", "Subtitles: English (PGS) (burned in)", "Bitrate: 24 Mbps → 8 Mbps", "Network: 42 Mbps"])
    ' A direct stream copies the video; what the node says of it is the file's.
    remux = detReadout({ method: "remux", audio_converted: true }, file, -1, { videoFormat: "hevc", audioFormat: "ac3", streamBps: 0, networkBps: 0, subtitles: "" })
    tkEqual(remux, ["Playback: Direct stream · Audio transcoding", "Video: HEVC 4K Dolby Vision P8.1", "Audio: TrueHD 7.1 → AC3", "Bitrate: 24 Mbps"])
    tkEqual(detReadout({ method: "direct" }, invalid, -1, { subtitles: "" }), ["Playback: Direct play"], "no file known")
    tkEqual(detNodeCodec("HEVC"), "hevc")
    tkEqual(detNodeCodec("vp9"), "vp9")
end sub

sub test_details_chapters()
    chapters = detChapters(detailsTestFile())
    tkEqual(chapters, [{ label: "Opening · 0:00", startMs: 0 }, { label: "Chapter 2 · 1:00", startMs: 60000 }])
    tkEqual(detChapterAt(chapters, 59999), 0)
    tkEqual(detChapterAt(chapters, 60000), 1)
    tkEqual(detChapters(invalid).Count(), 0)
end sub

sub test_details_number_chapters_named_by_their_time()
    tkEqual(detChapterName("00:06:19.212", 3), "Chapter 3", "as mkvmerge names them")
    tkEqual(detChapterName("6:19", 3), "Chapter 3")
    tkEqual(detChapterName("01:02:03,500", 4), "Chapter 4")
    tkEqual(detChapterName(" ", 1), "Chapter 1")
    tkEqual(detChapterName(invalid, 2), "Chapter 2")
    tkEqual(detChapterName("Act 2: 12:00 at Noon", 2), "Act 2: 12:00 at Noon", "a time inside a real name")
    tkEqual(detChapterName("The Heist", 5), "The Heist")
end sub

sub test_details_next_and_previous_chapters_as_android_tv()
    chapters = [{ label: "", startMs: 0 }, { label: "", startMs: 60000 }, { label: "", startMs: 120000 }]
    tkEqual(detNextChapter(chapters, 30000), 1)
    tkEqual(detNextChapter(chapters, 59500), 2, "one starting within a second is where it is already")
    tkEqual(detNextChapter(chapters, 130000), -1, "the last chapter")
    tkEqual(detPreviousChapterMs(chapters, 70000), 60000, "back to the start of the chapter playing")
    tkEqual(detPreviousChapterMs(chapters, 62000), 0, "just into it: the one before")
    tkEqual(detPreviousChapterMs(chapters, 2000), 0, "the first's start")
    tkEqual(detPreviousChapterMs([{ label: "", startMs: 5000 }, { label: "", startMs: 60000 }], 3000), 0, "before the first: the file's start")
    tkEqual(detPreviousChapterMs([], 3000), 0)
end sub
