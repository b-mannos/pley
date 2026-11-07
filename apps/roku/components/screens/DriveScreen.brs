' A folder of the USB drive (docs/ROKU.md, "Videos on a USB drive"), as
' Android TV and the native Samsung app show it: the drive's name ("USB
' drive" when it has none), where the folder is under it ("/Films/2019";
' at the top, "/" and how full the drive is), then a line each: ".." first
' under the drive's top, its folders, then its videos with their kind, or
' where they resume with a bar of how far through, and their size. OK on a
' folder opens it over this page (Back comes back to its line), on a video
' plays it in Pley's player from where it stopped. No drive, an empty
' folder and one that can't be read say so in the middle of the page. The
' drive task reads the folder; a drive plugged in or pulled out shows here
' at once.

sub init()
    m.title = m.top.findNode("title")
    m.place = m.top.findNode("place")
    m.lines = m.top.findNode("lines")
    m.message = m.top.findNode("message")
    m.messageIcon = m.top.findNode("messageIcon")
    m.messageTitle = m.top.findNode("messageTitle")
    m.messageText = m.top.findNode("messageText")
    m.loading = m.top.findNode("loading")
    m.title.text = driveDefaultName()
    m.loading.text = t("common.states.loading")
    m.started = false
    ' Which asking for the folder an answer must be for.
    m.asked = 0
    ' The drive the page shows ("" for none), and the video played from it,
    ' whose line says where it resumes once the player closes.
    m.volume = ""
    m.played = ""
    m.lines.observeField("itemSelected", "onLine")
    m.top.observeField("active", "onActive")
    m.global.drive.observeFieldScoped("drive", "onDrive")
end sub

sub onActive()
    if not m.top.active then return
    if not m.started
        m.started = true
        load()
        return
    end if
    if m.played <> "" then playedLine()
    focus()
end sub

' The folder, asked of the drive task (once it listens).
sub load()
    if not m.global.drive.ready
        m.global.drive.observeFieldScoped("ready", "load")
        return
    end if
    m.global.drive.unobserveFieldScoped("ready")
    m.asked = m.asked + 1
    if m.lines.content = invalid then m.loading.visible = true
    uiDrive({ op: "list", folder: httpString(m.top.params.folder), asked: m.asked }, "onList")
end sub

' Another drive plugged in, or this one pulled out: the page shows what is
' there now.
sub onDrive()
    drive = m.global.drive.drive
    if drive <> invalid and httpString(drive.volume) <> m.volume then load()
end sub

sub onList(event as object)
    request = event.getRoSGNode()
    if request.spec.asked <> m.asked then return
    answer = event.getData()
    m.loading.visible = false
    data = httpObject(answer.data)
    drive = invalid
    if data <> invalid then drive = httpObject(data.drive)
    m.volume = ""
    m.title.text = driveDefaultName()
    if drive <> invalid
        m.volume = httpString(drive.volume)
        if m.volume <> "" then m.title.text = httpString(drive.name)
    end if
    if not answer.ok and answer.error <> "unreadable"
        m.place.text = ""
        showLines(invalid, false)
        showMessage("usb", t("drive.none.title"), t("drive.none.body"), false)
        return
    end if
    m.place.text = httpString(data.line)
    up = httpTrue(data.up)
    if not answer.ok
        showLines(invalid, up)
        showMessage("circle-alert", t("drive.unreadable.title"), t("drive.unreadable.body"), up)
        return
    end if
    showLines(request.content, up)
    if httpNumber(data.count, 0) = 0
        showMessage("folder-open", t("drive.empty.title"), t("drive.empty.body"), up)
    else
        m.message.visible = false
    end if
end sub

' The lines (content: the task's; invalid: ".." alone where up), the focus
' kept on the line it was on, else on the first folder or video, else on
' "..".
sub showLines(content as dynamic, up as boolean)
    focusedPath = invalid
    if m.lines.content <> invalid
        line = m.lines.content.getChild(m.lines.itemFocused)
        if line <> invalid then focusedPath = line.url
    end if
    if content = invalid
        content = CreateObject("roSGNode", "ContentNode")
        if up
            node = content.createChild("ContentNode")
            node.setFields({ url: "", title: "..", description: t("drive.up"), shortDescriptionLine1: "up", shortDescriptionLine2: "", hdPosterUrl: "pkg:/images/icons/corner-left-up.png" })
        end if
    end if
    at = 0
    if up and content.getChildCount() > 1 then at = 1
    if focusedPath <> invalid
        for i = 0 to content.getChildCount() - 1
            if content.getChild(i).url = focusedPath then at = i
        end for
    end if
    m.lines.content = content
    m.lines.visible = content.getChildCount() > 0
    if content.getChildCount() > 0 then m.lines.jumpToItem = at
    focus()
end sub

' The lines take the focus; with none, the page itself, so Back still
' leaves it.
sub focus()
    if not m.top.active then return
    if m.lines.visible and m.lines.content <> invalid and m.lines.content.getChildCount() > 0
        m.lines.setFocus(true)
    else
        m.top.setFocus(true)
    end if
end sub

' Why there is nothing to list, under its icon, in the middle of the page:
' under the ".." line when there is one.
sub showMessage(icon as string, title as string, text as string, up as boolean)
    m.messageIcon.uri = "pkg:/images/icons/" + icon + ".png"
    m.messageTitle.text = title
    m.messageText.text = text
    if up
        m.message.translation = [96, 196 + 96 + 48]
    else
        m.message.translation = [96, 300]
    end if
    m.message.visible = true
end sub

' ".." goes back up; a folder opens over this page; a video plays from
' where it stopped.
sub onLine(event as object)
    line = m.lines.content.getChild(event.getData())
    if line = invalid then return
    kind = line.shortDescriptionLine1
    if kind = "up"
        uiNavigate({ back: true })
    else if kind = "folder"
        uiNavigate({ to: "drive", params: { folder: line.url } })
    else
        m.played = line.url
        kept = driveKept(CreateObject("roRegistrySection", "driveResume"), line.url)
        uiNavigate({ to: "player", params: { local: { path: line.url, title: line.title }, startMs: kept.positionMs } })
    end if
end sub

' Back from the player: the line of what played says where it resumes now
' (the focus stayed on it).
sub playedLine()
    path = m.played
    m.played = ""
    if m.lines.content = invalid then return
    line = m.lines.content.getChild(m.lines.itemFocused)
    if line = invalid or line.url <> path then return
    shown = driveLine({ path: path, name: driveFileName(path), folder: false, size: 0 }, driveKept(CreateObject("roRegistrySection", "driveResume"), path))
    line.description = shown.subtitle
    line.progress = shown.progress
end sub
