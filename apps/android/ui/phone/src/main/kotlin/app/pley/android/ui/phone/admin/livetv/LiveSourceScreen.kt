package app.pley.android.ui.phone.admin.livetv

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.errorSentence
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.text.asString
import app.pley.android.designsystem.text.formatWords
import app.pley.android.designsystem.text.shownLocale
import app.pley.android.model.admin.LiveSource
import app.pley.android.model.admin.LiveSourceKind
import app.pley.android.model.format.formatRelative
import app.pley.android.screens.admin.AdminLoad
import app.pley.android.screens.admin.livetv.LiveSourceViewModel
import app.pley.android.screens.admin.livetv.SourceDraft
import app.pley.android.screens.admin.livetv.SourceTesting
import app.pley.android.screens.admin.livetv.defaultSourceName
import app.pley.android.screens.admin.livetv.expiresSoon
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.Choice
import app.pley.android.ui.phone.account.ChoiceSheet
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.account.SettingsRow
import app.pley.android.ui.phone.account.SwitchRow
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.Entry
import app.pley.android.ui.phone.admin.TextEntryDialog
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.admin.numberIn
import app.pley.android.ui.phone.admin.numberProblem
import app.pley.android.ui.phone.kit.ConfirmDialog
import app.pley.android.ui.phone.kit.EmptyState
import app.pley.android.ui.phone.kit.PrimaryButton
import app.pley.android.ui.phone.kit.TonalButton
import app.pley.android.ui.phone.shell.LocalPhoneNavigator
import java.text.NumberFormat

/** What a source's page shows: the source as saved; `null` when adding one, or when it is gone. */
private data class Shown(val source: LiveSource?)

/** The details a [TextEntryDialog] of a source's page takes. */
private enum class Field { Name, Url, Username, Password, Guide, Connections, UserAgent }

/**
 * A source's page ([sourceId]), or adding one (`null`), as the web's
 * source card and SourceModal: its state and counts (read again while it
 * is being read), turning it on or off and reading it again at once,
 * removing it (asked first); and its details, each typed in its dialog,
 * then tested with the server or saved together with one button.
 */
@Composable
internal fun LiveSourceScreen(sourceId: Long?) {
    val model: LiveSourceViewModel = viewModel(key = "livetv.source.$sourceId", factory = LiveSourceViewModel.factory(sourceId))
    val load by model.source.state.collectAsStateWithLifecycle()
    val draft by model.draft.collectAsStateWithLifecycle()
    val testing by model.testing.collectAsStateWithLifecycle()
    val busy by model.busy.collectAsStateWithLifecycle()
    val navigator = LocalPhoneNavigator.current
    var editing by rememberSaveable { mutableStateOf<Field?>(null) }
    var choosingKind by rememberSaveable { mutableStateOf(false) }
    var removing by rememberSaveable { mutableStateOf(false) }
    AdminEvents(model, liveTvSaid())
    LaunchedEffect(model) { model.finished.collect { navigator.back() } }
    WhileShown(model) { model.watch() }
    val shown = when (val l = load) {
        is AdminLoad.Ready -> AdminLoad.Ready(Shown(l.value))
        is AdminLoad.Failed -> AdminLoad.Failed(l.error, l.stale?.let(::Shown))
        AdminLoad.Loading -> AdminLoad.Loading
    }
    val saved = (shown as? AdminLoad.Ready)?.value?.source ?: (shown as? AdminLoad.Failed)?.stale?.source
    val title = when {
        model.adding -> stringResource(R.string.admin_livetv_source_add_title)
        else -> saved?.name ?: stringResource(R.string.admin_section_livetv)
    }
    AdminPage(title = title, load = shown, onRetry = model.source::reload, tag = LIVETV_SOURCE_PAGE) { page ->
        val source = page.source
        if (!model.adding && source == null) {
            item(key = "gone") { EmptyState(stringResource(R.string.admin_livetv_source_gone), body = null, icon = PleyIcons.Radio) }
            return@AdminPage
        }
        if (source != null) {
            item(key = "status") { Status(source, onEnabled = model::setEnabled) }
            item(key = "actions") {
                SettingsGroup(null, Modifier.adminWidth) {
                    SettingsRow(
                        stringResource(R.string.admin_livetv_source_read_now),
                        value = if (source.refreshing) stringResource(R.string.admin_livetv_sources_reading) else null,
                        icon = PleyIcons.RefreshCw,
                        enabled = !source.refreshing && source.enabled,
                        chevron = false,
                        onClick = model::refresh,
                    )
                    RowDivider()
                    SettingsRow(
                        stringResource(R.string.admin_livetv_source_remove),
                        icon = PleyIcons.Delete,
                        danger = true,
                        onClick = { removing = true },
                    )
                }
            }
        }
        val typed = draft ?: return@AdminPage
        item(key = "details") {
            Details(
                typed,
                source,
                adding = model.adding,
                onKind = { choosingKind = true },
                onField = { editing = it },
            )
        }
        item(key = "advanced") {
            SettingsGroup(stringResource(R.string.admin_livetv_source_advanced), Modifier.adminWidth) {
                SettingsRow(
                    stringResource(R.string.admin_livetv_source_user_agent),
                    value = typed.userAgent.ifEmpty { stringResource(R.string.admin_livetv_source_user_agent_hint) },
                    onClick = { editing = Field.UserAgent },
                )
            }
        }
        item(key = "test") { TestResult(testing, typed) }
        item(key = "buttons") {
            Row(Modifier.adminWidth, horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.End)) {
                TonalButton(
                    stringResource(R.string.admin_livetv_source_test),
                    onClick = model::test,
                    enabled = typed.complete && testing != SourceTesting.Testing && busy == 0,
                    modifier = Modifier.testTag(LIVETV_SOURCE_TEST),
                )
                PrimaryButton(
                    stringResource(if (model.adding) R.string.admin_livetv_source_add else R.string.admin_save),
                    onClick = model::save,
                    enabled = typed.complete && model.changed(typed) && busy == 0,
                    modifier = Modifier.testTag(LIVETV_SOURCE_SAVE),
                )
            }
        }
    }
    val typed = draft
    if (choosingKind && typed != null) {
        ChoiceSheet(
            title = stringResource(R.string.admin_livetv_source_kind),
            choices = listOf(
                Choice(LiveSourceKind.M3u, stringResource(R.string.admin_livetv_source_m3u), stringResource(R.string.admin_livetv_source_m3u_text)),
                Choice(LiveSourceKind.Xtream, "Xtream Codes", stringResource(R.string.admin_livetv_source_xtream_text)),
            ),
            selected = typed.kind,
            onPick = { kind -> model.edit { it.copy(kind = kind) } },
            onDismiss = { choosingKind = false },
        )
    }
    val field = editing
    if (field != null && typed != null) {
        FieldDialog(field, typed, saved, onSave = { value -> model.edit { set(it, field, value) } }, onDismiss = { editing = null })
    }
    if (removing && saved != null) {
        ConfirmDialog(
            title = stringResource(R.string.admin_livetv_source_remove_title, saved.name),
            body = stringResource(R.string.admin_livetv_source_remove_body),
            confirmLabel = stringResource(R.string.admin_livetv_source_remove),
            onConfirm = {
                removing = false
                model.remove()
            },
            onDismiss = { removing = false },
            destructive = true,
        )
    }
}

/** A saved source's state: on or off, its channels, guide, connections, when it was read, and what is wrong. */
@Composable
private fun Status(source: LiveSource, onEnabled: (Boolean) -> Unit) {
    val words = formatWords()
    val locale = shownLocale()
    val shownChannels = source.channelCount - source.hiddenCount
    SettingsGroup(null, Modifier.adminWidth) {
        SwitchRow(
            stringResource(R.string.admin_livetv_source_enabled),
            checked = source.enabled,
            onChange = onEnabled,
            subtitle = kindName(source.kind),
        )
        RowDivider(inset = false)
        InfoRow(
            stringResource(R.string.admin_livetv_channels),
            value = if (source.channelsAt != null) NumberFormat.getIntegerInstance(locale).format(shownChannels) else "—",
            note = source.hiddenCount.takeIf { it > 0 }?.let { pluralStringResource(R.plurals.admin_livetv_sources_hidden, it, it) },
        )
        RowDivider(inset = false)
        val guided = if (source.channelCount > 0) source.guidedCount.toDouble() / source.channelCount else 0.0
        InfoRow(
            stringResource(R.string.admin_livetv_sources_in_guide),
            value = if (source.guideAt != null) NumberFormat.getPercentInstance(locale).format(guided) else "—",
            note = if (source.guideAt != null) {
                pluralStringResource(R.plurals.admin_livetv_sources_guide_channels, source.guidedCount, source.guidedCount)
            } else {
                stringResource(R.string.admin_livetv_sources_no_guide_yet)
            },
        )
        RowDivider(inset = false)
        InfoRow(
            stringResource(R.string.admin_livetv_sources_connections),
            value = if (source.maxConnections == 0) {
                pluralStringResource(R.plurals.admin_livetv_sources_in_use_count, source.connectionsInUse, source.connectionsInUse)
            } else {
                stringResource(R.string.admin_livetv_sources_of_max, source.connectionsInUse, source.maxConnections)
            },
            note = when {
                source.maxConnections == 0 -> stringResource(R.string.admin_livetv_sources_no_limit)
                source.connectionsInUse >= source.maxConnections -> stringResource(R.string.admin_livetv_sources_all_in_use)
                else -> stringResource(R.string.admin_livetv_sources_in_use)
            },
        )
        RowDivider(inset = false)
        InfoRow(
            stringResource(R.string.admin_livetv_sources_read),
            value = if (source.refreshing) stringResource(R.string.admin_livetv_sources_reading) else formatRelative(source.channelsAt, words = words),
            note = source.guideAt?.let { stringResource(R.string.admin_livetv_sources_guide_ago, formatRelative(it, words = words)) },
        )
        source.account?.let { account ->
            val soon = expiresSoon(account)
            RowDivider(inset = false)
            InfoRow(
                accountLine(account),
                value = soon?.let { if (it <= 0) stringResource(R.string.admin_livetv_sources_renew) else pluralStringResource(R.plurals.admin_livetv_sources_days_left, it, it) },
                icon = PleyIcons.CalendarDays,
                danger = soon != null,
            )
        }
        source.channelsError?.let {
            RowDivider(inset = false)
            InfoRow(it, null, icon = PleyIcons.CircleAlert, danger = true)
        }
        source.guideError?.let {
            RowDivider(inset = false)
            InfoRow(stringResource(R.string.admin_livetv_sources_guide_error, it), null, icon = PleyIcons.CircleAlert, danger = true)
        }
    }
}

/** The details as typed: kind (when adding), name, address, account, guide and connections, each opening its dialog. */
@Composable
private fun Details(draft: SourceDraft, source: LiveSource?, adding: Boolean, onKind: () -> Unit, onField: (Field) -> Unit) {
    val xtream = draft.kind == LiveSourceKind.Xtream
    SettingsGroup(
        if (adding) null else stringResource(R.string.admin_livetv_source_details),
        Modifier.adminWidth,
        footer = stringResource(R.string.admin_livetv_source_connections_note),
    ) {
        if (adding) {
            SettingsRow(stringResource(R.string.admin_livetv_source_kind), value = kindName(draft.kind), onClick = onKind)
            RowDivider(inset = false)
        }
        SettingsRow(
            stringResource(R.string.admin_livetv_source_name),
            value = draft.name.ifEmpty { defaultSourceName(draft.url) },
            onClick = { onField(Field.Name) },
        )
        RowDivider(inset = false)
        SettingsRow(
            stringResource(if (xtream) R.string.admin_livetv_source_server else R.string.admin_livetv_source_playlist),
            value = draft.url,
            onClick = { onField(Field.Url) },
            modifier = Modifier.testTag(LIVETV_SOURCE_URL),
        )
        if (xtream) {
            RowDivider(inset = false)
            SettingsRow(stringResource(R.string.admin_livetv_source_user_name), value = draft.username, onClick = { onField(Field.Username) })
            RowDivider(inset = false)
            SettingsRow(
                stringResource(R.string.admin_livetv_source_password),
                value = when {
                    draft.password.isNotEmpty() -> "••••••••"
                    source?.passwordSet == true -> stringResource(R.string.admin_livetv_source_password_saved)
                    else -> null
                },
                onClick = { onField(Field.Password) },
            )
        }
        RowDivider(inset = false)
        SettingsRow(
            stringResource(R.string.admin_livetv_source_guide),
            value = draft.guideUrl.ifEmpty {
                stringResource(if (xtream) R.string.admin_livetv_source_guide_hint_xtream else R.string.admin_livetv_source_guide_hint_m3u)
            },
            onClick = { onField(Field.Guide) },
        )
        RowDivider(inset = false)
        SettingsRow(
            stringResource(R.string.admin_livetv_source_connections),
            value = if (draft.maxConnections == 0) stringResource(R.string.admin_livetv_source_connections_hint) else draft.maxConnections.toString(),
            onClick = { onField(Field.Connections) },
        )
    }
}

/** What testing the details found, or why it couldn't: shown while the details are still those tested. */
@Composable
private fun TestResult(testing: SourceTesting, draft: SourceDraft) {
    when {
        testing is SourceTesting.Found && testing.draft == draft -> SettingsGroup(null, Modifier.adminWidth) {
            val result = testing.result
            val channels = pluralStringResource(R.plurals.admin_livetv_source_found_channels, result.channels, result.channels)
            val groups = pluralStringResource(R.plurals.admin_livetv_source_found_groups, result.groups, result.groups)
            InfoRow(
                pluralStringResource(R.plurals.admin_livetv_source_found, result.channels, channels, groups),
                value = result.account?.let { accountLine(it) },
                note = result.guideUrl?.let { stringResource(R.string.admin_livetv_source_guide_found, it) }
                    ?: stringResource(R.string.admin_livetv_source_no_guide),
                icon = PleyIcons.CircleCheck,
            )
        }
        testing is SourceTesting.Failed && testing.draft == draft -> SettingsGroup(null, Modifier.adminWidth) {
            InfoRow(errorSentence(testing.error).asString(), null, icon = PleyIcons.TriangleAlert, danger = true)
        }
    }
}

/** The dialog [field] is typed into. */
@Composable
private fun FieldDialog(field: Field, draft: SourceDraft, source: LiveSource?, onSave: (String) -> Unit, onDismiss: () -> Unit) {
    val xtream = draft.kind == LiveSourceKind.Xtream
    val save: (String) -> Unit = {
        onSave(it)
        onDismiss()
    }
    when (field) {
        Field.Name -> TextEntryDialog(
            title = stringResource(R.string.admin_livetv_source_name),
            initial = draft.name,
            onSave = save,
            onDismiss = onDismiss,
            hint = stringResource(R.string.admin_livetv_source_name_hint),
        )
        Field.Url -> TextEntryDialog(
            title = stringResource(if (xtream) R.string.admin_livetv_source_server else R.string.admin_livetv_source_playlist),
            initial = draft.url,
            onSave = save,
            onDismiss = onDismiss,
            hint = stringResource(if (xtream) R.string.admin_livetv_source_server_hint else R.string.admin_livetv_source_playlist_hint),
            entry = Entry.Address,
        )
        Field.Username -> TextEntryDialog(
            title = stringResource(R.string.admin_livetv_source_user_name),
            initial = draft.username,
            onSave = save,
            onDismiss = onDismiss,
        )
        Field.Password -> TextEntryDialog(
            title = stringResource(R.string.admin_livetv_source_password),
            initial = draft.password,
            onSave = save,
            onDismiss = onDismiss,
            hint = if (source?.passwordSet == true) stringResource(R.string.admin_livetv_source_password_saved) else null,
            entry = Entry.Secret,
        )
        Field.Guide -> TextEntryDialog(
            title = stringResource(R.string.admin_livetv_source_guide),
            initial = draft.guideUrl,
            onSave = save,
            onDismiss = onDismiss,
            hint = stringResource(if (xtream) R.string.admin_livetv_source_guide_hint_xtream else R.string.admin_livetv_source_guide_hint_m3u),
            entry = Entry.Address,
        )
        Field.Connections -> TextEntryDialog(
            title = stringResource(R.string.admin_livetv_source_connections),
            initial = draft.maxConnections.toString(),
            onSave = save,
            onDismiss = onDismiss,
            hint = stringResource(R.string.admin_livetv_source_connections_hint),
            entry = Entry.Number,
            check = numberProblem(CONNECTIONS),
        )
        Field.UserAgent -> TextEntryDialog(
            title = stringResource(R.string.admin_livetv_source_user_agent),
            initial = draft.userAgent,
            onSave = save,
            onDismiss = onDismiss,
            hint = stringResource(R.string.admin_livetv_source_user_agent_hint),
        )
    }
}

/** [draft] with [field] set to [value]. */
private fun set(draft: SourceDraft, field: Field, value: String): SourceDraft = when (field) {
    Field.Name -> draft.copy(name = value)
    Field.Url -> draft.copy(url = value)
    Field.Username -> draft.copy(username = value)
    Field.Password -> draft.copy(password = value)
    Field.Guide -> draft.copy(guideUrl = value)
    Field.Connections -> draft.copy(maxConnections = numberIn(value, CONNECTIONS)?.toInt() ?: draft.maxConnections)
    Field.UserAgent -> draft.copy(userAgent = value)
}

/** How many connections a source may have, as the web's field takes them; 0: no limit. */
private val CONNECTIONS = 0L..100L

internal const val LIVETV_SOURCE_PAGE = "pley.admin.livetv.source"
internal const val LIVETV_SOURCE_URL = "pley.admin.livetv.source.url"
internal const val LIVETV_SOURCE_TEST = "pley.admin.livetv.source.test"
internal const val LIVETV_SOURCE_SAVE = "pley.admin.livetv.source.save"
