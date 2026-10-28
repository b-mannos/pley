' The Quality menu and how Automatic chooses (docs/QUALITY.md), without
' SceneGraph: the player tells a watch what its Video node does, and the
' watch answers a quality to open the stream at again, or "".
'
' Roku's own HLS adapts between a stream's variants, but pley's streams
' have one, so stepping down is pley's. The Video node tells of waits
' (state "buffering", bufferingStatus.isUnderrun) and of each HLS segment
' it downloaded (downloadedSegment: its size and how long it took), which
' measures the network. A file played as it is has no segments: a step
' down from it halves, and nothing measures a way back up. Roku doesn't
' tell an app of the frames it drops; what it can't decode, the server
' converts (the player's fallback for an error).

function aqMenu() as object
    return [
        { id: "original", label: "Original", kbps: 0, lines: 0 },
        { id: "auto", label: "Automatic", kbps: 0, lines: 0 },
        { id: "30m", label: "30 Mbps 4K", kbps: 30000, lines: 2160 },
        { id: "15m", label: "15 Mbps 4K", kbps: 15000, lines: 2160 },
        { id: "20m", label: "20 Mbps 1080p", kbps: 20000, lines: 1080 },
        { id: "12m", label: "12 Mbps 1080p", kbps: 12000, lines: 1080 },
        { id: "8m", label: "8 Mbps 1080p", kbps: 8000, lines: 1080 },
        { id: "4m", label: "4 Mbps 720p", kbps: 4000, lines: 720 },
        { id: "2m", label: "2 Mbps 480p", kbps: 2000, lines: 480 }
    ]
end function

' Each quality's hint in Settings.
function aqHint(id as string) as string
    if id = "original" then return "The file as it is. A slow network waits."
    if id = "auto" then return "The original, lower only when it doesn't play well."
    return "Converted by the server, when the file is over it."
end function

function aqFind(id as dynamic) as dynamic
    for each quality in aqMenu()
        if quality.id = httpString(id) then return quality
    end for
    return invalid
end function

function aqLabel(id as dynamic) as string
    quality = aqFind(id)
    if quality = invalid then return "Original"
    return quality.label
end function

' A quality's bitrate in kbps; 0 for Original and Automatic.
function aqKbps(id as dynamic) as integer
    quality = aqFind(id)
    if quality = invalid then return 0
    return quality.kbps
end function

function aqLines(id as dynamic) as integer
    quality = aqFind(id)
    if quality = invalid then return 0
    return quality.lines
end function

' The setting's quality: Automatic unless another was chosen.
function aqFromSetting(value as dynamic) as string
    if aqFind(value) <> invalid then return value
    return "auto"
end function

' The converted qualities worth offering for a file (docs/QUALITY.md):
' those with fewer lines than its fileLines, or at most 80 % of its
' bitrate, as a copy a little lighter plays no better than the file;
' every one, in the menu's order, when neither is known. Named by the
' file's lines when taller (aqMenuFor), they go by those lines, tallest
' first, then by bitrate: a 4K file keeps the menu's order.
function aqOffered(fileKbps as float, fileLines as integer) as object
    menu = ["30m", "15m", "20m", "12m", "8m", "4m", "2m"]
    if fileKbps <= 0 and fileLines <= 0 then return menu
    named = aqNamedLines(0, fileLines)
    copies = []
    for each id in menu
        fewer = fileLines > 0 and aqLines(id) < fileLines
        lighter = fileKbps > 0 and aqKbps(id) <= fileKbps * 0.8
        if fewer or lighter
            shown = aqLines(id)
            if named > 0 and named < shown then shown = named
            ' In order: after every copy taller, or as tall and of more bitrate.
            at = copies.Count()
            for i = copies.Count() - 1 to 0 step -1
                if copies[i].lines < shown or (copies[i].lines = shown and copies[i].kbps < aqKbps(id)) then at = i
            end for
            copies.Push(invalid)
            for i = copies.Count() - 1 to at + 1 step -1
                copies[i] = copies[i - 1]
            end for
            copies[at] = { id: id, lines: shown, kbps: aqKbps(id) }
        end if
    end for
    ids = []
    for each copy in copies
        ids.Push(copy.id)
    end for
    return ids
end function

' The lines qualities are named by for a file width × height: 2160, 1080,
' 720 or 480, by its width too (a 1920 × 800 film is 1080p); 0 when
' neither is known.
function aqNamedLines(width as integer, height as integer) as integer
    if width <= 0 and height <= 0 then return 0
    if width >= 3800 or height >= 2100 then return 2160
    if width >= 1800 or height >= 1000 then return 1080
    if width >= 1200 or height >= 700 then return 720
    return 480
end function

' The player's Quality menu for a file: Original, Automatic and the
' qualities offered for it ([{ id, label }]), a quality taller than the
' file named by the file's own lines ("8 Mbps 720p" for a 720p file).
function aqMenuFor(file as dynamic) as object
    width = 0
    if file <> invalid
        width = Int(httpNumber(file.width, 0))
        if width <= 0
            for each stream in httpArray(file.streams)
                if width <= 0 and GetInterface(stream, "ifAssociativeArray") <> invalid and httpString(stream.kind) = "video" then width = Int(httpNumber(stream.width, 0))
            end for
        end if
    end if
    named = aqNamedLines(width, aqFileLines(file))
    menu = [{ id: "original", label: "Original" }, { id: "auto", label: "Automatic" }]
    for each id in aqOffered(aqFileKbps(file), aqFileLines(file))
        label = aqLabel(id)
        if named > 0 and aqLines(id) > named
            lines = urlValue(named) + "p"
            if named >= 2160 then lines = "4K"
            label = urlValue(aqKbps(id) \ 1000) + " Mbps " + lines
        end if
        menu.Push({ id: id, label: label })
    end for
    return menu
end function

' The qualities Automatic steps through below the original, highest
' first: those offered for the file, no higher than the server's own
' quality for it (auto_quality, "" for none) in bitrate and lines, each
' of less bitrate than the one above (a 4K file goes 30 Mbps 4K, 15 Mbps
' 4K, then 12 Mbps 1080p).
function aqLadder(fileKbps as float, fileLines as integer, cap as string) as object
    most = aqKbps(cap)
    tallest = aqLines(cap)
    ladder = []
    below = 0
    for each id in aqOffered(fileKbps, fileLines)
        kbps = aqKbps(id)
        if (most <= 0 or (kbps <= most and aqLines(id) <= tallest)) and (below = 0 or kbps < below)
            ladder.Push(id)
            below = kbps
        end if
    end for
    return ladder
end function

function aqHas(ids as object, id as string) as boolean
    for each found in ids
        if found = id then return true
    end for
    return false
end function

' Where Automatic starts under a remembered ceiling (kbps, 0 for none):
' the best quality at or under it, 2 Mbps 480p at least; "" for none.
function aqStartQuality(ceilingKbps as float) as string
    if ceilingKbps <= 0 then return ""
    for each id in ["30m", "20m", "15m", "12m", "8m", "4m", "2m"]
        if aqKbps(id) <= ceilingKbps then return id
    end for
    return "2m"
end function

' A file's bitrate in kbps and lines, from its details (0 when unknown).
function aqFileKbps(file as dynamic) as float
    if file = invalid then return 0
    return httpNumber(file.bitrate, 0) / 1000.0
end function

function aqFileLines(file as dynamic) as integer
    if file = invalid then return 0
    lines = Int(httpNumber(file.height, 0))
    if lines > 0 then return lines
    for each stream in httpArray(file.streams)
        if GetInterface(stream, "ifAssociativeArray") <> invalid and httpString(stream.kind) = "video" then return Int(httpNumber(stream.height, 0))
    end for
    return 0
end function

' How a playback is asked for: { automatic, quality, lowered, mode,
' maxBitrate, maxHeight } (0 for no limit). choice is the setting's
' quality; again Automatic's quality when it opens the title again ("" for
' none); ceilingKbps what it remembers of a slow network (0 for nothing).
' A quality is asked for converted only when it makes the file smaller,
' else the file plays as it is, as on the other apps.
function aqAsk(choice as string, again as string, ceilingKbps as float, file as dynamic) as object
    asked = { automatic: choice = "auto" or again <> "", quality: "original", lowered: false, mode: "auto", maxBitrate: 0, maxHeight: 0 }
    wanted = again
    if wanted = ""
        if choice = "auto"
            wanted = aqStartQuality(ceilingKbps)
            asked.lowered = wanted <> ""
        else if choice <> "original"
            wanted = choice
        end if
    end if
    if wanted = "" or wanted = "original" or aqFind(wanted) = invalid
        asked.lowered = false
        return asked
    end if
    if again = "" and not aqHas(aqOffered(aqFileKbps(file), aqFileLines(file)), wanted)
        ' Already under it: the file as it is.
        asked.lowered = false
        return asked
    end if
    asked.quality = wanted
    asked.mode = "transcode"
    asked.maxBitrate = aqKbps(wanted)
    asked.maxHeight = aqLines(wanted)
    return asked
end function

' The quality after a play issue and the ceiling it came from: { quality,
' ceiling }, or invalid when there is nothing lower. The ceiling is what
' the network carried (the bitrate playing, or the segments' speed when
' lower) times 0.75, never above the server's own quality for the file;
' the quality is the best under it, always lower than the one playing, 2
' Mbps 480p at least. Without a measure, the bitrate playing halves.
function aqStepDown(current as string, playingKbps as float, networkKbps as float, fileKbps as float, fileLines as integer, cap as string) as dynamic
    carried = 0.0
    if networkKbps > 0 and playingKbps > 0
        carried = networkKbps
        if playingKbps < carried then carried = playingKbps
    else if networkKbps > 0
        carried = networkKbps
    else if playingKbps > 0
        carried = playingKbps * 0.5
    end if
    below = playingKbps
    if current <> "original" then below = aqKbps(current)
    candidates = []
    for each id in aqLadder(fileKbps, fileLines, cap)
        if below <= 0 or aqKbps(id) < below then candidates.Push(id)
    end for
    if candidates.Count() = 0 then return invalid
    if carried <= 0
        ' Nothing known of the file or the network: a middling quality.
        chosen = candidates[0]
        if aqHas(candidates, "8m") then chosen = "8m"
        return { quality: chosen, ceiling: aqKbps(chosen) }
    end if
    ceiling = carried * 0.75
    for each id in candidates
        if aqKbps(id) <= ceiling then return { quality: id, ceiling: ceiling }
    end for
    return { quality: candidates[candidates.Count() - 1], ceiling: ceiling }
end function

' The next quality up and the bitrate it needs carried (before the
' headroom): { quality, kbps }, or invalid at the original or when the
' file's bitrate isn't known.
function aqStepUp(current as string, fileKbps as float, fileLines as integer, cap as string) as dynamic
    if current = "original" then return invalid
    up = []
    for each id in aqLadder(fileKbps, fileLines, cap)
        if aqKbps(id) > aqKbps(current) then up.Push(id)
    end for
    if up.Count() > 0 then return { quality: up[up.Count() - 1], kbps: aqKbps(up[up.Count() - 1]) }
    need = fileKbps
    if aqKbps(cap) > 0 then need = aqKbps(cap)
    if need <= 0 then return invalid
    return { quality: "original", kbps: need }
end function

' --- The watch over one title ---

' now is the player's clock in ms; quality what plays ("original" or a
' quality's id), at playingKbps (0 unknown); the file's bitrate and lines;
' cap the server's own quality for it; lowered when it started under a
' remembered ceiling (counted as a step down).
function aqWatch(now as integer, quality as string, playingKbps as float, fileKbps as float, fileLines as integer, cap as string, lowered as boolean) as object
    watch = {
        quality: quality,
        playingKbps: playingKbps,
        fileKbps: fileKbps,
        fileLines: fileLines,
        cap: cap,
        changedAt: now,
        issueAt: now,
        downAt: -1,
        ceiling: 0.0,
        playingSince: -1,
        waitSince: -1,
        waitCounted: false,
        shortWaits: [],
        segments: [],
        lastPosition: -1,
        lastAt: -1
    }
    if lowered then watch.downAt = now
    return watch
end function

' Playing again at quality: a new stream of the same title.
sub aqRestarted(watch as object, now as integer, quality as string, playingKbps as float)
    watch.quality = quality
    watch.playingKbps = playingKbps
    watch.changedAt = now
    watch.segments = []
    watch.lastPosition = -1
    aqSeeked(watch)
end sub

' Where the video is (ms), as the Video node says each second: a jump
' ahead of the time gone by, or back, is a seek in Roku's own controls.
sub aqPosition(watch as object, now as integer, positionMs as integer)
    if watch.lastPosition >= 0
        moved = positionMs - watch.lastPosition
        if moved - (now - watch.lastAt) > 3000 or moved < -1000
            ' A wait in progress was the seek's; one now needs 2 s of playing first.
            watch.waitSince = -1
            watch.waitCounted = false
            if watch.playingSince >= 0 then watch.playingSince = now
        end if
    end if
    watch.lastPosition = positionMs
    watch.lastAt = now
end sub

' A seek, a start or a pause: the wait that follows isn't the network's.
sub aqSeeked(watch as object)
    watch.playingSince = -1
    watch.waitSince = -1
    watch.waitCounted = false
end sub

' The Video node's state changed. underrun is bufferingStatus.isUnderrun
' (invalid when the Roku doesn't say). Answers a quality to restart at, or "".
function aqState(watch as object, now as integer, state as string, underrun as dynamic) as string
    issue = false
    if state = "playing"
        if watch.waitSince >= 0 and not watch.waitCounted
            ' A short wait: two within a minute are a play issue.
            kept = [now]
            for each at in watch.shortWaits
                if now - at <= 60000 then kept.Push(at)
            end for
            watch.shortWaits = kept
            if kept.Count() >= 2
                watch.shortWaits = []
                issue = true
            end if
        end if
        watch.waitSince = -1
        if watch.playingSince < 0 then watch.playingSince = now
    else if state = "buffering"
        ' Only after 2 s of playing (not a start or a seek), and not when
        ' the Roku says it isn't for want of data.
        flowing = watch.playingSince >= 0 and now - watch.playingSince >= 2000
        notUnderrun = GetInterface(underrun, "ifBoolean") <> invalid and not underrun
        if flowing and not notUnderrun and watch.waitSince < 0
            watch.waitSince = now
            watch.waitCounted = false
        end if
        watch.playingSince = -1
    else
        aqSeeked(watch)
    end if
    if issue then return aqIssue(watch, now)
    return ""
end function

' A segment the Video node downloaded: bytes in ms.
sub aqSegment(watch as object, now as integer, bytes as float, ms as float)
    if bytes <= 0 or ms <= 0 then return
    kept = []
    for each segment in watch.segments
        if now - segment.at <= 240000 then kept.Push(segment)
    end for
    kept.Push({ at: now, bytes: bytes, ms: ms })
    watch.segments = kept
end sub

' What the network carried, in kbps, over the segments since `since`
' (their bytes over the time they took); 0 with fewer than `least`.
function aqCarried(watch as object, since as integer, least as integer) as float
    bytes = 0.0
    ms = 0.0
    count = 0
    for each segment in watch.segments
        if segment.at >= since
            bytes = bytes + segment.bytes
            ms = ms + segment.ms
            count = count + 1
        end if
    end for
    if count < least or ms <= 0 then return 0
    return bytes * 8 / ms
end function

' Once a second while it plays or waits: a wait of 2 s is a play issue,
' and a quiet network may bring the quality back up. Answers a quality or "".
function aqTick(watch as object, now as integer) as string
    if watch.waitSince >= 0 and not watch.waitCounted and now - watch.waitSince >= 2000
        watch.waitCounted = true
        return aqIssue(watch, now)
    end if
    if watch.quality = "original" or watch.playingSince < 0 then return ""
    if now - watch.issueAt < 180000 or now - watch.changedAt < 180000 then return ""
    if watch.downAt >= 0 and now - watch.downAt < 300000 then return ""
    up = aqStepUp(watch.quality, watch.fileKbps, watch.fileLines, watch.cap)
    if up = invalid then return ""
    since = watch.issueAt
    if watch.changedAt > since then since = watch.changedAt
    ' 20 s of video at least.
    carried = aqCarried(watch, since, 10)
    if carried <= 0 or carried < 1.6 * up.kbps then return ""
    watch.changedAt = now
    return up.quality
end function

function aqIssue(watch as object, now as integer) as string
    watch.issueAt = now
    if now - watch.changedAt < 20000 then return ""
    network = aqCarried(watch, now - 20000, 1)
    down = aqStepDown(watch.quality, watch.playingKbps, network, watch.fileKbps, watch.fileLines, watch.cap)
    if down = invalid then return ""
    watch.changedAt = now
    watch.downAt = now
    watch.ceiling = down.ceiling
    return down.quality
end function

' --- Remembered on this Roku ---

' The ceiling Automatic remembers for server (kbps), 0 past 30 minutes.
' atSeconds is the wall clock; section a roRegistrySection or a stand-in.
function aqRemembered(section as object, server as string, atSeconds as integer) as float
    found = httpObject(httpJson(section.Read("ceilings")))
    if found = invalid then return 0
    entry = httpObject(found[server])
    if entry = invalid then return 0
    at = httpNumber(entry.at, 0)
    if atSeconds - at > 1800 or at - atSeconds > 60 then return 0
    return httpNumber(entry.kbps, 0)
end function

' Remembers kbps for server (0 forgets it).
sub aqRemember(section as object, server as string, kbps as float, atSeconds as integer)
    found = httpObject(httpJson(section.Read("ceilings")))
    kept = {}
    if found <> invalid
        ' Only those still in force: the registry is small.
        for each key in found
            entry = httpObject(found[key])
            if entry <> invalid and atSeconds - httpNumber(entry.at, 0) <= 1800 then kept[key] = entry
        end for
    end if
    if kbps > 0
        kept[server] = { kbps: Int(kbps), at: atSeconds }
    else
        kept.Delete(server)
    end if
    section.Write("ceilings", FormatJson(kept))
    section.Flush()
end sub

' The quality setting, in the same section.
function aqChoice(section as object) as string
    return aqFromSetting(section.Read("quality"))
end function

sub aqChoose(section as object, id as string)
    section.Write("quality", aqFromSetting(id))
    section.Flush()
end sub
