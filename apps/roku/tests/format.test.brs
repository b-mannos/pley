sub test_format_clock()
    tkEqual(fmtClock(0), "0:00")
    tkEqual(fmtClock(754000), "12:34")
    tkEqual(fmtClock(3723000), "1:02:03")
    tkEqual(fmtClock(59999), "0:59")
    tkEqual(fmtClock(-5), "0:00")
    tkEqual(fmtClock(invalid), "0:00")
end sub

sub test_format_durations()
    tkEqual(fmtDuration(6120000), "1 h 42 min")
    tkEqual(fmtDuration(2520000), "42 min")
    tkEqual(fmtDuration(7200000), "2 h")
    tkEqual(fmtDuration(20000), "1 min")
    tkEqual(fmtDuration(0), "")
    tkEqual(fmtDuration(invalid), "")
    tkEqual(fmtLeft(600000, 3000000), "40 min left")
    tkEqual(fmtLeft(3000000, 3000000), "")
end sub

sub test_format_progress()
    tkEqual(fmtProgress(500, 1000), 0.5)
    tkEqual(fmtProgress(0, 1000), 0)
    tkEqual(fmtProgress(2000, 1000), 1)
    tkEqual(fmtProgress(500, 0), 0)
end sub

sub test_format_episode_codes()
    tkEqual(fmtEpisode(1, 2), "S1 E2")
    tkEqual(fmtEpisode(0, 3), "S0 E3")
    tkEqual(fmtEpisode(invalid, 7), "E7")
    tkEqual(fmtEpisode(1, invalid), "")
end sub

sub test_format_resume_points()
    tkEqual(fmtResume(754000), "Resume from 12:34")
    tkTrue(fmtResumable(600000, 6000000))
    tkFalse(fmtResumable(0, 6000000))
    tkFalse(fmtResumable(500, 6000000), "under a second")
    tkFalse(fmtResumable(5500000, 6000000), "past 90 %, which counts as watched")
    tkTrue(fmtResumable(600000, 0), "a duration the server doesn't know")
end sub

sub test_format_library_counts()
    tkEqual(fmtCount(245, "movies"), "245 movies")
    tkEqual(fmtCount(1, "shows"), "1 show")
    tkEqual(fmtCount(0, "shows"), "0 shows")
    tkEqual(fmtCount(3, "music"), "3 titles")
end sub

sub test_format_meta_line()
    tkEqual(fmtMeta({ year: 2016, duration_ms: 6960000, content_rating: "PG-13", rating: 7.9 }), "2016 · 1 h 56 min · PG-13 · ★ 7.9")
    tkEqual(fmtMeta({ year: 2016, rating: 8 }), "2016 · ★ 8")
    tkEqual(fmtMeta({}), "")
    tkEqual(fmtOneDecimal(7.25), "7.3")
end sub
