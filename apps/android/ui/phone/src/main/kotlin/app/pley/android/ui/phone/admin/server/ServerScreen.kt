package app.pley.android.ui.phone.admin.server

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.Language
import app.pley.android.common.errorSentence
import app.pley.android.common.resolve
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.text.formatWords
import app.pley.android.designsystem.text.shownLocale
import app.pley.android.model.admin.LibraryWorkMode
import app.pley.android.model.admin.LibraryWorkWait
import app.pley.android.model.admin.LibraryWorkWeek
import app.pley.android.model.admin.DatabaseBackups
import app.pley.android.model.admin.ServerSettings
import app.pley.android.model.admin.TestedService
import app.pley.android.model.admin.TvMetadataSource
import app.pley.android.model.format.formatBytes
import app.pley.android.model.format.formatRelative
import app.pley.android.model.format.formatTimestamp
import app.pley.android.screens.admin.server.MAX_SCAN_INTERVAL
import app.pley.android.screens.admin.server.ServerViewModel
import app.pley.android.screens.admin.server.ServiceTestState
import app.pley.android.screens.admin.server.automaticTvSource
import app.pley.android.screens.admin.server.libraryWorkToDo
import app.pley.android.screens.admin.server.maxCpuChoices
import app.pley.android.screens.admin.server.metadataLanguages
import app.pley.android.screens.admin.server.withoutTwoStep
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.Choice
import app.pley.android.ui.phone.account.ChoiceSheet
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.Entry
import app.pley.android.ui.phone.admin.TextEntryDialog
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.admin.numberProblem
import app.pley.android.ui.phone.kit.ConfirmDialog
import app.pley.android.ui.phone.shell.LocalPhoneShell

/** What a value of the page is typed into, or chosen in; one at a time. */
private enum class Open {
    Name, Language, TmdbKey, TvSource, TvdbKey, TvdbPin, MetadataLanguage, OpenSubtitlesKey, OpenSubtitlesUsername,
    OpenSubtitlesPassword, LibraryWork, WorkFrom, WorkTo, MaxCpu, ScanInterval, RequireTwoStep, ThirdPartyOff,
}

/**
 * Settings → Server on a phone (the web's ServerSettings and its backups):
 * the server's name and language, metadata sources and their keys,
 * OpenSubtitles, library scanning and when library work runs (with the
 * week as the server learned it), sign-in, apps by other publishers, and
 * the database's backups. Each value is sent as it is chosen; a key is
 * tested as saved. [id] names nothing: the page is one.
 */
@Composable
internal fun ServerScreen(@Suppress("UNUSED_PARAMETER") id: String?) {
    val model: ServerViewModel = viewModel(factory = ServerViewModel.factory)
    val load by model.settings.state.collectAsStateWithLifecycle()
    val system by model.system.state.collectAsStateWithLifecycle()
    val me by model.me.state.collectAsStateWithLifecycle()
    val accounts by model.accounts.state.collectAsStateWithLifecycle()
    val week by model.week.state.collectAsStateWithLifecycle()
    val backups by model.backups.state.collectAsStateWithLifecycle()
    val tests by model.tests.collectAsStateWithLifecycle()
    val busy by model.busy.collectAsStateWithLifecycle()
    var open by rememberSaveable { mutableStateOf<Open?>(null) }
    val shell = LocalPhoneShell.current
    val resources = LocalResources.current
    val words = formatWords()
    AdminEvents(model)
    LaunchedEffect(model) {
        model.backedUp.collect { shell.toasts.show(resources.getString(R.string.admin_server_backed_up, formatBytes(it.bytes, words))) }
    }

    // The week is read again every minute while its chart shows, as the web polls it.
    val settings = load.valueOrNull
    val charted = settings != null && settings.libraryWork == LibraryWorkMode.Auto && settings.libraryWorkToDo
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    if (charted) {
        LaunchedEffect(model, lifecycle) { lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) { model.followWeek() } }
    }

    AdminPage(
        title = stringResource(R.string.admin_server_title),
        load = load,
        onRetry = model.settings::reload,
        tag = SERVER_PAGE,
    ) { s ->
        general(s) { open = it }
        metadata(s, tests, model, onOpen = { open = it })
        subtitles(s, tests, model) { open = it }
        scanning(
            s,
            noChromaprint = system.valueOrNull?.transcoder?.let { it.known && !it.chromaprint } == true,
            week = week.valueOrNull?.takeIf { charted },
            model = model,
            onOpen = { open = it },
        )
        signIn(
            s,
            meUsesIt = me.valueOrNull?.twoStep,
            noKey = system.valueOrNull?.secretKeyOk == false,
            model = model,
            onOpen = { open = it },
        )
        apps(s, model) { open = it }
        backups(backups.valueOrNull, busy > 0, model)
    }

    val s = settings ?: return
    val close = { open = null }
    when (open) {
        Open.Name -> TextEntryDialog(
            title = stringResource(R.string.admin_server_name),
            initial = s.serverName,
            hint = stringResource(R.string.admin_server_name_hint),
            onSave = { model.rename(it); close() },
            onDismiss = close,
        )
        Open.Language -> ChoiceSheet(
            title = stringResource(R.string.admin_server_language),
            choices = Language.entries.map { Choice(it.code, stringResource(it.autonym)) },
            selected = s.language.ifEmpty { Language.English.code },
            onPick = model::setLanguage,
            onDismiss = close,
        )
        Open.TmdbKey -> TextEntryDialog(
            title = stringResource(R.string.admin_server_tmdb_key),
            initial = s.tmdbApiKey,
            hint = stringResource(R.string.admin_server_tmdb_key_hint),
            entry = Entry.Secret,
            onSave = { model.setTmdbKey(it.trim()); close() },
            onDismiss = close,
        )
        Open.TvSource -> ChoiceSheet(
            title = stringResource(R.string.admin_server_tv_shows),
            choices = tvSources(s).map { Choice(it, tvSourceLabel(it, s)) },
            selected = s.tvMetadata,
            onPick = model::setTvSource,
            onDismiss = close,
        )
        Open.TvdbKey -> TextEntryDialog(
            title = stringResource(R.string.admin_server_tvdb_key),
            initial = s.tvdbApiKey,
            hint = stringResource(R.string.admin_server_tvdb_key_hint),
            entry = Entry.Secret,
            onSave = { model.setTvdbKey(it.trim()); close() },
            onDismiss = close,
        )
        Open.TvdbPin -> TextEntryDialog(
            title = stringResource(R.string.admin_server_tvdb_pin),
            initial = s.tvdbPin,
            hint = stringResource(R.string.admin_server_tvdb_pin_hint),
            entry = Entry.Secret,
            onSave = { model.setTvdbPin(it.trim()); close() },
            onDismiss = close,
        )
        Open.MetadataLanguage -> ChoiceSheet(
            title = stringResource(R.string.admin_server_metadata_language),
            choices = metadataLanguages(s.metadataLanguage).map { Choice(it, it) },
            selected = s.metadataLanguage,
            onPick = model::setMetadataLanguage,
            onDismiss = close,
        )
        Open.OpenSubtitlesKey -> TextEntryDialog(
            title = stringResource(R.string.admin_server_os_key),
            initial = s.opensubtitlesApiKey,
            hint = stringResource(R.string.admin_server_os_key_hint),
            entry = Entry.Secret,
            onSave = { model.setOpenSubtitlesKey(it.trim()); close() },
            onDismiss = close,
        )
        Open.OpenSubtitlesUsername -> TextEntryDialog(
            title = stringResource(R.string.admin_server_os_username),
            initial = s.opensubtitlesUsername,
            hint = stringResource(R.string.admin_server_os_account_hint),
            onSave = { model.setOpenSubtitlesUsername(it); close() },
            onDismiss = close,
        )
        Open.OpenSubtitlesPassword -> TextEntryDialog(
            title = stringResource(R.string.admin_server_os_password),
            initial = "",
            hint = if (s.opensubtitlesPasswordSet) stringResource(R.string.admin_server_os_password_keep) else null,
            entry = Entry.Secret,
            onSave = { model.setOpenSubtitlesPassword(it); close() },
            onDismiss = close,
        )
        Open.LibraryWork -> ChoiceSheet(
            title = stringResource(R.string.admin_server_work),
            choices = listOf(
                Choice(LibraryWorkMode.Auto, stringResource(R.string.admin_server_work_auto), stringResource(R.string.admin_server_work_auto_note)),
                Choice(LibraryWorkMode.Hours, stringResource(R.string.admin_server_work_hours)),
                Choice(LibraryWorkMode.Idle, stringResource(R.string.admin_server_work_idle)),
            ),
            selected = s.libraryWork ?: LibraryWorkMode.Auto,
            onPick = model::setLibraryWork,
            onDismiss = close,
        )
        Open.WorkFrom, Open.WorkTo -> {
            val locale = shownLocale()
            val from = open == Open.WorkFrom
            ChoiceSheet(
                title = stringResource(if (from) R.string.admin_server_work_from else R.string.admin_server_work_to),
                choices = (0 until 24).map { Choice(it, hourOfDay(it, locale)) },
                selected = if (from) s.libraryWorkFrom else s.libraryWorkTo,
                onPick = { if (from) model.setLibraryWorkFrom(it) else model.setLibraryWorkTo(it) },
                onDismiss = close,
            )
        }
        Open.MaxCpu -> ChoiceSheet(
            title = stringResource(R.string.admin_server_wait_others),
            choices = maxCpuChoices(s.libraryWorkMaxCpu).map { Choice(it, maxCpuLabel(it)) },
            selected = s.libraryWorkMaxCpu,
            onPick = model::setLibraryWorkMaxCpu,
            onDismiss = close,
        )
        Open.ScanInterval -> TextEntryDialog(
            title = stringResource(R.string.admin_server_interval),
            initial = s.scanIntervalMinutes.toString(),
            hint = stringResource(R.string.admin_server_interval_hint),
            entry = Entry.Number,
            check = numberProblem(0L..MAX_SCAN_INTERVAL),
            onSave = { model.setScanInterval(it.toInt()); close() },
            onDismiss = close,
        )
        Open.RequireTwoStep -> {
            val without = accounts.valueOrNull?.let(::withoutTwoStep)
            ConfirmDialog(
                title = stringResource(R.string.admin_server_require_title),
                body = when (without) {
                    null -> stringResource(R.string.admin_server_require_two_step_hint)
                    0 -> stringResource(R.string.admin_server_everyone_has_it)
                    else -> pluralStringResource(R.plurals.admin_server_signs_out, without, without)
                },
                confirmLabel = stringResource(R.string.admin_server_require_confirm),
                destructive = without != 0,
                onConfirm = { model.setRequireTwoStep(true); close() },
                onDismiss = close,
            )
        }
        Open.ThirdPartyOff -> ConfirmDialog(
            title = stringResource(R.string.admin_server_third_party_off_title),
            body = stringResource(R.string.admin_server_third_party_off_body),
            confirmLabel = stringResource(R.string.admin_server_third_party_off),
            destructive = true,
            onConfirm = { model.setThirdPartyApps(false); close() },
            onDismiss = close,
        )
        null -> Unit
    }
}

/** The name, and the language of the server's own words. */
private fun LazyListScope.general(s: ServerSettings, onOpen: (Open) -> Unit) {
    item(key = "general") {
        SettingsGroup(stringResource(R.string.admin_server_general), Modifier.adminWidth) {
            ValueRow(stringResource(R.string.admin_server_name), s.serverName, onClick = { onOpen(Open.Name) })
            RowDivider(inset = false)
            val language = Language.of(s.language) ?: Language.English
            ValueRow(
                stringResource(R.string.admin_server_language),
                stringResource(language.autonym),
                hint = stringResource(R.string.admin_server_language_hint),
                onClick = { onOpen(Open.Language) },
            )
        }
    }
}

/** Metadata sources, their keys and tests. */
private fun LazyListScope.metadata(
    s: ServerSettings,
    tests: Map<TestedService, ServiceTestState>,
    model: ServerViewModel,
    onOpen: (Open) -> Unit,
) {
    item(key = "metadata") {
        SettingsGroup(
            stringResource(R.string.admin_server_metadata),
            Modifier.adminWidth,
            footer = stringResource(R.string.admin_server_attribution),
        ) {
            GroupLine(stringResource(R.string.admin_server_metadata_description))
            ValueRow(
                stringResource(R.string.admin_server_tmdb_key),
                savedOrNot(s.tmdbApiKey.isNotBlank()),
                hint = stringResource(R.string.admin_server_tmdb_key_hint),
                onClick = { onOpen(Open.TmdbKey) },
            )
            TestRow(stringResource(R.string.admin_server_test_tmdb), tests[TestedService.Tmdb]) { model.test(TestedService.Tmdb) }
            RowDivider(inset = false)
            ValueRow(
                stringResource(R.string.admin_server_tv_shows),
                tvSourceLabel(s.tvMetadata, s),
                hint = stringResource(R.string.admin_server_tv_shows_hint),
                onClick = { onOpen(Open.TvSource) },
            )
            TestRow(stringResource(R.string.admin_server_test_tvmaze), tests[TestedService.Tvmaze]) { model.test(TestedService.Tvmaze) }
            RowDivider(inset = false)
            ValueRow(
                stringResource(R.string.admin_server_tvdb_key),
                savedOrNot(s.tvdbApiKey.isNotBlank()),
                hint = stringResource(R.string.admin_server_tvdb_key_hint),
                onClick = { onOpen(Open.TvdbKey) },
            )
            ValueRow(
                stringResource(R.string.admin_server_tvdb_pin),
                savedOrNot(s.tvdbPin.isNotBlank()),
                hint = stringResource(R.string.admin_server_tvdb_pin_hint),
                onClick = { onOpen(Open.TvdbPin) },
            )
            TestRow(stringResource(R.string.admin_server_test_tvdb), tests[TestedService.Tvdb]) { model.test(TestedService.Tvdb) }
            RowDivider(inset = false)
            ToggleRow(
                stringResource(R.string.admin_server_music_toggle),
                checked = s.musicMetadata,
                onChange = model::setMusicMetadata,
                hint = stringResource(R.string.admin_server_music_hint),
            )
            TestRow(stringResource(R.string.admin_server_test_musicbrainz), tests[TestedService.Musicbrainz]) { model.test(TestedService.Musicbrainz) }
            if (s.readNfo != null) {
                RowDivider(inset = false)
                ToggleRow(
                    stringResource(R.string.admin_server_nfo_toggle),
                    checked = s.readNfo == true,
                    onChange = model::setReadNfo,
                    hint = stringResource(R.string.admin_server_nfo_hint),
                )
            }
            RowDivider(inset = false)
            ValueRow(
                stringResource(R.string.admin_server_metadata_language),
                s.metadataLanguage,
                hint = stringResource(R.string.admin_server_metadata_language_hint),
                onClick = { onOpen(Open.MetadataLanguage) },
            )
        }
    }
}

/** OpenSubtitles: its key, an account for its larger quota, and a test. */
private fun LazyListScope.subtitles(
    s: ServerSettings,
    tests: Map<TestedService, ServiceTestState>,
    model: ServerViewModel,
    onOpen: (Open) -> Unit,
) {
    item(key = "subtitles") {
        SettingsGroup(
            stringResource(R.string.admin_server_subtitles),
            Modifier.adminWidth,
            footer = stringResource(R.string.admin_server_subtitles_description),
        ) {
            ValueRow(
                stringResource(R.string.admin_server_os_key),
                savedOrNot(s.opensubtitlesApiKey.isNotBlank()),
                hint = stringResource(R.string.admin_server_os_key_hint),
                onClick = { onOpen(Open.OpenSubtitlesKey) },
            )
            RowDivider(inset = false)
            ValueRow(
                stringResource(R.string.admin_server_os_account),
                s.opensubtitlesUsername.ifBlank { stringResource(R.string.admin_server_not_set) },
                hint = stringResource(R.string.admin_server_os_account_hint),
                onClick = { onOpen(Open.OpenSubtitlesUsername) },
            )
            ValueRow(
                stringResource(R.string.admin_server_os_password),
                savedOrNot(s.opensubtitlesPasswordSet),
                enabled = s.opensubtitlesUsername.isNotBlank(),
                onClick = { onOpen(Open.OpenSubtitlesPassword) },
            )
            TestRow(stringResource(R.string.admin_server_test_os), tests[TestedService.Opensubtitles]) { model.test(TestedService.Opensubtitles) }
        }
    }
}

/** Scanning, and when library work runs, with the week it goes by. */
private fun LazyListScope.scanning(
    s: ServerSettings,
    noChromaprint: Boolean,
    week: LibraryWorkWeek?,
    model: ServerViewModel,
    onOpen: (Open) -> Unit,
) {
    item(key = "scanning") {
        SettingsGroup(stringResource(R.string.admin_server_scanning), Modifier.adminWidth) {
            ToggleRow(
                stringResource(R.string.admin_server_watch),
                checked = s.watchLibraries,
                onChange = model::setWatchLibraries,
                hint = stringResource(R.string.admin_server_watch_hint),
            )
            RowDivider(inset = false)
            ToggleRow(
                stringResource(R.string.admin_server_markers),
                checked = s.detectMarkers,
                onChange = model::setDetectMarkers,
                enabled = !noChromaprint,
                hint = stringResource(R.string.admin_server_markers_hint),
                note = if (noChromaprint) RowNote(stringResource(R.string.admin_server_no_chromaprint)) else null,
            )
            RowDivider(inset = false)
            ToggleRow(
                stringResource(R.string.admin_server_seek_previews),
                checked = s.seekPreviews,
                onChange = model::setSeekPreviews,
                hint = stringResource(R.string.admin_server_seek_previews_hint),
            )
            RowDivider(inset = false)
            ToggleRow(
                stringResource(R.string.admin_server_whole_libraries),
                checked = s.previewsInBackground,
                onChange = model::setPreviewsInBackground,
                enabled = s.seekPreviews,
                hint = stringResource(R.string.admin_server_whole_libraries_hint),
                note = if (s.seekPreviews) null else RowNote(stringResource(R.string.admin_server_needs_seek_previews)),
            )
            val mode = s.libraryWork
            // A server before library work was scheduled has none of it.
            if (mode != null) {
                val toDo = s.libraryWorkToDo
                val locale = shownLocale()
                RowDivider(inset = false)
                ValueRow(
                    stringResource(R.string.admin_server_work),
                    stringResource(
                        when (mode) {
                            LibraryWorkMode.Hours -> R.string.admin_server_work_hours
                            LibraryWorkMode.Idle -> R.string.admin_server_work_idle
                            else -> R.string.admin_server_work_auto
                        },
                    ),
                    enabled = toDo,
                    hint = stringResource(R.string.admin_server_work_hint),
                    note = when {
                        !toDo -> RowNote(stringResource(R.string.admin_server_work_needs))
                        mode == LibraryWorkMode.Auto -> RowNote(stringResource(R.string.admin_server_work_auto_note))
                        else -> null
                    },
                    onClick = { onOpen(Open.LibraryWork) },
                )
                if (toDo && mode == LibraryWorkMode.Hours) {
                    ValueRow(
                        stringResource(R.string.admin_server_work_from),
                        hourOfDay(s.libraryWorkFrom, locale),
                        hint = stringResource(R.string.admin_server_work_server_time),
                        onClick = { onOpen(Open.WorkFrom) },
                    )
                    ValueRow(
                        stringResource(R.string.admin_server_work_to),
                        hourOfDay(s.libraryWorkTo, locale),
                        hint = stringResource(R.string.admin_server_work_server_time),
                        onClick = { onOpen(Open.WorkTo) },
                    )
                }
                if (week != null) {
                    Column(Modifier.fillMaxWidth().padding(16.dp)) {
                        Text(stringResource(R.string.admin_server_when_people_watch), style = MaterialTheme.typography.bodyLarge, color = PleyColors.Fg)
                        val learned = stringResource(if (week.learned) R.string.admin_server_learned else R.string.admin_server_learning)
                        val resumes = week.resumesAt?.takeIf { week.waiting == LibraryWorkWait.Hours }
                        Text(
                            if (resumes == null) learned else learned + " " + stringResource(R.string.admin_server_next_from, formatTimestamp(resumes, locale)),
                            style = MaterialTheme.typography.bodySmall,
                            color = PleyColors.Subtle,
                            modifier = Modifier.padding(top = 2.dp, bottom = 12.dp),
                        )
                        WatchingHoursChart(week)
                    }
                }
                RowDivider(inset = false)
                ValueRow(
                    stringResource(R.string.admin_server_wait_others),
                    maxCpuLabel(s.libraryWorkMaxCpu),
                    enabled = toDo,
                    hint = stringResource(R.string.admin_server_wait_others_hint),
                    onClick = { onOpen(Open.MaxCpu) },
                )
            }
            RowDivider(inset = false)
            ValueRow(
                stringResource(R.string.admin_server_interval),
                pluralStringResource(R.plurals.admin_server_minutes, s.scanIntervalMinutes, s.scanIntervalMinutes),
                hint = stringResource(R.string.admin_server_interval_hint),
                onClick = { onOpen(Open.ScanInterval) },
            )
        }
    }
}

/**
 * Requiring two-step sign-in: only an administrator who uses it can turn it
 * on, and turning it on signs out everyone without it, which is asked first.
 */
private fun LazyListScope.signIn(
    s: ServerSettings,
    meUsesIt: Boolean?,
    noKey: Boolean,
    model: ServerViewModel,
    onOpen: (Open) -> Unit,
) {
    item(key = "sign-in") {
        val checked = s.requireTwoStep == true
        // Off, it turns on only for someone who uses it; on, it can always go off.
        val blocked = !checked && meUsesIt != true
        SettingsGroup(stringResource(R.string.admin_server_sign_in), Modifier.adminWidth) {
            ToggleRow(
                stringResource(R.string.admin_server_require_two_step),
                checked = checked,
                // Off until a server that knows the setting sends it.
                enabled = s.requireTwoStep != null && !blocked,
                onChange = { on -> if (on) onOpen(Open.RequireTwoStep) else model.setRequireTwoStep(false) },
                hint = stringResource(R.string.admin_server_require_two_step_hint),
                note = when {
                    blocked && s.requireTwoStep != null -> RowNote(stringResource(R.string.admin_server_own_first))
                    checked && noKey -> RowNote(stringResource(R.string.admin_server_no_secret_key), notice = true)
                    else -> null
                },
            )
        }
    }
}

/** Apps by other publishers; turning them off signs them out, which is asked first. */
private fun LazyListScope.apps(s: ServerSettings, model: ServerViewModel, onOpen: (Open) -> Unit) {
    item(key = "apps") {
        SettingsGroup(stringResource(R.string.admin_server_apps), Modifier.adminWidth) {
            ToggleRow(
                stringResource(R.string.admin_server_third_party),
                checked = s.thirdPartyApps,
                onChange = { on -> if (on) model.setThirdPartyApps(true) else onOpen(Open.ThirdPartyOff) },
                hint = stringResource(R.string.admin_server_third_party_hint),
            )
        }
    }
}

/** The database's health, its backups, and backing up now. */
private fun LazyListScope.backups(backups: DatabaseBackups?, busy: Boolean, model: ServerViewModel) {
    item(key = "backups") {
        val words = formatWords()
        SettingsGroup(stringResource(R.string.admin_server_backups), Modifier.adminWidth) {
            val problem = backups?.health?.takeIf { it.ok == false }
            if (problem != null) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) {
                    NoteLine(RowNote(stringResource(R.string.admin_server_damaged), notice = true))
                    Text(
                        stringResource(R.string.admin_server_damaged_hint, problem.problem.orEmpty()),
                        style = MaterialTheme.typography.bodySmall,
                        color = PleyColors.Muted,
                        modifier = Modifier.padding(start = 20.dp, top = 2.dp),
                    )
                }
                RowDivider(inset = false)
            }
            when {
                backups == null -> Unit
                backups.backups.isEmpty() -> Text(
                    stringResource(R.string.admin_server_no_backups),
                    style = MaterialTheme.typography.bodyMedium,
                    color = PleyColors.Subtle,
                    modifier = Modifier.padding(16.dp),
                )
                else -> for (b in backups.backups) {
                    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
                        Text(b.name, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Fg)
                        Text(
                            formatBytes(b.bytes, words) + " · " + formatRelative(b.createdAt, words = words),
                            style = MaterialTheme.typography.bodySmall,
                            color = PleyColors.Muted,
                        )
                    }
                }
            }
            RowDivider(inset = false)
            ValueRow(
                stringResource(R.string.admin_server_back_up_now),
                value = null,
                enabled = !busy,
                onClick = model::backUp,
                trailing = if (busy) ({ Spinner() }) else null,
            )
        }
    }
}

/**
 * A test of a service with what is saved: its button, and how it went in
 * the server's words, with how long the answer took.
 */
@Composable
private fun TestRow(label: String, state: ServiceTestState?, onTest: () -> Unit) {
    val resources = LocalResources.current
    val testing = state == ServiceTestState.Testing
    val (line, color) = when (state) {
        is ServiceTestState.Done -> (if (state.result.ok) "${state.result.message} ${state.result.ms} ms" else state.result.message) to
            (if (state.result.ok) PleyColors.Success else PleyColors.DangerText)
        is ServiceTestState.Failed -> errorSentence(state.error).resolve(resources) to PleyColors.DangerText
        ServiceTestState.Testing -> stringResource(R.string.admin_server_testing) to PleyColors.Muted
        null -> null to null
    }
    ValueRow(
        label,
        line,
        valueColor = color,
        enabled = !testing,
        onClick = onTest,
        trailing = if (testing) ({ Spinner() }) else null,
    )
}

@Composable
private fun Spinner() {
    Box(Modifier.size(20.dp)) {
        CircularProgressIndicator(Modifier.size(20.dp), color = PleyColors.Accent, trackColor = PleyColors.Surface4, strokeWidth = 2.dp)
    }
}

/** "Saved" or "Not set": a key is never shown on the page. */
@Composable
private fun savedOrNot(saved: Boolean): String = stringResource(if (saved) R.string.admin_server_saved else R.string.admin_server_not_set)

/** The sources that can be chosen: one without its key can't. */
private fun tvSources(s: ServerSettings): List<TvMetadataSource> = buildList {
    add(TvMetadataSource.Auto)
    if (s.tmdbApiKey.isNotBlank()) add(TvMetadataSource.Tmdb)
    if (s.tvdbApiKey.isNotBlank()) add(TvMetadataSource.Tvdb)
    add(TvMetadataSource.Tvmaze)
}

@Composable
private fun tvSourceLabel(source: TvMetadataSource, s: ServerSettings): String = when (source) {
    TvMetadataSource.Tmdb -> "TMDB"
    TvMetadataSource.Tvdb -> "TheTVDB"
    TvMetadataSource.Tvmaze -> stringResource(R.string.admin_server_tvmaze_no_key)
    else -> stringResource(R.string.admin_server_automatic, tvSourceName(automaticTvSource(s)))
}

private fun tvSourceName(source: TvMetadataSource): String = when (source) {
    TvMetadataSource.Tmdb -> "TMDB"
    TvMetadataSource.Tvdb -> "TheTVDB"
    else -> "TVmaze"
}

@Composable
private fun maxCpuLabel(percent: Int): String =
    if (percent >= 100) stringResource(R.string.admin_server_never) else stringResource(R.string.admin_server_over, percent)

internal const val SERVER_PAGE = "pley.admin.server"
