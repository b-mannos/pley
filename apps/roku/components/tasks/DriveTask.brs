' The USB drive (lib/Drive.brs): which one is plugged in, looked at as the
' task starts, as Roku mounts or unmounts one (roFileSystemEvent), and as
' Home, the server out of reach or the drive's page shows, for a Roku that
' tells nothing; and what the screens ask of it: a folder's lines, the
' subtitle files beside a video, and a subtitle file's cues. roFileSystem
' works only in a Task, and a folder of thousands of entries, or a drive
' waking up, takes a while: the render thread never waits on it.

sub init()
    m.top.functionName = "driveRun"
end sub

sub driveRun()
    port = CreateObject("roMessagePort")
    m.top.observeField("request", port)
    m.fs = CreateObject("roFileSystem")
    m.fs.SetMessagePort(port)
    ' The drive's name and lines in the language the scene shows, as the
    ' API task's answers are.
    i18nUse(m.global.language)
    m.global.observeField("language", port)
    driveLook()
    m.top.ready = true
    while true
        ' A second at a time: brs-node looks for a volume's events only while
        ' a wait can end, and a Roku loses nothing by it.
        msg = wait(1000, port)
        ' Its type says roFileSystemEvent on a Roku, roFilesystemEvent in
        ' brs-node.
        kind = LCase(type(msg))
        if kind = "rosgnodeevent"
            if msg.getField() = "request" then driveAccept(msg.getData())
            if msg.getField() = "language" then i18nUse(msg.getData())
        else if kind = "rofilesystemevent"
            ' A drive plugged in or pulled out: the volumes say which is left.
            driveLook()
        end if
    end while
end sub

' The drive plugged in now, its name and how full it is: told when it
' changes.
sub driveLook()
    volume = driveVolume(m.fs.GetVolumeList())
    drive = { volume: volume, name: driveDefaultName(), space: "" }
    if volume <> ""
        info = m.fs.GetVolumeInfo(driveTop(volume))
        drive.name = driveName(info)
        drive.space = driveSpace(info)
    end if
    if FormatJson(drive) <> FormatJson(m.top.drive) then m.top.drive = drive
end sub

sub driveAccept(request as dynamic)
    if request = invalid then return
    spec = request.spec
    if spec = invalid then spec = {}
    op = httpString(spec.op)
    if op = "look"
        driveLook()
    else if op = "list"
        driveList(request, httpString(spec.folder))
    else if op = "sidecars"
        driveSidecarsOf(request, httpString(spec.path))
    else if op = "cues"
        driveCuesOf(request, httpString(spec.path))
    else
        request.result = { ok: false, error: "unknown", data: invalid }
    end if
end sub

' A folder's lines (folder "": the drive's top), as ContentNodes in the
' request's content: ".." first under the top, then its folders and its
' videos, each saying where it resumes. The answer's data: { drive,
' folder, line (where it is, under the page's title), up, count }; error
' "none" (no drive, or not the one the folder is on) or "unreadable".
sub driveList(request as object, folder as string)
    ' Looked at again: a drive plugged in or pulled out may have told nothing.
    driveLook()
    drive = m.top.drive
    top = driveTop(drive.volume)
    if drive.volume = "" or (folder <> "" and LCase(Left(folder, Len(top))) <> top)
        request.result = { ok: false, error: "none", data: { drive: drive } }
        return
    end if
    if folder = "" then folder = top
    data = { drive: drive, folder: folder, line: drivePlaceLine(drive.volume, folder, drive.space), up: driveHasUp(drive.volume, folder), count: 0 }
    read = driveRead(m.fs, folder)
    if not read.ok
        request.result = { ok: false, error: "unreadable", data: data }
        return
    end if
    entries = driveEntries(folder, read.files)
    content = CreateObject("roSGNode", "ContentNode")
    if data.up then driveNode(content, { path: "", title: "..", subtitle: t("drive.up"), size: "", progress: 0, icon: "corner-left-up" })
    ' Where videos stopped, by their key: looked up only when some are kept.
    kept = {}
    for each point in driveResumePoints(CreateObject("roRegistrySection", "driveResume"))
        kept[point[0]] = { positionMs: point[1], durationMs: point[2] }
    end for
    none = { positionMs: 0, durationMs: 0 }
    for each entry in entries
        place = none
        if not entry.folder and kept.Count() > 0
            key = driveResumeKey(entry.path)
            if kept.DoesExist(key) then place = kept[key]
        end if
        line = driveLine(entry, place)
        line.icon = "file-video"
        if line.folder then line.icon = "folder"
        driveNode(content, line)
    end for
    data.count = entries.Count()
    request.content = content
    request.result = { ok: true, status: 200, data: data, error: "" }
end sub

' A line of the page as a ContentNode, in two native calls: url its path
' ("" for ".."), shortDescriptionLine1 what it is ("folder", "video",
' "up"), description what its line says under its name,
' shortDescriptionLine2 its size, hdPosterUrl its icon, and progress how
' far through it is (0 to 1).
sub driveNode(content as object, line as object)
    kind = "video"
    if line.path = "" then kind = "up"
    if line.folder = true then kind = "folder"
    node = content.createChild("ContentNode")
    node.setFields({ url: line.path, title: line.title, description: line.subtitle, shortDescriptionLine1: kind, shortDescriptionLine2: line.size, hdPosterUrl: "pkg:/images/icons/" + line.icon + ".png" })
    node.addFields({ progress: line.progress })
end sub

' The subtitle files beside the video at path: data { sidecars: [{ path,
' label }] }, by name.
sub driveSidecarsOf(request as object, path as string)
    found = []
    dir = driveParent(path)
    if dir <> path and driveReadable(m.fs, dir)
        for each sidecar in driveSidecars(path, m.fs.GetDirectoryListing(dir))
            found.Push({ path: sidecar.path, label: driveSidecarName(sidecar) })
        end for
    end if
    request.result = { ok: true, status: 200, data: { sidecars: found }, error: "" }
end sub

' A subtitle file's cues (driveCues), 2 MB of it at most: data { cues };
' error "large" or "unreadable".
sub driveCuesOf(request as object, path as string)
    info = m.fs.Stat(path)
    size = -1
    if GetInterface(info, "ifAssociativeArray") <> invalid and LCase(httpString(info.type)) = "file" then size = httpNumber(info.size, -1)
    if size > driveSubtitleMaxBytes()
        request.result = { ok: false, error: "large", data: invalid, message: t("drive.subtitles.tooLarge") }
        return
    end if
    bytes = CreateObject("roByteArray")
    if size < 0 or not bytes.ReadFile(path)
        request.result = { ok: false, error: "unreadable", data: invalid, message: t("drive.subtitles.unreadable") }
        return
    end if
    request.result = { ok: true, status: 200, data: { cues: driveCues(driveDecode(bytes)) }, error: "" }
end sub
