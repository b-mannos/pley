' Times and progress as the screens write them. Durations and positions
' are milliseconds, as the server sends them.

' "1:02:03", or "12:34" under an hour.
function fmtClock(ms as dynamic) as string
    total = Int(httpNumber(ms, 0) / 1000)
    if total < 0 then total = 0
    hours = Int(total / 3600)
    minutes = Int((total mod 3600) / 60)
    seconds = total mod 60
    if hours > 0 then return hours.ToStr() + ":" + fmtTwo(minutes) + ":" + fmtTwo(seconds)
    return minutes.ToStr() + ":" + fmtTwo(seconds)
end function

function fmtTwo(value as integer) as string
    if value < 10 then return "0" + value.ToStr()
    return value.ToStr()
end function

' "1 h 42 min", "42 min"; "" without a duration.
function fmtDuration(ms as dynamic) as string
    total = httpNumber(ms, 0)
    if total <= 0 then return ""
    minutes = Int((total + 30000) / 60000)
    if minutes < 1 then minutes = 1
    hours = Int(minutes / 60)
    minutes = minutes mod 60
    if hours = 0 then return minutes.ToStr() + " min"
    if minutes = 0 then return hours.ToStr() + " h"
    return hours.ToStr() + " h " + minutes.ToStr() + " min"
end function

' "42 min left"; "" without a duration.
function fmtLeft(positionMs as dynamic, durationMs as dynamic) as string
    remaining = httpNumber(durationMs, 0) - httpNumber(positionMs, 0)
    if httpNumber(durationMs, 0) <= 0 or remaining <= 0 then return ""
    return fmtDuration(remaining) + " left"
end function

' How far along, from 0 to 1.
function fmtProgress(positionMs as dynamic, durationMs as dynamic) as float
    duration = httpNumber(durationMs, 0)
    position = httpNumber(positionMs, 0)
    if duration <= 0 or position <= 0 then return 0
    if position >= duration then return 1
    return position / duration
end function

' "S1 E2"; "E2" without a season; "" without an episode.
function fmtEpisode(season as dynamic, episode as dynamic) as string
    s = httpNumber(season, -1)
    e = httpNumber(episode, -1)
    if e < 0 then return ""
    if s < 0 then return "E" + Int(e).ToStr()
    return "S" + Int(s).ToStr() + " E" + Int(e).ToStr()
end function

' "Resume from 12:34".
function fmtResume(ms as dynamic) as string
    return "Resume from " + fmtClock(ms)
end function

' Whether a title has a place to resume from: started, not near its end.
function fmtResumable(positionMs as dynamic, durationMs as dynamic) as boolean
    position = httpNumber(positionMs, 0)
    if position < 1000 then return false
    duration = httpNumber(durationMs, 0)
    if duration <= 0 then return true
    return position < duration * 0.9
end function

' The line under a title on its details: "2019 · 1 h 42 min · PG-13 · ★ 7.5".
function fmtMeta(item as object) as string
    parts = []
    year = httpNumber(item.year, 0)
    if year > 0 then parts.Push(Int(year).ToStr())
    duration = fmtDuration(item.duration_ms)
    if duration <> "" then parts.Push(duration)
    rating = httpSentence(item.content_rating)
    if rating <> "" then parts.Push(rating)
    score = httpNumber(item.rating, 0)
    if score > 0 then parts.Push("★ " + fmtOneDecimal(score))
    return parts.Join(" · ")
end function

' "1 movie", "245 movies", "31 shows" for a library's kind.
function fmtCount(count as dynamic, kind as string) as string
    n = Int(httpNumber(count, 0))
    noun = "title"
    if kind = "movies" then noun = "movie"
    if kind = "shows" then noun = "show"
    if n = 1 then return "1 " + noun
    return n.ToStr() + " " + noun + "s"
end function

' 7.5, 8 (no ".0").
function fmtOneDecimal(value as dynamic) as string
    tenths = Int(value * 10 + 0.5)
    if tenths mod 10 = 0 then return Int(tenths / 10).ToStr()
    return Int(tenths / 10).ToStr() + "." + (tenths mod 10).ToStr()
end function

' The languages offered for the audio and subtitle defaults, as ISO 639-2/B
' codes (the other apps' 34): [[code, name]].
function fmtLanguages() as object
    return [
        ["ara", "Arabic"], ["bul", "Bulgarian"], ["cat", "Catalan"], ["chi", "Chinese"], ["hrv", "Croatian"], ["cze", "Czech"], ["dan", "Danish"],
        ["dut", "Dutch"], ["eng", "English"], ["fin", "Finnish"], ["fre", "French"], ["ger", "German"], ["gre", "Greek"], ["heb", "Hebrew"],
        ["hin", "Hindi"], ["hun", "Hungarian"], ["ind", "Indonesian"], ["ita", "Italian"], ["jpn", "Japanese"], ["kor", "Korean"], ["nor", "Norwegian"],
        ["per", "Persian"], ["pol", "Polish"], ["por", "Portuguese"], ["rum", "Romanian"], ["rus", "Russian"], ["srp", "Serbian"], ["slo", "Slovak"],
        ["spa", "Spanish"], ["swe", "Swedish"], ["tha", "Thai"], ["tur", "Turkish"], ["ukr", "Ukrainian"], ["vie", "Vietnamese"]
    ]
end function

' A language's name, its code for one not offered, none for "". Files name
' some by their ISO 639-2/T code (fra for fre).
function fmtLanguageName(code as string, none as string) as string
    if code = "" then return none
    wanted = LCase(code)
    other = { fra: "fre", deu: "ger", zho: "chi", ces: "cze", nld: "dut", ell: "gre", fas: "per", ron: "rum", slk: "slo" }
    if other.DoesExist(wanted) then wanted = other[wanted]
    for each language in fmtLanguages()
        if language[0] = wanted then return language[1]
    end for
    return code
end function
