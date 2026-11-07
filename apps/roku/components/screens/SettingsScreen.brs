' Settings in two panes, as on Android TV and the native Samsung app: the
' categories at the left (Profile, Playback, Subtitles and display, Server,
' Diagnostics, About, Sign out), the focused category's rows at the right,
' each with its value on the line under its label. Right or OK goes into
' the rows, at their first, and Left back; OK on Sign out asks first. What
' a change did, or why it didn't, shows as a toast.
'
' Profile has the profile's audio and subtitle defaults (Tracks.brs), as
' on Samsung and LG: Audio language, Subtitles, and Subtitle language when
' subtitles come on by language. OK on one opens its choices in a panel at
' the right; a choice is saved at once (PUT /api/me/track-defaults, all
' three) and shows before the server answers, put back if it refuses. A
' server or sign-in that doesn't let apps change them shows where they are
' changed on the web. Language, after them, is the account's on a server
' that keeps one (PUT /api/me/language), else this Roku's, as the globe
' picks it (I18n.brs's i18nRow); a new one draws the screen again in it,
' the focus on its row. Quality and Playback details are this Roku's own
' (AutoQuality.brs, the registry's playback section), and so is "Always
' start as" (Profiles.brs), offered when the household has more than one
' profile. Switch profile shows for a sign-in that may switch them.

sub init()
    m.title = m.top.findNode("title")
    m.categories = m.top.findNode("categories")
    m.rows = m.top.findNode("rows")
    m.header = m.top.findNode("header")
    m.face = m.top.findNode("face")
    m.initial = m.top.findNode("initial")
    m.picture = m.top.findNode("picture")
    m.name = m.top.findNode("name")
    m.who = m.top.findNode("who")
    m.intro = m.top.findNode("intro")
    m.panel = m.top.findNode("panel")
    m.panelTitle = m.top.findNode("panelTitle")
    m.choices = m.top.findNode("choices")
    m.panelHint = m.top.findNode("panelHint")
    ' The category shown, what each of its rows does (in order), and what
    ' they show, so that the same rows aren't set again.
    m.shown = "profile"
    m.kinds = []
    m.rowsShown = ""
    ' The pane the focus comes back to.
    m.lastFocus = m.categories
    m.dialog = invalid
    ' The profile as GET /api/auth/me last said it, and its defaults as
    ' Settings knows them ({ audio, subtitle, mode }), invalid until it
    ' answered; refused once the server said apps can't change them.
    m.me = invalid
    m.defaults = invalid
    m.refused = false
    m.saving = 0
    ' The latest save of the defaults, by number (saveDefaults).
    m.saveSeq = 0
    ' Each language picked counts: an answer to one before the last
    ' changes nothing the last did.
    m.languagePicks = 0
    ' The household's profiles ([{ id, name }]), for "Always start as".
    m.household = []
    ' The choices the panel shows, and whose they are.
    m.choice = invalid
    m.choiceKind = ""
    m.categories.observeField("itemFocused", "onCategoryFocused")
    m.categories.observeField("itemSelected", "onCategorySelected")
    m.rows.observeField("itemSelected", "onRow")
    m.choices.observeField("itemSelected", "onChoice")
    m.choices.observeField("itemFocused", "onChoiceFocused")
    m.top.observeField("active", "onActive")
    m.top.observeField("language", "relabel")
    relabel()
end sub

' The screen's words in the language shown, the focus on the category and
' the row it was on (Language's, for a language just chosen).
sub relabel()
    typeSet(m.title, t("settings.title"))
    items = []
    for each category in settingsCategories()
        items.Push({ title: t("settings.categories." + category), value: "", state: "", danger: category = "signOut", rowWidth: 348, rowHeight: 72 })
    end for
    focused = m.categories.itemFocused
    m.categories.content = nodesContent(items)
    if focused > 0 then m.categories.jumpToItem = focused
    if m.me <> invalid then showProfile()
    if m.panel.visible then closeChoices()
    showRows()
end sub

' The categories, top to bottom. Android TV's Audio (passthrough by
' format) has no place here: the Roku's own settings decide what goes to a
' receiver.
function settingsCategories() as object
    return ["profile", "playback", "display", "server", "diagnostics", "about", "signOut"]
end function

sub onActive()
    if not m.top.active then return
    session = m.global.session
    ' What the player changed meanwhile (the quality) shows.
    showRows()
    if m.panel.visible
        m.choices.setFocus(true)
    else
        m.lastFocus.setFocus(true)
    end if
    uiGet("/api/auth/me", "onMe")
    if authAllows(session.scope, "profiles") then uiGet("/api/oauth/profiles", "onHousehold")
end sub

sub onHousehold(event as object)
    answer = event.getData()
    if not answer.ok or GetInterface(answer.data, "ifArray") = invalid then return
    m.household = profHousehold(answer.data)
    profKeepHousehold(CreateObject("roRegistrySection", "household"), m.global.session.server, answer.data)
    showRows()
end sub

function startAsSection() as object
    return CreateObject("roRegistrySection", "startAs")
end function

' Settings stay usable when the server doesn't answer: switching server
' and signing out need no answer.
sub onMe(event as object)
    answer = event.getData()
    if not answer.ok or httpObject(answer.data) = invalid then return
    m.me = answer.data
    showProfile()
    ' A choice still on its way stays as chosen.
    if m.saving = 0 then useProfile(answer.data)
end sub

' Who is signed in, above Profile's rows: the picture (or the initial on
' the profile's colour), the name, and "Managed profile · host:port".
sub showProfile()
    me = m.me
    server = m.global.session.server
    name = httpSentence(me.name)
    typeSet(m.name, name)
    typeSet(m.initial, UCase(Left(name, 1)))
    m.face.color = profColor(me.avatar_color)
    ' The scene's agent carries the token, which a profile's picture needs.
    m.picture.setHttpAgent(m.top.getScene().getHttpAgent())
    m.picture.uri = urlImage(server, me.avatar_url, 96)
    who = authShownAddress(server)
    if httpTrue(me.managed) then who = t("settings.managed") + " · " + who
    typeSet(m.who, who)
    showRows()
end sub

' The profile as the server has it (GET /api/auth/me, or the answer to a
' change).
sub useProfile(me as dynamic)
    if httpObject(me) = invalid then return
    m.defaults = tracksDefaults(me)
    showRows()
end sub

' The rows of the category shown: { kind, label, value, on (a switch's),
' danger }. Profile's are in Android TV's order.
function rowsOf(category as string) as object
    session = m.global.session
    rows = []
    if category = "profile"
        if authAllows(session.scope, "profiles") then rows.Push({ kind: "switchProfile", label: t("settings.rows.switchProfile"), value: "" })
        rows.Append(tracksRows(m.defaults, m.refused))
        rows.Push({ kind: "language", label: t("settings.language.title"), value: languageShown() })
        rows.Push({ kind: "history", label: t("settings.rows.history"), value: "" })
        rows.Push({ kind: "sortLibraries", label: t("settings.rows.sortLibraries"), value: t("settings.rows.sortLibrariesValue") })
        if m.household.Count() > 1
            rows.Push({ kind: "startAs", label: t("settings.alwaysStartAs"), value: profStartAsLabel(m.household, profStartAs(startAsSection(), session.server)) })
        end if
        rows.Push({ kind: "web", label: t("settings.web"), value: t("settings.webHint") })
    else if category = "playback"
        ' This Roku's, kept on it: no server needed.
        rows.Push({ kind: "quality", label: t("settings.quality"), value: aqLabel(aqChoice(playbackSection())) })
        rows.Push({ kind: "autoplay", label: t("settings.rows.autoplay"), value: t("settings.rows.autoplayValue"), on: playerAutoplay(playbackSection()) })
        rows.Push({ kind: "details", label: t("settings.rows.details"), value: t("settings.rows.detailsValue"), on: showsDetails() })
    else if category = "display"
        ' The app never sets captionStyle, which would change them for the
        ' whole Roku: subtitles look as its own settings say.
        rows.Push({ kind: "captions", label: t("settings.rows.captions"), value: t("settings.rows.captionsValue") })
    else if category = "server"
        rows.Push({ kind: "server", label: authShownAddress(session.server), value: t("settings.rows.serverValue") })
        rows.Push({ kind: "switchServer", label: t("settings.rows.switchServer"), value: "" })
    else if category = "diagnostics"
        rows.Push({ kind: "diagnostics", label: t("settings.rows.diagnostics"), value: t("settings.rows.diagnosticsValue") })
    else if category = "about"
        rows.Push({ kind: "version", label: t("settings.rows.version"), value: session.appVersion })
        rows.Push({ kind: "licences", label: t("settings.licences"), value: t("settings.rows.licencesValue") })
    else
        rows.Push({ kind: "signOut", label: t("settings.rows.signOut"), value: "", danger: true })
    end if
    return rows
end function

' The rows of the category shown, under what goes above them; the focus
' stays on the row it was on.
sub showRows()
    top = 170
    m.header.visible = false
    m.intro.visible = false
    if m.shown = "profile"
        ' Room for who is signed in, shown once the server said.
        m.header.visible = m.me <> invalid
        top = 300
    else if m.shown = "signOut"
        typeSet(m.intro, t("settings.signOut.intro", { address: authShownAddress(m.global.session.server) }))
        m.intro.visible = true
        top = 270
    else if m.shown = "about"
        ' Discover's titles and pictures come from TMDB, whose terms ask
        ' for this.
        typeSet(m.intro, t("settings.tmdb"))
        m.intro.visible = true
        top = 270
    end if
    m.rows.translation = [460, top]
    m.rows.numRows = Int((1020 - top) / 108)
    items = []
    kinds = []
    for each row in rowsOf(m.shown)
        state = ""
        if row.DoesExist("on")
            state = t("common.states.off")
            if row.on then state = t("common.states.on")
        end if
        items.Push({ title: row.label, value: row.value, state: state, danger: httpTrue(row.danger), rowWidth: 1000, rowHeight: 100 })
        kinds.Push(row.kind)
    end for
    shows = m.shown + FormatJson(items)
    if shows = m.rowsShown then return
    focused = ""
    at = m.rows.itemFocused
    if m.kinds.Count() > 0 and at >= 0 and at < m.kinds.Count() then focused = m.kinds[at]
    hadFocus = m.rows.isInFocusChain()
    m.rowsShown = shows
    m.kinds = kinds
    m.rows.content = nodesContent(items)
    for i = 0 to kinds.Count() - 1
        if kinds[i] = focused then m.rows.jumpToItem = i
    end for
    if hadFocus then m.rows.setFocus(true)
end sub

' The focus on a category shows its rows. Only the focus moving in the
' categories changes the one shown: their words drawn again in a new
' language leave it.
sub onCategoryFocused(event as object)
    at = event.getData()
    categories = settingsCategories()
    if at < 0 or at >= categories.Count() or not m.categories.isInFocusChain() then return
    m.shown = categories[at]
    showRows()
end sub

sub onCategorySelected(event as object)
    if settingsCategories()[event.getData()] = "signOut"
        askSignOut()
    else
        focusRows()
    end if
end sub

' Into the rows, at their first.
sub focusRows()
    if m.kinds.Count() = 0 then return
    m.rows.jumpToItem = 0
    m.lastFocus = m.rows
    m.rows.setFocus(true)
end sub

sub focusCategories()
    m.lastFocus = m.categories
    m.categories.setFocus(true)
end sub

sub onRow(event as object)
    at = event.getData()
    if at < 0 or at >= m.kinds.Count() then return
    kind = m.kinds[at]
    m.lastFocus = m.rows
    if kind = "quality"
        openQuality()
    else if kind = "autoplay"
        playerSetAutoplay(playbackSection(), not playerAutoplay(playbackSection()))
        showRows()
    else if kind = "details"
        setShowsDetails(not showsDetails())
        showRows()
    else if kind = "audio" or kind = "mode" or kind = "subtitle"
        openChoices(kind)
    else if kind = "language"
        openLanguages()
    else if kind = "startAs"
        showChoices("startAs", profStartAsChoices(m.household, profStartAs(startAsSection(), m.global.session.server)))
    else if kind = "switchProfile"
        uiNavigate({ to: "profiles" })
    else if kind = "history"
        uiNavigate({ to: "history" })
    else if kind = "sortLibraries"
        uiNavigate({ to: "sortLibraries" })
    else if kind = "web"
        showWeb()
    else if kind = "switchServer"
        uiNavigate({ to: "servers", reset: true })
    else if kind = "diagnostics"
        uiNavigate({ to: "diagnostics" })
    else if kind = "licences"
        uiNavigate({ to: "licences" })
    else if kind = "signOut"
        askSignOut()
    end if
    ' The rest (the server, the version, the subtitles' style) only say.
end sub

' Sign out asks first, Cancel first: "You'll be signed out of host:port."
sub askSignOut()
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = t("settings.signOut.title")
    dialog.message = [t("settings.signOut.body", { address: authShownAddress(m.global.session.server) })]
    dialog.buttons = [t("common.actions.cancel"), t("settings.signOut.confirm")]
    dialog.observeField("buttonSelected", "onSignOutButton")
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

' Signed out, the scene shows sign-in (MainScene.onSignedIn).
sub onSignOutButton(event as object)
    closeDialog()
    if event.getData() = 1 then uiRequest({ op: "signOut" }, "")
end sub

' Where the account is managed: the address to type on a phone or
' computer. The other TVs draw its QR code too; the server draws QR codes
' of sign-in codes only, so Roku says the address alone.
sub showWeb()
    dialog = CreateObject("roSGNode", "StandardMessageDialog")
    dialog.title = t("settings.web")
    dialog.message = [t("settings.webBody"), webAddress()]
    dialog.buttons = [t("common.actions.close")]
    dialog.observeField("buttonSelected", "closeDialog")
    m.dialog = dialog
    m.top.getScene().dialog = dialog
end sub

sub closeDialog()
    if m.dialog <> invalid then m.dialog.close = true
    m.dialog = invalid
end sub

' Where the defaults are changed instead: the web app's account page.
function webAddress() as string
    return authShownAddress(m.global.session.server + "/account")
end function

' --- The choices ---

' The choices of one row, the current one checked and focused.
sub openChoices(kind as string)
    if m.defaults = invalid
        ' The profile hasn't come: asked again.
        uiToast(t("settings.loadingProfile"))
        uiGet("/api/auth/me", "onMe")
        return
    end if
    if m.refused
        showWeb()
        return
    end if
    showChoices(kind, tracksChoices(kind, m.defaults))
end sub

' The Quality menu (docs/QUALITY.md), kept on this Roku: Automatic unless
' another was chosen.
sub openQuality()
    current = aqChoice(playbackSection())
    choice = { title: t("settings.quality"), labels: [], values: [], hints: [], current: 0 }
    for each quality in aqMenu()
        if quality.id = current then choice.current = choice.values.Count()
        choice.labels.Push(quality.label)
        choice.values.Push(quality.id)
        choice.hints.Push(aqHint(quality.id))
    end for
    showChoices("quality", choice)
end sub

' Where this Roku's playback settings are kept (the player reads them).
function playbackSection() as object
    return CreateObject("roRegistrySection", "playback")
end function

' Playback details, this Roku's (its playback section's showDetails,
' "true" or "false", off unless turned on): the player shows how a video
' plays over it while it is on.
function showsDetails() as boolean
    return playbackSection().Read("showDetails") = "true"
end function

sub setShowsDetails(shown as boolean)
    section = playbackSection()
    value = "false"
    if shown then value = "true"
    section.Write("showDetails", value)
    section.Flush()
end sub

sub showChoices(kind as string, choice as object)
    m.choiceKind = kind
    m.choice = choice
    items = []
    for each label in m.choice.labels
        items.Push({ title: label })
    end for
    typeSet(m.panelTitle, m.choice.title)
    m.choices.content = nodesContent(items)
    m.choices.checkedItem = m.choice.current
    at = m.choice.current
    if at < 0 then at = 0
    m.choices.jumpToItem = at
    choiceHint(at)
    m.panel.visible = true
    m.choices.setFocus(true)
end sub

sub closeChoices()
    m.panel.visible = false
    m.rows.setFocus(true)
end sub

sub onChoiceFocused(event as object)
    choiceHint(event.getData())
end sub

' A choice's hint, under the choices.
sub choiceHint(at as integer)
    text = ""
    if m.choice <> invalid and at >= 0 and at < m.choice.hints.Count() then text = m.choice.hints[at]
    typeSet(m.panelHint, text)
end sub

sub onChoice(event as object)
    if m.choice = invalid then return
    at = event.getData()
    if at < 0 or at >= m.choice.values.Count() then return
    closeChoices()
    if m.choiceKind = "quality"
        value = m.choice.values[at]
        if value = "auto" and aqChoice(playbackSection()) <> "auto"
            ' Picked again, Automatic starts afresh at the original, here
            ' and on the server.
            aqRemember(playbackSection(), m.global.session.server, 0, CreateObject("roDateTime").AsSeconds())
            uiRequest({ op: "tellNetwork", ceilingKbps: 0 }, "")
        end if
        aqChoose(playbackSection(), value)
        showRows()
        return
    end if
    if m.choiceKind = "startAs"
        profSetStartAs(startAsSection(), m.global.session.server, m.choice.values[at])
        showRows()
        return
    end if
    if m.choiceKind = "language"
        chooseLanguage(m.choice.values[at])
        return
    end if
    saveDefaults(tracksWith(m.defaults, m.choiceKind, m.choice.values[at]))
end sub

' Shows defaults at once and saves them; the server's answer is the
' profile, a refusal puts back what was. Each save sends every default, so
' only the latest one's refusal puts its before back: an earlier one's
' would undo a later save.
sub saveDefaults(defaults as object)
    if tracksSame(defaults, m.defaults) then return
    before = m.defaults
    m.defaults = defaults
    showRows()
    m.saving = m.saving + 1
    m.saveSeq = m.saveSeq + 1
    uiRequest({ method: "PUT", path: "/api/me/track-defaults", jsonText: FormatJson(tracksDefaultsBody(defaults)), before: before, seq: m.saveSeq }, "onSaved")
end sub

sub onSaved(event as object)
    m.saving = m.saving - 1
    answer = event.getData()
    if answer.ok
        uiToast(t("common.states.saved"))
        if m.saving = 0 then useProfile(answer.data)
        return
    end if
    spec = event.getRoSGNode().spec
    if spec.seq <> m.saveSeq then return
    m.defaults = spec.before
    if answer.error = "signed_out"
        showRows()
        return
    end if
    if tracksRefused(answer.status)
        m.refused = true
        uiToast(t("settings.readOnly"), true)
    else
        uiToast(answer.message, true)
    end if
    showRows()
end sub

' --- The language ---

' What the row says: Automatic, naming what that is, or the language
' chosen, in itself.
function languageShown() as string
    row = i18nRow(uiLanguageFacts())
    if row.current <> "" then return i18nName(row.current)
    return t("settings.language.automatic", { language: i18nName(row.automatic) })
end function

sub openLanguages()
    row = i18nRow(uiLanguageFacts())
    choice = i18nChoices(row.automatic, row.current)
    choice.title = t("settings.language.title")
    choice.hints = []
    showChoices("language", choice)
end sub

' A language chosen ("" for Automatic): the account's, on a server that
' keeps one, shown once the server has it (the API task hears it in the
' answer); else this Roku's, shown at once.
sub chooseLanguage(language as string)
    m.languagePicks = m.languagePicks + 1
    if i18nRow(uiLanguageFacts()).onDevice
        m.top.getScene().languageChoice = language
        showRows()
        return
    end if
    body = "{""language"":null}"
    if language <> "" then body = FormatJson({ language: language })
    spec = { method: "PUT", path: "/api/me/language", jsonText: body, profile: true, language: language, pick: m.languagePicks }
    uiRequest(spec, "onLanguageSaved")
end sub

' A Pley from before languages has no such route (404, or 405): the
' choice is this Roku's while the server is the one in use, unless one
' was picked since. Any other refusal says why.
sub onLanguageSaved(event as object)
    answer = event.getData()
    if answer.ok
        showRows()
        return
    end if
    if answer.status = 404 or answer.status = 405
        spec = event.getRoSGNode().spec
        scene = m.top.getScene()
        scene.languageRefused = m.global.session.server
        if spec.pick = m.languagePicks then scene.languageChoice = spec.language
        showRows()
        return
    end if
    if not uiFailed(answer) then uiToast(answer.message, true)
end sub

' Back closes the choices; Left from them does too. Right and Left go
' between the categories and their rows.
function onKeyEvent(key as string, press as boolean) as boolean
    if not press then return false
    if m.panel.visible
        if key = "back" or key = "left"
            closeChoices()
            return true
        end if
        return false
    end if
    if key = "right" and m.categories.isInFocusChain()
        focusRows()
        return true
    end if
    if key = "left" and m.rows.isInFocusChain()
        focusCategories()
        return true
    end if
    return false
end function
