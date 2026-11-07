package app.pley.android.ui.phone.admin.livetv

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.text.formatWords
import app.pley.android.model.admin.LiveSource
import app.pley.android.model.admin.LiveSourceKind
import app.pley.android.model.admin.TunerStatus
import app.pley.android.model.format.formatRelative
import app.pley.android.screens.PleyKey
import app.pley.android.screens.admin.AdminSection
import app.pley.android.screens.admin.livetv.LiveTvAdminPage
import app.pley.android.screens.admin.livetv.LiveTvAdminViewModel
import app.pley.android.screens.admin.livetv.expiresSoon
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.account.SettingsRow
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.kit.ConfirmDialog
import app.pley.android.ui.phone.kit.EmptyState
import app.pley.android.ui.phone.kit.PleyIconButton
import app.pley.android.ui.phone.kit.PleyTextButton
import app.pley.android.ui.phone.kit.PrimaryButton
import app.pley.android.ui.phone.shell.LocalPhoneNavigator

/**
 * Live TV's administration (the web's Settings → Live TV): [id] names
 * the page ([LiveTvAdminPage]), its first page when it names none.
 */
@Composable
internal fun LiveTvAdminScreen(id: String?) {
    when (val page = LiveTvAdminPage.of(id)) {
        LiveTvAdminPage.Front -> LiveTvFrontScreen()
        LiveTvAdminPage.Options -> LiveTvOptionsScreen()
        LiveTvAdminPage.Groups -> LineupGroupsScreen()
        LiveTvAdminPage.Channels -> LineupScreen()
        is LiveTvAdminPage.Source -> LiveSourceScreen(page.sourceId)
    }
}

/** Opens Live TV's page [page]. */
internal fun manage(page: LiveTvAdminPage) = PleyKey.Manage(AdminSection.LiveTv, page.id)

/**
 * Live TV's first page, as the web's Sources tab: what is being watched
 * now (read again every few seconds while it shows; a channel can be
 * stopped), the sources with their state, and the way to Groups,
 * Channels and Options. Groups and Channels wait for a source, as the
 * web's tabs do.
 */
@Composable
private fun LiveTvFrontScreen() {
    val model: LiveTvAdminViewModel = viewModel(factory = LiveTvAdminViewModel.factory)
    val load by model.sources.state.collectAsStateWithLifecycle()
    val tuners by model.tuners.collectAsStateWithLifecycle()
    val navigator = LocalPhoneNavigator.current
    var stopping by remember { mutableStateOf<TunerStatus?>(null) }
    AdminEvents(model, liveTvSaid())
    WhileShown(model) { model.watch() }
    val hasSources = load.valueOrNull?.isNotEmpty() == true
    AdminPage(
        title = stringResource(R.string.admin_section_livetv),
        load = load,
        onRetry = model.sources::reload,
        tag = LIVETV_ADMIN_PAGE,
        actions = {
            if (hasSources) {
                PleyIconButton(PleyIcons.Tv, stringResource(R.string.admin_livetv_open), onClick = { navigator.open(PleyKey.LiveTv) })
            }
        },
    ) { sources ->
        item(key = "intro") { PageLine(stringResource(R.string.admin_livetv_intro), Modifier.adminWidth) }
        if (sources.isEmpty()) {
            item(key = "none") {
                EmptyState(
                    title = stringResource(R.string.admin_livetv_sources_none_title),
                    body = stringResource(R.string.admin_livetv_sources_none_body),
                    icon = PleyIcons.Radio,
                    action = {
                        PrimaryButton(
                            stringResource(R.string.admin_livetv_sources_add_first),
                            onClick = { navigator.open(manage(LiveTvAdminPage.Source(null))) },
                            icon = PleyIcons.Plus,
                        )
                    },
                )
            }
        } else {
            item(key = "watching") { Watching(tuners, onStop = { stopping = it }) }
            item(key = "sources") {
                SettingsGroup(stringResource(R.string.admin_livetv_sources_title), Modifier.adminWidth, footer = stringResource(R.string.admin_livetv_sources_hint)) {
                    for (source in sources) {
                        SourceRow(source, onClick = { navigator.open(manage(LiveTvAdminPage.Source(source.id))) })
                        RowDivider()
                    }
                    SettingsRow(
                        stringResource(R.string.admin_livetv_sources_add),
                        icon = PleyIcons.Plus,
                        onClick = { navigator.open(manage(LiveTvAdminPage.Source(null))) },
                        modifier = Modifier.testTag(LIVETV_ADD_SOURCE),
                    )
                }
            }
        }
        item(key = "pages") {
            SettingsGroup(null, Modifier.adminWidth) {
                SettingsRow(
                    stringResource(R.string.admin_livetv_tab_groups),
                    value = stringResource(R.string.admin_livetv_tab_groups_value),
                    icon = PleyIcons.Folder,
                    enabled = hasSources,
                    onClick = { navigator.open(manage(LiveTvAdminPage.Groups)) },
                )
                RowDivider()
                SettingsRow(
                    stringResource(R.string.admin_livetv_tab_channels),
                    value = stringResource(R.string.admin_livetv_tab_channels_value),
                    icon = PleyIcons.ListVideo,
                    enabled = hasSources,
                    onClick = { navigator.open(manage(LiveTvAdminPage.Channels)) },
                )
                RowDivider()
                SettingsRow(
                    stringResource(R.string.admin_livetv_tab_options),
                    value = stringResource(R.string.admin_livetv_tab_options_value),
                    icon = PleyIcons.SlidersHorizontal,
                    onClick = { navigator.open(manage(LiveTvAdminPage.Options)) },
                )
            }
        }
    }
    stopping?.let { tuner ->
        ConfirmDialog(
            title = stringResource(R.string.admin_livetv_tuner_stop_title, tuner.channelName),
            body = stringResource(R.string.admin_livetv_tuner_stop_body),
            confirmLabel = stringResource(R.string.admin_livetv_tuner_stop),
            onConfirm = {
                model.stop(tuner)
                stopping = null
            },
            onDismiss = { stopping = null },
            destructive = true,
        )
    }
}

/** What is being watched now: each connection, who is on it, what it carries; Stop on each. */
@Composable
private fun Watching(tuners: List<TunerStatus>?, onStop: (TunerStatus) -> Unit) {
    SettingsGroup(stringResource(R.string.admin_livetv_watching_title), Modifier.adminWidth) {
        when {
            tuners == null -> InfoRow(stringResource(R.string.admin_loading), null, icon = PleyIcons.Radio)
            tuners.isEmpty() -> InfoRow(stringResource(R.string.admin_livetv_watching_nobody), null, icon = PleyIcons.Radio)
            else -> tuners.forEachIndexed { i, tuner ->
                if (i > 0) RowDivider(inset = false)
                TunerRow(tuner, onStop = { onStop(tuner) })
            }
        }
    }
}

@Composable
private fun TunerRow(tuner: TunerStatus, onStop: () -> Unit) {
    val dot = when {
        tuner.error != null -> PleyColors.Danger
        tuner.connected -> PleyColors.Success
        else -> PleyColors.Control
    }
    val viewers = tuner.viewers.joinToString(", ") { it.username }.ifEmpty { stringResource(R.string.admin_livetv_watching_nobody_on_it) }
    val details = listOfNotNull(
        viewers,
        tuner.kbps.takeIf { it > 0 }?.let { rateLine(it) },
        forLine(tuner.startedAt),
        tuner.plan?.let { planLine(it) },
    ).joinToString(" · ")
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(start = 16.dp, end = 8.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(8.dp).background(dot, PleyShape.Circle))
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(tuner.channelName, style = MaterialTheme.typography.bodyLarge, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val source = listOfNotNull(tuner.sourceName, stringResource(R.string.admin_livetv_watching_letting_go).takeIf { tuner.idle })
            Text(source.joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = PleyColors.Subtle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(details, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted, maxLines = 3, overflow = TextOverflow.Ellipsis)
            tuner.error?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = PleyColors.DangerText, maxLines = 2, overflow = TextOverflow.Ellipsis) }
        }
        PleyTextButton(stringResource(R.string.admin_livetv_tuner_stop), onClick = onStop)
    }
}

/** A source in the list: its name, kind and channels, whether it is off or being read, and what is wrong with it. */
@Composable
private fun SourceRow(source: LiveSource, onClick: () -> Unit) {
    val words = formatWords()
    val kind = kindName(source.kind)
    val state = when {
        source.refreshing -> stringResource(R.string.admin_livetv_sources_reading)
        !source.enabled -> stringResource(R.string.admin_livetv_sources_off)
        source.channelsAt != null -> stringResource(R.string.admin_livetv_sources_read_ago, formatRelative(source.channelsAt, words = words))
        else -> null
    }
    val channels = source.channelsAt?.let {
        val shown = source.channelCount - source.hiddenCount
        pluralStringResource(R.plurals.admin_livetv_channels_count, shown, shown)
    }
    val soon = expiresSoon(source.account)
    val problem = source.channelsError
        ?: source.guideError?.let { stringResource(R.string.admin_livetv_sources_guide_error, it) }
        ?: soon?.let { if (it <= 0) stringResource(R.string.admin_livetv_sources_renew) else pluralStringResource(R.plurals.admin_livetv_sources_days_left, it, it) }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(if (source.kind == LiveSourceKind.Xtream) PleyIcons.Server else PleyIcons.ListVideo, contentDescription = null, size = 24.dp, tint = PleyColors.Muted)
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                source.name,
                style = MaterialTheme.typography.bodyLarge,
                color = if (source.enabled) PleyColors.Fg else PleyColors.Muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                listOfNotNull(kind, channels, state).joinToString(" · "),
                style = MaterialTheme.typography.bodyMedium,
                color = PleyColors.Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (problem != null) {
                Text(problem, style = MaterialTheme.typography.bodySmall, color = PleyColors.DangerText, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        Spacer(Modifier.width(12.dp))
        Icon(PleyIcons.ChevronRight, contentDescription = null, size = 20.dp, tint = PleyColors.Subtle)
    }
}

/** A kind of source, as its provider names it, in every language: "M3U", "Xtream Codes". */
internal fun kindName(kind: LiveSourceKind): String = when (kind) {
    LiveSourceKind.Xtream -> "Xtream Codes"
    else -> "M3U"
}

internal const val LIVETV_ADMIN_PAGE = "pley.admin.livetv"
internal const val LIVETV_ADD_SOURCE = "pley.admin.livetv.add"
