' A stand-in for roFileSystem over a drive's folders: { "ext1:/": ["Films",
' …] }, files as { "ext1:/Films/Ad Astra.mkv": size }.
function fakeDrive(folders as object, files as object) as object
    return {
        folders: folders,
        files: files,
        stats: 0,
        Exists: function(path as string) as boolean
            return m.folders.DoesExist(path) or m.files.DoesExist(path)
        end function,
        Stat: function(path as string) as object
            m.stats = m.stats + 1
            if m.folders.DoesExist(path) then return { type: "directory", permissions: "r" }
            if m.files.DoesExist(path) then return { type: "file", size: m.files[path], permissions: "r" }
            return {}
        end function,
        GetDirectoryListing: function(path as string) as object
            if not m.folders.DoesExist(path) then return []
            return m.folders[path]
        end function
    }
end function

function driveNames(entries as object) as object
    names = []
    for each entry in entries
        names.Push(entry.name)
    end for
    return names
end function

sub test_drive_the_first_usb_volume()
    tkEqual(driveVolume(["cachefs:", "common:", "ext2:", "ext1:", "pkg:", "tmp:"]), "ext1:")
    tkEqual(driveVolume(["cachefs:", "ext3:", "pkg:"]), "ext3:")
    tkEqual(driveVolume(["cachefs:", "common:", "pkg:", "tmp:"]), "", "no drive")
    tkEqual(driveVolume(invalid), "")
    tkEqual(driveTop("ext1:"), "ext1:/")
end sub

sub test_drive_names_sort_as_a_person_sorts_them()
    entries = []
    for each name in ["Part 10.mkv", "part 2.mkv", "Part 1.mkv", "Alpha.mkv", "Part 02b.mkv"]
        entries.Push({ name: name })
    end for
    driveSort(entries)
    tkEqual(driveNames(entries), ["Alpha.mkv", "Part 1.mkv", "part 2.mkv", "Part 02b.mkv", "Part 10.mkv"], "as Android's and Samsung's")
    tkEqual(entries[0], { name: "Alpha.mkv" }, "the sort's key goes")
    entries = []
    for each name in ["Episode 100", "Episode 9", "Episode 0010", "10 Things", "9 Lives", "Ép 1"]
        entries.Push({ name: name })
    end for
    driveSort(entries)
    tkEqual(driveNames(entries), ["9 Lives", "10 Things", "Episode 9", "Episode 0010", "Episode 100", "Ép 1"])
end sub

sub test_drive_hidden_and_system_entries_are_left_out()
    for each name in ["$RECYCLE.BIN", ".device_info_8cdc30de", "System Volume Information", "._Film.mkv", ".Trashes", "lost+found", "Thumbs.db", "desktop.ini", "RECYCLER", ""]
        tkFalse(driveShown(name), name)
    end for
    tkTrue(driveShown("Ad Astra"))
end sub

sub test_drive_a_folder_lists_its_folders_then_its_videos()
    files = [
        { name: "Season 10", folder: true, size: 0 },
        { name: "Season 2", folder: true, size: 0 },
        { name: "$RECYCLE.BIN", folder: true, size: 0 },
        { name: "Film.mkv", folder: false, size: 4000 },
        { name: "Film.en.srt", folder: false, size: 30 },
        { name: "notes.txt", folder: false, size: 10 },
        { name: ".hidden.mkv", folder: false, size: 10 },
        { name: "Clip.MP4", folder: false, size: 900 },
        { name: "Trailer.webm", folder: false, size: 50 },
        { name: "Camera.ts", folder: false, size: 70 },
        { name: "Old.avi", folder: false, size: 70 },
        { name: "Big.mov", folder: false, size: -1294967296 }
    ]
    listed = driveEntries("ext1:/Films", files)
    ' Only what plays: subtitle files, notes, hidden files and what the
    ' Video node has no format for aren't shown.
    tkEqual(driveNames(listed), ["Season 2", "Season 10", "Big.mov", "Clip.MP4", "Film.mkv", "Trailer.webm"])
    tkEqual(listed[0], { path: "ext1:/Films/Season 2", name: "Season 2", folder: true, size: 0 })
    tkEqual(listed[3], { path: "ext1:/Films/Clip.MP4", name: "Clip.MP4", folder: false, size: 900 })
    tkEqual(listed[2].size, 0, "a size the drive gets wrong (past 2 GB) isn't shown")
    tkEqual(driveEntries("ext1:/", files)[2].path, "ext1:/Big.mov", "at the drive's top")
end sub

sub test_drive_a_folder_keeps_five_thousand_entries_folders_first()
    files = []
    for i = 1 to 3000
        files.Push({ name: "Video " + i.ToStr() + ".mkv", folder: false, size: 1 })
        files.Push({ name: "Folder " + i.ToStr(), folder: true, size: 0 })
    end for
    listed = driveEntries("ext1:/", files)
    tkEqual(listed.Count(), driveMaxEntries())
    tkEqual(listed[2999].name, "Folder 3000")
    tkEqual(listed[3000].name, "Video 1.mkv")
    tkEqual(listed[4999].name, "Video 2000.mkv")
end sub

sub test_drive_reads_a_folder_through_the_file_system()
    fs = fakeDrive({ "ext1:/": ["Films", "Ad Astra.mkv", "Ad Astra.en.srt", ".Spotlight-V100"], "ext1:/Films": [] }, { "ext1:/Ad Astra.mkv": 2048, "ext1:/Ad Astra.en.srt": 30 })
    read = driveRead(fs, "ext1:/")
    tkTrue(read.ok)
    tkEqual(read.names, ["Films", "Ad Astra.mkv", "Ad Astra.en.srt", ".Spotlight-V100"], "every name, for the subtitle files")
    tkEqual(read.files, [{ name: "Films", folder: true, size: 0 }, { name: "Ad Astra.mkv", folder: false, size: 2048 }, { name: "Ad Astra.en.srt", folder: false, size: 30 }])
    tkEqual(driveEntries("ext1:/", read.files)[1].path, "ext1:/Ad Astra.mkv")
    empty = driveRead(fs, "ext1:/Films")
    tkTrue(empty.ok, "an empty folder can be read")
    tkEqual(empty.files, [])
    tkFalse(driveRead(fs, "ext1:/Gone").ok, "a folder gone (the drive pulled out)")
    tkFalse(driveRead(fs, "ext1:/Ad Astra.mkv").ok, "a file isn't a folder")
end sub

sub test_drive_the_page_says_where_it_is()
    tkEqual(drivePlace("ext1:", "ext1:/"), "/")
    tkEqual(drivePlace("ext1:", "ext1:/Films/2019"), "/Films/2019")
    tkEqual(drivePlace("ext1:", "ext1:/Films/"), "/Films")
    tkEqual(drivePlaceLine("ext1:", "ext1:/", "437 GB free of 1.0 TB"), "/ · 437 GB free of 1.0 TB")
    tkEqual(drivePlaceLine("ext1:", "ext1:/", ""), "/", "how full it is, when the Roku says")
    tkEqual(drivePlaceLine("ext1:", "ext1:/Films", "437 GB free of 1.0 TB"), "/Films", "a folder's path alone")
    tkFalse(driveHasUp("ext1:", "ext1:/"))
    tkTrue(driveHasUp("ext1:", "ext1:/Films"))
    tkEqual(driveParent("ext1:/Films/Ad Astra.mkv"), "ext1:/Films")
    tkEqual(driveParent("ext1:/Ad Astra.mkv"), "ext1:/")
    tkEqual(driveFileName("ext1:/Films/Ad Astra.mkv"), "Ad Astra.mkv")
end sub

sub test_drive_its_name_and_how_full_it_is()
    tkEqual(driveName({ label: "KINGSTON", blocks: 100 }), "KINGSTON")
    tkEqual(driveName({ label: "" }), "USB drive")
    tkEqual(driveName(invalid), "USB drive")
    ' Roku's reference spells it "free-blocks"; brs-node, freeblocks.
    tkEqual(driveSpace({ blocksize: 4096, blocks: 244190646, "free-blocks": 114434000, usedblocks: 129756646 }), "437 GB free of 932 GB")
    tkEqual(driveSpace({ blocksize: 1, blocks: 2048, freeblocks: 1024 }), "1.0 KB free of 2.0 KB")
    tkEqual(driveSpace({ blocksize: 1, blocks: 0, freeblocks: 0 }), "", "a size not known")
    tkEqual(driveSpace({ blocksize: 512, blocks: 100 }), "")
end sub

sub test_drive_a_video_shows_its_name_without_its_extension()
    tkEqual(driveTitle("ext1:/Ad Astra (2019).mkv"), "Ad Astra (2019)")
    tkEqual(driveTitle("ext1:/Film.Final.Cut.mp4"), "Film.Final.Cut")
    tkEqual(driveTitle("ext1:/README"), "README")
    tkEqual(driveKind("Film.mkv"), "MKV")
    tkEqual(driveStreamFormat("ext1:/Film.MKV"), "mkv")
    tkEqual(driveStreamFormat("ext1:/Trailer.webm"), "mkv")
    tkEqual(driveStreamFormat("ext1:/Clip.mov"), "mp4")
    tkEqual(driveStreamFormat("ext1:/Clip.m4v"), "mp4")
end sub

sub test_drive_a_line_says_where_a_video_resumes()
    video = { path: "ext1:/Ad Astra.mkv", name: "Ad Astra.mkv", folder: false, size: 1503238554 }
    none = { positionMs: 0, durationMs: 0 }
    tkEqual(driveLine(video, none), { path: "ext1:/Ad Astra.mkv", title: "Ad Astra", folder: false, subtitle: "MKV", size: "1.4 GB", progress: 0 })
    line = driveLine(video, { positionMs: 2530000, durationMs: 7380000 })
    tkEqual(line.subtitle, "MKV · Resumes at 42:10")
    tkTrue(Abs(line.progress - 2530000 / 7380000) < 0.001)
    tkEqual(driveLine(video, { positionMs: 5000, durationMs: 7380000 }).progress, 0.02, "2 % at least, so it shows")
    tkEqual(driveLine(video, { positionMs: 5000, durationMs: 0 }).progress, 0, "its length not known")
    folder = driveLine({ path: "ext1:/Films", name: "Films", folder: true, size: 0 }, none)
    tkEqual(folder, { path: "ext1:/Films", title: "Films", folder: true, subtitle: "", size: "", progress: 0 })
end sub

sub test_drive_subtitle_files_beside_a_video_say_their_language_and_kind()
    found = driveSidecars("ext1:/media/Ad Astra.mkv", ["Ad Astra.mkv", "Ad Astra.srt", "Ad Astra.en.forced.srt", "Ad Astra.English.SDH.srt", "Ad Astra.fr.vtt", "Ad Astra 2.srt", "Other.srt", "Ad Astra.ass"])
    paths = []
    for each sidecar in found
        paths.Push(sidecar.path)
    end for
    tkEqual(paths, ["ext1:/media/Ad Astra.en.forced.srt", "ext1:/media/Ad Astra.English.SDH.srt", "ext1:/media/Ad Astra.fr.vtt", "ext1:/media/Ad Astra.srt"])
    tkEqual(found[0], { path: "ext1:/media/Ad Astra.en.forced.srt", codec: "subrip", language: "eng", title: "", forced: true })
    tkEqual(found[1].language, "eng")
    tkEqual(found[1].title, "SDH")
    tkEqual(found[2].codec, "webvtt")
    tkEqual(found[2].language, "fre")
    tkEqual(found[3].language, "")
    names = []
    for each sidecar in found
        names.Push(driveSidecarName(sidecar))
    end for
    tkEqual(names, ["English (Forced)", "English – SDH", "French", "SRT"])
    tkEqual(driveSidecars("ext1:/media/Film.srt.mkv", ["Film.srt"]), [], "a video whose name ends like a subtitle file's has none of its own")
    tkEqual(driveSidecars("ext1:/Film.mkv", ["film.pt-br.srt"])[0].language, "pt-br", "a code the app doesn't name stays")
    tkEqual(driveSidecarName({ codec: "webvtt", language: "", title: "", forced: true }), "Forced")
end sub

' Bytes from numbers.
function driveBytes(values as object) as object
    bytes = CreateObject("roByteArray")
    for each value in values
        bytes.Push(value)
    end for
    return bytes
end function

' text's bytes as UTF-16, little-endian or not, each character below
' U+10000.
function driveUtf16Bytes(text as string, little as boolean, mark as boolean) as object
    bytes = CreateObject("roByteArray")
    if mark and little then bytes.Append(driveBytes([&hFF, &hFE]))
    if mark and not little then bytes.Append(driveBytes([&hFE, &hFF]))
    for i = 1 to Len(text)
        unit = Asc(Mid(text, i, 1))
        if little
            bytes.Push(unit and &hFF)
            bytes.Push(unit >> 8)
        else
            bytes.Push(unit >> 8)
            bytes.Push(unit and &hFF)
        end if
    end for
    return bytes
end function

sub test_drive_a_subtitle_file_not_in_utf8_is_read_as_windows_1252()
    tkEqual(driveDecode(driveBytes([&h63, &h61, &h66, &hE9])), "caf" + Chr(&hE9))
    utf8 = CreateObject("roByteArray")
    utf8.FromAsciiString("caf" + Chr(&hE9))
    tkEqual(driveDecode(utf8), "caf" + Chr(&hE9), "UTF-8 as it is")
    ' Its curly quotes, dash and euro, which Latin-1 would make control
    ' characters.
    quoted = driveBytes([&h93, &h48, &h69, &h94, &h20, &h96, &h20, &h35, &h80])
    tkEqual(driveDecode(quoted), Chr(&h201C) + "Hi" + Chr(&h201D) + " " + Chr(&h2013) + " 5" + Chr(&h20AC))
    tkFalse(driveIsUtf8(driveBytes([&hC0, &h80])), "an overlong form")
    tkFalse(driveIsUtf8(driveBytes([&hED, &hA0, &h80])), "a surrogate")
    tkFalse(driveIsUtf8(driveBytes([&hE6, &h97])), "cut short")
end sub

sub test_drive_a_subtitle_file_in_utf16_is_read_by_its_mark_or_its_nul_bytes()
    text = "1" + Chr(10) + "00:00:01,000 --> 00:00:02,000" + Chr(10) + "Caf" + Chr(&hE9) + " " + Chr(&h65E5) + Chr(&h672C) + Chr(10)
    tkEqual(driveDecode(driveUtf16Bytes(text, true, true)), text, "little-endian, marked")
    tkEqual(driveDecode(driveUtf16Bytes(text, false, true)), text, "big-endian, marked")
    tkEqual(driveDecode(driveUtf16Bytes(text, true, false)), text, "little-endian, by its NULs")
    tkEqual(driveDecode(driveUtf16Bytes(text, false, false)), text, "big-endian, by its NULs")
    clapper = driveBytes([&hFF, &hFE, &h3C, &hD8, &hAC, &hDF])
    tkEqual(driveDecode(clapper), Chr(&h1F3AC), "a pair of surrogates")
    ' A lone surrogate and an odd last byte don't stop the rest.
    tkEqual(driveDecode(driveBytes([&hFF, &hFE, &h00, &hD8, &h41, &h00, &h42])), Chr(&hFFFD) + "A")
end sub

sub test_drive_a_subtitle_file_in_utf32_is_read_by_its_mark()
    tkEqual(driveDecode(driveBytes([&hFF, &hFE, 0, 0, &h43, 0, 0, 0, &hE9, 0, 0, 0])), "C" + Chr(&hE9))
    tkEqual(driveDecode(driveBytes([0, 0, &hFE, &hFF, 0, 0, 0, &h43, 0, 0, 0, &hE9])), "C" + Chr(&hE9))
end sub

sub test_drive_subrip_cues_as_the_player_draws_them()
    nl = Chr(13) + Chr(10)
    srt = Chr(&hFEFF) + "1" + nl + "00:00:01,500 --> 00:00:03,000" + nl + "<i>Hello</i>" + nl + "{\an8}there" + nl + nl
    srt = srt + "2" + nl + "00:01:02,250 --> 00:01:04,000" + nl + "Tom &amp; Jerry" + nl
    srt = srt + "3" + nl + "01:00:00,000 --> 01:00:01,000 X1:0" + nl + "Late" + nl
    srt = srt + "4" + nl + "00:00:05,000 --> 00:00:04,000" + nl + "Backwards" + nl + nl
    srt = srt + "5" + nl + "00:00:06,000 --> 00:00:07,000" + nl + nl
    cues = driveCues(srt)
    tkEqual(cues, [
        { startMs: 1500, endMs: 3000, text: "Hello" + Chr(10) + "there" },
        { startMs: 62250, endMs: 64000, text: "Tom & Jerry" },
        { startMs: 3600000, endMs: 3601000, text: "Late" }
    ], "no empty line before the next cue's number; one ending first, or saying nothing, is left out")
end sub

sub test_drive_webvtt_cues_as_the_player_draws_them()
    vtt = "WEBVTT" + Chr(10) + Chr(10) + "NOTE made by hand" + Chr(10) + Chr(10) + "intro" + Chr(10) + "00:01.000 --> 00:02.500 line:90%" + Chr(10) + "<c.yellow>Hi</c>" + Chr(10) + Chr(10) + "00:00:03.000 --> 00:00:04.000" + Chr(10) + "Bye" + Chr(10)
    tkEqual(driveCues(vtt), [{ startMs: 1000, endMs: 2500, text: "Hi" }, { startMs: 3000, endMs: 4000, text: "Bye" }])
    tkEqual(driveTimeMs("02:03.4"), 123400)
    tkEqual(driveTimeMs("1:02:03,045"), 3723045)
end sub

sub test_drive_cues_show_at_their_time_moved_by_sync()
    cues = [
        { startMs: 1000, endMs: 3000, text: "One" },
        { startMs: 2000, endMs: 5000, text: "Two" },
        { startMs: 6000, endMs: 7000, text: "Three" }
    ]
    tkEqual(driveCueText(cues, 500, 0), "", "before the first")
    tkEqual(driveCueText(cues, 1000, 0), "One")
    tkEqual(driveCueText(cues, 2500, 0), "One" + Chr(10) + "Two", "two at once, the first on top")
    tkEqual(driveCueText(cues, 3000, 0), "Two", "a cue's end is when it goes")
    tkEqual(driveCueText(cues, 5500, 0), "")
    tkEqual(driveCueText(cues, 6500, 0), "Three")
    tkEqual(driveCueText(cues, 7500, 1000), "Three", "a second later")
    tkEqual(driveCueText(cues, 5500, -500), "Three", "half a second earlier")
    tkEqual(driveCueText([], 1000, 0), "")
end sub

sub test_drive_keeps_where_each_video_stopped()
    section = tkRegistrySection()
    tkEqual(driveKept(section, "ext1:/Ad Astra.mkv"), { positionMs: 0, durationMs: 0 })
    tkTrue(driveRemember(section, "ext1:/Ad Astra.mkv", 2530000, 7380000))
    tkEqual(driveKept(section, "ext1:/Ad Astra.mkv"), { positionMs: 2530000, durationMs: 7380000 })
    tkEqual(driveKept(section, "ext1:/ad astra.mkv"), { positionMs: 0, durationMs: 0 }, "by its path")
    driveRemember(section, "ext1:/Clip.mp4", 1000, 0)
    tkEqual(driveKept(section, "ext1:/Clip.mp4"), { positionMs: 1000, durationMs: 0 }, "its length not known")
    ' Past 95 %, it was watched: it starts over next time.
    driveRemember(section, "ext1:/Ad Astra.mkv", 7020000, 7380000)
    tkEqual(driveKept(section, "ext1:/Ad Astra.mkv"), { positionMs: 0, durationMs: 0 })
    tkEqual(driveKept(section, "ext1:/Clip.mp4").positionMs, 1000, "the others stay")
    for i = 1 to 60
        driveRemember(section, "ext1:/Video " + i.ToStr() + ".mkv", i * 1000, 0)
    end for
    tkEqual(driveResumePoints(section).Count(), driveKeep(), "the latest 50")
    tkEqual(driveKept(section, "ext1:/Video 60.mkv").positionMs, 60000)
    tkEqual(driveKept(section, "ext1:/Video 10.mkv").positionMs, 0, "the oldest go")
    tkEqual(driveKept(section, "ext1:/Clip.mp4").positionMs, 0)
    section.Write("points", "not json")
    tkEqual(driveKept(section, "ext1:/Video 60.mkv").positionMs, 0, "what can't be read is nothing kept")
end sub

sub test_drive_a_video_s_place_is_kept_under_a_short_key()
    ' As scripts/smoke.mjs works it out for the walk's drive.
    tkEqual(driveResumeKey("ext1:/Clip.mp4"), "bcef50fa2d3a0d7e")
    tkEqual(driveResumeKey("ext1:/Films/Ad Astra (2019).mkv"), "0d68836cd9e81d78")
    tkEqual(driveResumeKey(""), "0000000000000000")
end sub

sub test_drive_audio_tracks_say_their_name_or_language()
    tkEqual(driveAudioName({ Track: "1", Language: "eng", Name: "", Format: "ac3" }, 0), "English (AC3)")
    tkEqual(driveAudioName({ Track: "2", Language: "fre", Name: "Commentary", Format: "" }, 1), "Commentary")
    tkEqual(driveAudioName({ Track: "3", Language: "", Name: "", Format: "" }, 2), "Track 3")
    tkEqual(driveAudioName({ Track: "4", Language: "eng", Name: "English AC3", Format: "AC3" }, 3), "English AC3", "its format not said twice")
    tkEqual(driveAudioName(invalid, 0), "Track 1")
end sub

sub test_drive_subtitles_list_the_file_s_own_tracks_then_the_files_beside_it()
    tracks = [{ TrackName: "mkv/2", Language: "eng", Description: "" }, { TrackName: "mkv/3", Language: "fre", Description: "French SDH" }, { TrackName: "", Language: "spa" }]
    sidecars = [{ path: "ext1:/Film.en.srt", label: "English" }]
    choices = driveSubtitleChoices(tracks, sidecars)
    tkEqual(choices.labels, ["Off", "English", "French SDH", "English"])
    tkEqual(choices.values, ["off", "track:mkv/2", "track:mkv/3", "file:0"], "a track the Roku names nothing isn't offered")
    tkEqual(driveSubtitleChoices(invalid, []).values, ["off"])
    tkEqual(driveTrackName({ TrackName: "mkv/4", Language: "", Description: "" }, 3), "Track 4")
end sub

sub test_drive_the_subtitles_shown_are_the_file_drawn_else_the_roku_s()
    tkEqual(driveSubtitleShown(0, "Off", ""), "file:0", "a file beside it, drawn")
    tkEqual(driveSubtitleShown(-1, "On", "mkv/2"), "track:mkv/2", "the file's own, with captions on")
    tkEqual(driveSubtitleShown(-1, "Off", "mkv/2"), "off", "captions off show none")
    tkEqual(driveSubtitleShown(-1, "On", ""), "off")
end sub

sub test_drive_subtitle_files_are_drawn_as_the_roku_s_caption_settings_say()
    tkEqual(driveCueStyle({ "Text/Size": "Default", "Text/Color": "Default", "Text/Opacity": "Default", "Text/Effect": "Default" }), { font: "font:MediumBoldSystemFont", color: "0xEBEBEBFF", shadow: true, shadowColor: "0x101010CC" })
    style = driveCueStyle({ "Text/Size": "Extra Large", "Text/Color": "Yellow", "Text/Opacity": "75%", "Text/Effect": "None" })
    tkEqual(style, { font: "font:ExtraLargeBoldSystemFont", color: "0xEBEB10BF", shadow: false, shadowColor: "0x101010CC" })
    tkEqual(driveCueStyle({ "Text/Color": "Black" }).shadowColor, "0xEBEBEBCC", "black text over a light shadow")
    tkEqual(driveCueStyle({}).font, "font:MediumBoldSystemFont", "nothing said")
    tkEqual(driveCueOptions(), ["Text/Size", "Text/Color", "Text/Opacity", "Text/Effect"])
end sub
