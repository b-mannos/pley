' Videos on a USB drive (docs/ROKU.md, "Videos on a USB drive"), as the
' Android TV app and the native Samsung app have them (DriveFolder.kt,
' LocalFolder.cs): a folder's folders, then its videos, each by name as a
' person sorts them ("Part 2" before "Part 10"), without hidden files and
' what Windows and macOS leave on a drive; the subtitle files beside a
' video, whose cues the player draws; and where each video stopped, kept on
' this Roku. The drive task (components/tasks/DriveTask) reads the drive
' with roFileSystem, here fs: a stand-in with its methods in tests.

' The containers the Video node plays from a drive as they are, by
' extension, with the streamFormat each is given (Roku's content metadata
' has mp4 for MOV and M4V too, and mkv for WebM). Not TS: it has no stream
' format for a transport stream on its own, only as HLS's segments.
function driveFormats() as object
    return { mkv: "mkv", webm: "mkv", mp4: "mp4", m4v: "mp4", mov: "mp4" }
end function

' What Windows and macOS leave on a drive, never shown (macOS's "._" files
' and .Trashes go as hidden ones).
function driveSystemNames() as object
    return { "system volume information": true, "$recycle.bin": true, "recycler": true, "lost+found": true, "thumbs.db": true, "desktop.ini": true }
end function

' A folder shows this many entries at most, folders first: each line is a
' ContentNode kept in the Roku's memory.
function driveMaxEntries() as integer
    return 5000
end function

' What the page calls the drive when it gives no name.
function driveDefaultName() as string
    return "USB drive"
end function

' The drive the app shows, of the volumes Roku lists: the first USB one
' ("ext1:", "ext2:"…), "" for none.
function driveVolume(volumes as dynamic) as string
    found = ""
    for each volume in httpArray(volumes)
        name = LCase(httpString(volume))
        if Left(name, 3) = "ext" and Right(name, 1) = ":" and Len(name) > 4
            if found = "" or Val(Mid(name, 4, Len(name) - 4)) < Val(Mid(found, 4, Len(found) - 4)) then found = name
        end if
    end for
    return found
end function

' The drive's top folder: "ext1:/".
function driveTop(volume as string) as string
    return volume + "/"
end function

' The page's title: the name the drive was given, else "USB drive".
function driveName(info as dynamic) as string
    data = httpObject(info)
    name = ""
    if data <> invalid then name = httpSentence(data.label)
    if name = "" then return driveDefaultName()
    return name
end function

' "437 GB free of 1.0 TB", from what roFileSystem's GetVolumeInfo says of
' the drive (Roku's reference spells its free blocks "free-blocks"); ""
' when it doesn't say.
function driveSpace(info as dynamic) as string
    data = httpObject(info)
    if data = invalid then return ""
    size = httpNumber(data.blocksize, 0) * 1#
    total = httpNumber(data.blocks, 0) * size
    freeBlocks = httpNumber(data["free-blocks"], httpNumber(data.freeblocks, -1))
    if freeBlocks < 0 then return ""
    free = freeBlocks * size
    if total <= 0 or free < 0 or free > total then return ""
    return fmtBytes(free) + " free of " + fmtBytes(total)
end function

' Where dir is on the drive, as a path: "/Films/2019"; at its top, "/".
function drivePlace(volume as string, dir as string) as string
    under = Mid(dir, Len(volume) + 1)
    while Right(under, 1) = "/" and Len(under) > 0
        under = Left(under, Len(under) - 1)
    end while
    if Left(under, 1) <> "/" then under = "/" + under
    return under
end function

' The line under the title: the folder's path, at the drive's top "/" and
' how full the drive is ("/ · 437 GB free of 1.0 TB").
function drivePlaceLine(volume as string, dir as string, space as string) as string
    place = drivePlace(volume, dir)
    if place = "/" and space <> "" then return "/ · " + space
    return place
end function

' Whether dir is a folder under the drive's top, which the page opens with
' ".." to go up.
function driveHasUp(volume as string, dir as string) as boolean
    return drivePlace(volume, dir) <> "/"
end function

function driveJoin(dir as string, name as string) as string
    if Right(dir, 1) = "/" then return dir + name
    return dir + "/" + name
end function

' The folder path is in: "ext1:/Films" for "ext1:/Films/Ad Astra.mkv".
function driveParent(path as string) as string
    for i = Len(path) to 1 step -1
        if Mid(path, i, 1) = "/"
            if i > 1 and Mid(path, i - 1, 1) = ":" then return Left(path, i)
            return Left(path, i - 1)
        end if
    end for
    return path
end function

' The last part of a path: "Ad Astra.mkv".
function driveFileName(path as string) as string
    for i = Len(path) to 1 step -1
        if Mid(path, i, 1) = "/" then return Mid(path, i + 1)
    end for
    return path
end function

' A name's extension in lower case: "mkv"; "" for none.
function driveExtension(name as string) as string
    for i = Len(name) to 1 step -1
        c = Mid(name, i, 1)
        if c = "." then return LCase(Mid(name, i + 1))
        if c = "/" then return ""
    end for
    return ""
end function

' The title a video shows: its file name without the extension.
function driveTitle(path as string) as string
    name = driveFileName(path)
    extension = driveExtension(name)
    if extension = "" or Len(extension) + 1 >= Len(name) then return name
    return Left(name, Len(name) - Len(extension) - 1)
end function

' A video's kind, as its line says it: "MKV".
function driveKind(name as string) as string
    return UCase(driveExtension(name))
end function

' The streamFormat the Video node is given for the video at path.
function driveStreamFormat(path as string) as string
    formats = driveFormats()
    extension = driveExtension(path)
    if formats.DoesExist(extension) then return formats[extension]
    return "mp4"
end function

' Hidden ones (a dot, a dollar) and the systems' own aren't shown.
function driveShown(name as string, system = invalid as dynamic) as boolean
    if name = "" then return false
    first = Left(name, 1)
    if first = "." or first = "$" then return false
    if system = invalid then system = driveSystemNames()
    return not system.DoesExist(LCase(name))
end function

' What sorts names as a person does, with plain string order: runs of
' digits as numbers (their zeros before dropped, then their length, so that
' "10" comes after "9"), the rest in capitals, as the other apps compare
' them.
function driveSortKey(name as string) as string
    runs = CreateObject("roRegex", "[0-9]+|[^0-9]+", "").MatchAll(name)
    parts = []
    for each piece in runs
        text = piece[0]
        if Asc(text) >= 48 and Asc(text) <= 57
            digits = text
            while Len(digits) > 0 and Left(digits, 1) = "0"
                digits = Mid(digits, 2)
            end while
            length = Len(digits)
            if length < 10 then parts.Push("0")
            parts.Push(length.ToStr())
            parts.Push(digits)
        else
            parts.Push(UCase(text))
        end if
    end for
    return parts.Join("")
end function

' entries ([{ name, … }]) by name as a person sorts them, in place.
sub driveSort(entries as object)
    for each entry in entries
        entry.sortKey = driveSortKey(entry.name)
    end for
    entries.SortBy("sortKey")
    for each entry in entries
        entry.Delete("sortKey")
    end for
end sub

' What dir holds, as its page lists it: its folders, then its videos, each
' by name as a person sorts them; other files aren't shown. files: what it
' holds ([{ name, folder, size }]). [{ path, name, folder, size }].
function driveEntries(dir as string, files as object) as object
    system = driveSystemNames()
    formats = driveFormats()
    folders = []
    videos = []
    for each file in files
        name = httpString(file.name)
        if driveShown(name, system)
            if file.folder = true
                folders.Push({ path: driveJoin(dir, name), name: name, folder: true, size: 0 })
            else if formats.DoesExist(driveExtension(name))
                size = httpNumber(file.size, 0)
                if size < 0 then size = 0
                videos.Push({ path: driveJoin(dir, name), name: name, folder: false, size: size })
            end if
        end if
    end for
    driveSort(folders)
    driveSort(videos)
    entries = folders
    for each video in videos
        if entries.Count() >= driveMaxEntries() then exit for
        entries.Push(video)
    end for
    while entries.Count() > driveMaxEntries()
        entries.Pop()
    end while
    return entries
end function

' Whether fs can read dir as a folder: it is there, and isn't a file.
function driveReadable(fs as object, dir as string) as boolean
    if not fs.Exists(dir) then return false
    info = fs.Stat(dir)
    if GetInterface(info, "ifAssociativeArray") = invalid then return true
    return LCase(httpString(info.type)) <> "file"
end function

' What dir holds, as fs reads it: { ok, names, files: [{ name, folder,
' size }] }, files only for names that show (a folder's or a file's
' details are read one by one). ok is false for a folder fs can't read
' (the drive pulled out, the folder gone).
function driveRead(fs as object, dir as string) as object
    if not driveReadable(fs, dir) then return { ok: false, names: [], files: [] }
    system = driveSystemNames()
    names = []
    files = []
    for each name in fs.GetDirectoryListing(dir)
        names.Push(name)
        if driveShown(name, system)
            info = fs.Stat(driveJoin(dir, name))
            if GetInterface(info, "ifAssociativeArray") <> invalid
                kind = LCase(httpString(info.type))
                if kind = "directory"
                    files.Push({ name: name, folder: true, size: 0 })
                else if kind = "file"
                    files.Push({ name: name, folder: false, size: httpNumber(info.size, 0) })
                end if
            end if
        end if
    end for
    return { ok: true, names: names, files: files }
end function

' A line of the drive's page: { path, title, folder, subtitle (a video's
' kind, or where it resumes: "MKV · Resumes at 42:10"), size ("1.4 GB";
' "" for a folder), progress (how far through, 2 % at least once started;
' 0 when its length isn't kept) }. kept: driveKept's.
function driveLine(entry as object, kept as object) as object
    if entry.folder then return { path: entry.path, title: entry.name, folder: true, subtitle: "", size: "", progress: 0 }
    kind = driveKind(entry.name)
    subtitle = kind
    progress = 0
    if kept.positionMs > 0
        subtitle = kind + " · Resumes at " + fmtClock(kept.positionMs)
        if kept.durationMs > 0
            progress = kept.positionMs / kept.durationMs
            if progress < 0.02 then progress = 0.02
            if progress > 1 then progress = 1
        end if
    end if
    size = ""
    if entry.size > 0 then size = fmtBytes(entry.size)
    return { path: entry.path, title: driveTitle(entry.path), folder: false, subtitle: subtitle, size: size, progress: progress }
end function

' --- Subtitle files beside a video ---

' Language names a file gives, as the codes the app names (fmtLanguages).
function driveLanguageNames() as object
    return {
        english: "eng", french: "fre", spanish: "spa", german: "ger", italian: "ita", portuguese: "por", japanese: "jpn",
        korean: "kor", chinese: "chi", arabic: "ara", russian: "rus", dutch: "dut", swedish: "swe", hindi: "hin", turkish: "tur"
    }
end function

' The two-letter codes of the languages the app names (ISO 639-1), as
' their ISO 639-2/B codes.
function driveLanguageCodes() as object
    return {
        ar: "ara", bg: "bul", ca: "cat", zh: "chi", hr: "hrv", cs: "cze", da: "dan", nl: "dut", en: "eng", fi: "fin", fr: "fre", de: "ger",
        el: "gre", he: "heb", hi: "hin", hu: "hun", id: "ind", it: "ita", ja: "jpn", ko: "kor", no: "nor", nb: "nor", fa: "per", pl: "pol",
        pt: "por", ro: "rum", ru: "rus", sr: "srp", sk: "slo", es: "spa", sv: "swe", th: "tha", tr: "tur", uk: "ukr", vi: "vie"
    }
end function

' A part of a subtitle file's name as a language: a code ("en", "eng",
' "pt-br") or an English name; "" for anything else. Two-letter codes of
' the languages the app names become theirs ("en" is "eng").
function driveLanguage(part as string) as string
    word = LCase(part)
    names = driveLanguageNames()
    if names.DoesExist(word) then return names[word]
    if not CreateObject("roRegex", "^[a-z]{2,3}(-[a-z]{2})?$", "").IsMatch(word) then return ""
    codes = driveLanguageCodes()
    base = Left(word, 2)
    if Len(word) = 2 and codes.DoesExist(base) then return codes[base]
    return word
end function

' The subtitle files beside the video at path among names (its folder's):
' those named as it is, then a language ("en", "eng", "English"),
' "forced", "sdh", in any order ("Film.en.forced.srt"; "Film 2.srt" is
' Film 2's). SubRip and WebVTT. [{ path, codec, language ("" for none),
' title ("SDH"; "" for none), forced }], by name.
function driveSidecars(path as string, names as object) as object
    dir = driveParent(path)
    stem = LCase(driveTitle(path))
    found = []
    for each name in names
        extension = driveExtension(name)
        codec = ""
        if extension = "srt" then codec = "subrip"
        if extension = "vtt" then codec = "webvtt"
        ends = Len(name) - Len(extension) - 1
        if codec <> "" and ends >= Len(stem) and LCase(Left(name, Len(stem))) = stem
            middle = Mid(name, Len(stem) + 1, ends - Len(stem))
            first = Left(middle, 1)
            if middle = "" or first = "." or first = "_"
                language = ""
                forced = false
                titles = []
                for each part in CreateObject("roRegex", "[._ ]", "").Split(middle)
                    word = LCase(part)
                    code = ""
                    if language = "" then code = driveLanguage(part)
                    if part = ""
                    else if word = "forced"
                        forced = true
                    else if word = "sdh" or word = "cc" or word = "hi"
                        titles.Push("SDH")
                    else if code <> ""
                        language = code
                    else
                        titles.Push(part)
                    end if
                end for
                found.Push({ path: driveJoin(dir, name), name: name, codec: codec, language: language, title: titles.Join(" "), forced: forced })
            end if
        end if
    end for
    driveSort(found)
    for each sidecar in found
        sidecar.Delete("name")
    end for
    return found
end function

' What Pley's menu calls a subtitle file, as the server's tracks are
' named: "English", "English – SDH", "English (Forced)", "Forced"; the
' file's kind for one whose name says nothing more ("SRT").
function driveSidecarName(sidecar as object) as string
    name = fmtLanguageName(sidecar.language, "")
    if sidecar.title <> ""
        if name <> "" then name = name + " – "
        name = name + sidecar.title
    end if
    if sidecar.forced
        if name = "" then return "Forced"
        return name + " (Forced)"
    end if
    if name <> "" then return name
    if sidecar.codec = "webvtt" then return "VTT"
    return "SRT"
end function

' What Pley's menu calls an audio track of the Video node's
' (availableAudioTracks): its name, else its language's, else "Track 2";
' then its format: "English (AC3)".
function driveAudioName(track as dynamic, index as integer) as string
    data = httpObject(track)
    if data = invalid then return "Track " + (index + 1).ToStr()
    name = httpSentence(data.Name)
    if name = "" then name = fmtLanguageName(httpString(data.Language), "")
    if name = "" then name = "Track " + (index + 1).ToStr()
    format = UCase(httpString(data.Format))
    if format <> "" and Instr(1, UCase(name), format) = 0 then name = name + " (" + format + ")"
    return name
end function

' What Pley's menu calls a subtitle track of the file's own, as the Video
' node lists it (availableSubtitleTracks): its description, else its
' language's name, else "Track 2".
function driveTrackName(track as dynamic, index as integer) as string
    data = httpObject(track)
    name = ""
    if data <> invalid
        name = httpSentence(data.Description)
        if name = "" then name = fmtLanguageName(httpString(data.Language), "")
    end if
    if name = "" then name = "Track " + (index + 1).ToStr()
    return name
end function

' Pley's Subtitles for a drive's video, as Android TV lists them: Off, the
' file's own tracks as the Roku lists them (tracks), which the Roku shows,
' then the files beside it (sidecars: [{ label }]), which Pley draws.
' { labels, values }, values "off", "track:<TrackName>" and "file:<index>".
function driveSubtitleChoices(tracks as dynamic, sidecars as object) as object
    choices = { labels: ["Off"], values: ["off"] }
    list = httpArray(tracks)
    for i = 0 to list.Count() - 1
        track = httpObject(list[i])
        if track <> invalid and httpString(track.TrackName) <> ""
            choices.labels.Push(driveTrackName(track, i))
            choices.values.Push("track:" + httpString(track.TrackName))
        end if
    end for
    for i = 0 to sidecars.Count() - 1
        choices.labels.Push(httpString(sidecars[i].label))
        choices.values.Push("file:" + i.ToStr())
    end for
    return choices
end function

' Which of driveSubtitleChoices shows: the file drawn (drawn, -1 for
' none), else the Roku's own track while its captions are on, else Off.
function driveSubtitleShown(drawn as integer, captionMode as string, track as string) as string
    if drawn >= 0 then return "file:" + drawn.ToStr()
    if LCase(captionMode) = "on" and track <> "" then return "track:" + track
    return "off"
end function

' The Roku's caption settings a drive's subtitle files are drawn with,
' read with roDeviceInfo's GetCaptionsOption (Settings → Accessibility →
' Captions style).
function driveCueOptions() as object
    return ["Text/Size", "Text/Color", "Text/Opacity", "Text/Effect"]
end function

' How a drive's subtitle files are drawn, as the Roku's caption settings
' say for the subtitles it draws itself (options: { "Text/Size": "Large",
' … }, driveCueOptions' values): { font, color, shadow, shadowColor }.
' "Default", or a setting the app doesn't know, is medium white text with
' a shadow; an effect of "None" has no shadow. Colours are broadcast safe,
' as the app's are: white is the app's text colour, black a near black.
function driveCueStyle(options as object) as object
    fonts = { "extra small": "font:SmallestBoldSystemFont", "small": "font:SmallBoldSystemFont", "medium": "font:MediumBoldSystemFont", "large": "font:LargeBoldSystemFont", "extra large": "font:ExtraLargeBoldSystemFont" }
    colors = { white: "EBEBEB", black: "101010", red: "EB1010", green: "10EB10", blue: "1010EB", yellow: "EBEB10", magenta: "EB10EB", cyan: "10EBEB" }
    alphas = { "25%": "40", "50%": "80", "75%": "BF", "100%": "FF" }
    size = LCase(httpString(options["Text/Size"]))
    color = LCase(httpString(options["Text/Color"]))
    opacity = httpString(options["Text/Opacity"])
    font = "font:MediumBoldSystemFont"
    if fonts.DoesExist(size) then font = fonts[size]
    rgb = colors.white
    if colors.DoesExist(color) then rgb = colors[color]
    alpha = "FF"
    if alphas.DoesExist(opacity) then alpha = alphas[opacity]
    shadow = colors.black
    if color = "black" then shadow = colors.white
    return { font: font, color: "0x" + rgb + alpha, shadow: LCase(httpString(options["Text/Effect"])) <> "none", shadowColor: "0x" + shadow + "CC" }
end function

' The most a subtitle file read beside a video may be, as on Samsung TVs.
function driveSubtitleMaxBytes() as integer
    return 2097152
end function

' A subtitle file's text from its bytes (a roByteArray), as on LG TVs and
' the Android app: UTF-16 or UTF-32 when it starts with that byte-order
' mark (Windows' "Unicode"), UTF-16 without one when its NUL bytes say so,
' else UTF-8, else Windows' Western code page (Windows-1252), the one older
' SubRip files are in.
function driveDecode(bytes as object) as string
    count = bytes.Count()
    b0 = -1
    b1 = -1
    b2 = -1
    b3 = -1
    if count > 0 then b0 = bytes[0]
    if count > 1 then b1 = bytes[1]
    if count > 2 then b2 = bytes[2]
    if count > 3 then b3 = bytes[3]
    if b0 = &hFF and b1 = &hFE and b2 = 0 and b3 = 0 then return driveUtf32(bytes, 4, true)
    if b0 = 0 and b1 = 0 and b2 = &hFE and b3 = &hFF then return driveUtf32(bytes, 4, false)
    if b0 = &hFF and b1 = &hFE then return driveUtf16(bytes, 2, true)
    if b0 = &hFE and b1 = &hFF then return driveUtf16(bytes, 2, false)
    order = driveUtf16Order(bytes)
    if order <> "" then return driveUtf16(bytes, 0, order = "le")
    if driveIsUtf8(bytes) then return bytes.ToAsciiString()
    out = CreateObject("roByteArray")
    high = driveWindows1252High()
    for i = 0 to count - 1
        b = bytes[i]
        if b < &h80
            out.Push(b)
        else if b < &hA0
            driveAppendUtf8(out, high[b - &h80])
        else
            driveAppendUtf8(out, b)
        end if
    end for
    return out.ToAsciiString()
end function

' Windows-1252's characters at 0x80 to 0x9F: the euro, curly quotes,
' dashes and the like (the five it leaves out stay as Latin-1 has them).
function driveWindows1252High() as object
    return [
        &h20AC, &h81, &h201A, &h192, &h201E, &h2026, &h2020, &h2021, &h2C6, &h2030, &h160, &h2039, &h152, &h8D, &h17D, &h8F,
        &h90, &h2018, &h2019, &h201C, &h201D, &h2022, &h2013, &h2014, &h2DC, &h2122, &h161, &h203A, &h153, &h9D, &h17E, &h178
    ]
end function

' Whether UTF-16 without its mark is little-endian ("le") or big ("be"),
' from its first 4 KB: where it is Latin text, times or numbers, every
' other byte is NUL, the high one. "" when it isn't UTF-16.
function driveUtf16Order(bytes as object) as string
    pairs = bytes.Count()
    if pairs > 4096 then pairs = 4096
    pairs = Int(pairs / 2)
    first = 0
    second = 0
    for i = 0 to pairs - 1
        if bytes[2 * i] = 0 then first = first + 1
        if bytes[2 * i + 1] = 0 then second = second + 1
    end for
    if pairs < 8 then return ""
    if second * 4 >= pairs and first * 8 <= second then return "le"
    if first * 4 >= pairs and second * 8 <= first then return "be"
    return ""
end function

' Whether bytes are well-formed UTF-8.
function driveIsUtf8(bytes as object) as boolean
    count = bytes.Count()
    i = 0
    while i < count
        b = bytes[i]
        if b < &h80
            i = i + 1
        else
            follow = 0
            least = 0
            if b >= &hC2 and b <= &hDF
                follow = 1
                least = &h80
            else if b >= &hE0 and b <= &hEF
                follow = 2
                least = &h800
            else if b >= &hF0 and b <= &hF4
                follow = 3
                least = &h10000
            else
                return false
            end if
            if i + follow >= count then return false
            point = b and (&h3F >> follow)
            for j = 1 to follow
                c = bytes[i + j]
                if c < &h80 or c > &hBF then return false
                point = (point << 6) or (c and &h3F)
            end for
            if point < least or point > &h10FFFF or (point >= &hD800 and point <= &hDFFF) then return false
            i = i + follow + 1
        end if
    end while
    return true
end function

' Appends the character point to out as UTF-8.
sub driveAppendUtf8(out as object, point as integer)
    if point < &h80
        out.Push(point)
    else if point < &h800
        out.Push(&hC0 or (point >> 6))
        out.Push(&h80 or (point and &h3F))
    else if point < &h10000
        out.Push(&hE0 or (point >> 12))
        out.Push(&h80 or ((point >> 6) and &h3F))
        out.Push(&h80 or (point and &h3F))
    else
        out.Push(&hF0 or (point >> 18))
        out.Push(&h80 or ((point >> 12) and &h3F))
        out.Push(&h80 or ((point >> 6) and &h3F))
        out.Push(&h80 or (point and &h3F))
    end if
end sub

' UTF-16 text from byte from; a lone surrogate reads as U+FFFD and an odd
' last byte is dropped, as on LG TVs.
function driveUtf16(bytes as object, from as integer, little as boolean) as string
    out = CreateObject("roByteArray")
    units = Int((bytes.Count() - from) / 2)
    i = 0
    while i < units
        unit = driveUnit16(bytes, from + 2 * i, little)
        if unit >= &hD800 and unit <= &hDBFF and i + 1 < units
            low = driveUnit16(bytes, from + 2 * i + 2, little)
            if low >= &hDC00 and low <= &hDFFF
                driveAppendUtf8(out, &h10000 + ((unit - &hD800) << 10) + (low - &hDC00))
                i = i + 2
                continue while
            end if
        end if
        if unit >= &hD800 and unit <= &hDFFF then unit = &hFFFD
        driveAppendUtf8(out, unit)
        i = i + 1
    end while
    return out.ToAsciiString()
end function

function driveUnit16(bytes as object, at as integer, little as boolean) as integer
    if little then return bytes[at] or (bytes[at + 1] << 8)
    return (bytes[at] << 8) or bytes[at + 1]
end function

' UTF-32 text from byte from; what isn't a character reads as U+FFFD, and
' a last word cut short is dropped.
function driveUtf32(bytes as object, from as integer, little as boolean) as string
    out = CreateObject("roByteArray")
    for i = 0 to Int((bytes.Count() - from) / 4) - 1
        at = from + 4 * i
        if little
            point = bytes[at] or (bytes[at + 1] << 8) or (bytes[at + 2] << 16) or (bytes[at + 3] << 24)
        else
            point = (bytes[at] << 24) or (bytes[at + 1] << 16) or (bytes[at + 2] << 8) or bytes[at + 3]
        end if
        if point < 0 or point > &h10FFFF or (point >= &hD800 and point <= &hDFFF) then point = &hFFFD
        driveAppendUtf8(out, point)
    end for
    return out.ToAsciiString()
end function

' A subtitle file's cues, SubRip or WebVTT, as the player draws them:
' [{ startMs, endMs, text }] by when they start; text without its styling
' (<i>, {\an8}), its lines apart. Cues with no text, or that end before
' they start, are left out.
function driveCues(text as string) as object
    if Left(text, 1) = Chr(&hFEFF) then text = Mid(text, 2)
    lines = CreateObject("roRegex", "\r\n|\r|\n", "").Split(text)
    times = CreateObject("roRegex", "^\s*((?:\d+:)?\d{1,2}:\d{2}[,.]\d{1,3})\s*-->\s*((?:\d+:)?\d{1,2}:\d{2}[,.]\d{1,3})", "")
    number = CreateObject("roRegex", "^\s*\d+\s*$", "")
    cues = []
    count = lines.Count()
    i = 0
    while i < count
        match = times.Match(lines[i])
        i = i + 1
        if match.Count() = 3
            body = []
            while i < count
                line = lines[i]
                if line.Trim() = "" or times.IsMatch(line) then exit while
                ' The next cue's number, where no empty line came before it.
                if number.IsMatch(line) and i + 1 < count and times.IsMatch(lines[i + 1]) then exit while
                line = driveCueLine(line)
                if line <> "" then body.Push(line)
                i = i + 1
            end while
            startMs = driveTimeMs(match[1])
            endMs = driveTimeMs(match[2])
            if body.Count() > 0 and endMs > startMs then cues.Push({ startMs: startMs, endMs: endMs, text: body.Join(Chr(10)) })
        end if
    end while
    cues.SortBy("startMs")
    return cues
end function

' A line of a cue as it shows: its tags and styling taken out, its
' entities written out.
function driveCueLine(line as string) as string
    line = CreateObject("roRegex", "<[^>]*>|\{\\[^}]*\}", "").ReplaceAll(line, "")
    line = line.Replace("&lt;", "<").Replace("&gt;", ">").Replace("&nbsp;", " ").Replace("&amp;", "&")
    return line.Trim()
end function

' A cue's time ("01:02:03,450", "02:03.4") in milliseconds.
function driveTimeMs(time as string) as integer
    parts = CreateObject("roRegex", "[:,.]", "").Split(time)
    fraction = parts.Pop()
    ms = Int(Val(fraction) * 1000 / (10 ^ Len(fraction)) + 0.5)
    seconds = 0
    for each part in parts
        seconds = seconds * 60 + Int(Val(part))
    end for
    return seconds * 1000 + ms
end function

' Where cues (driveCues') are at positionMs, moved offsetMs (later when
' positive, as subtitle sync moves them): the last cue started by then, or
' -1 before the first.
function driveCueBefore(cues as object, positionMs as integer, offsetMs as integer) as integer
    at = positionMs - offsetMs
    low = 0
    high = cues.Count() - 1
    found = -1
    while low <= high
        middle = Int((low + high) / 2)
        if cues[middle].startMs <= at
            found = middle
            low = middle + 1
        else
            high = middle - 1
        end if
    end while
    return found
end function

' What shows at positionMs: the text of the cues showing then, the one
' started first on top; "" for none.
function driveCueText(cues as object, positionMs as integer, offsetMs as integer) as string
    at = positionMs - offsetMs
    last = driveCueBefore(cues, positionMs, offsetMs)
    showing = []
    ' Cues that overlap stay a few back at most.
    i = last
    while i >= 0 and last - i < 4
        if cues[i].endMs > at then showing.Unshift(cues[i].text)
        i = i - 1
    end while
    return showing.Join(Chr(10))
end function

' --- Where each video stopped ---

' Played this far through, a video was watched: it starts over next time.
function driveWatched() as float
    return 0.95
end function

' How many videos' places are kept, the latest: the registry is small.
function driveKeep() as integer
    return 50
end function

' What a video's place is kept under, short in the registry: four 16-bit
' hashes of its path's characters, in hex, each kept under a prime so that
' it never outgrows BrightScript's 32-bit integers.
function driveResumeKey(path as string) as string
    primes = [65521, 65519, 65497, 65479]
    factors = [31, 37, 131, 257]
    hashes = [0, 0, 0, 0]
    for i = 1 to Len(path)
        c = Asc(Mid(path, i, 1))
        for j = 0 to 3
            hashes[j] = (hashes[j] * factors[j] + c) mod primes[j]
        end for
    end for
    key = ""
    for each hash in hashes
        key = key + driveHex4(hash)
    end for
    return key
end function

' A value from 0 to 65535 as four hex digits.
function driveHex4(value as integer) as string
    digits = "0123456789abcdef"
    text = ""
    for i = 1 to 4
        text = Mid(digits, (value mod 16) + 1, 1) + text
        value = value \ 16
    end for
    return text
end function

' Where the video at path stopped and how long it is: { positionMs,
' durationMs }, zeros when it wasn't kept or was watched. section is a
' roRegistrySection, or a stand-in with the same methods in tests.
function driveKept(section as object, path as string) as object
    key = driveResumeKey(path)
    for each point in driveResumePoints(section)
        if point[0] = key then return { positionMs: point[1], durationMs: point[2] }
    end for
    return { positionMs: 0, durationMs: 0 }
end function

' The video at path played to positionMs of durationMs (0: not known):
' kept as its latest, or forgotten once it was watched; false when the
' registry couldn't be written.
function driveRemember(section as object, path as string, positionMs as dynamic, durationMs as dynamic) as boolean
    key = driveResumeKey(path)
    position = Int(httpNumber(positionMs, 0))
    duration = Int(httpNumber(durationMs, 0))
    if duration < 0 then duration = 0
    kept = []
    for each point in driveResumePoints(section)
        if point[0] <> key then kept.Push(point)
    end for
    watched = duration > 0 and position >= duration * driveWatched()
    if position > 0 and not watched then kept.Push([key, position, duration])
    while kept.Count() > driveKeep()
        kept.Shift()
    end while
    if not section.Write("points", FormatJson(kept)) then return false
    return section.Flush()
end function

' The [key, position, duration] kept, oldest first.
function driveResumePoints(section as object) as object
    points = []
    if not section.Exists("points") then return points
    for each point in httpArray(httpJson(section.Read("points")))
        if GetInterface(point, "ifArray") <> invalid and point.Count() = 3
            key = httpString(point[0])
            position = Int(httpNumber(point[1], 0))
            duration = Int(httpNumber(point[2], 0))
            if key <> "" and position > 0 and duration >= 0 then points.Push([key, position, duration])
        end if
    end for
    return points
end function
