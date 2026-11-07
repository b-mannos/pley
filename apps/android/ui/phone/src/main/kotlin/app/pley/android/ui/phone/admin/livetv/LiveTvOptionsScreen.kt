package app.pley.android.ui.phone.admin.livetv

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.designsystem.PleyColors
import app.pley.android.model.admin.LiveTvAccess
import app.pley.android.model.admin.LiveTvPerson
import app.pley.android.model.admin.LiveTvSettings
import app.pley.android.screens.admin.livetv.LiveTvOptionsViewModel
import app.pley.android.screens.admin.livetv.PersonAccess
import app.pley.android.screens.admin.livetv.guideDayChoices
import app.pley.android.screens.admin.livetv.heightChoices
import app.pley.android.screens.admin.livetv.personAccess
import app.pley.android.screens.admin.livetv.refreshChoices
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.Choice
import app.pley.android.ui.phone.account.ChoiceSheet
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.account.SettingsRow
import app.pley.android.ui.phone.account.SwitchRow
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.adminWidth

/** The values of Options chosen in a sheet. */
private enum class Pick { Access, Height, Refresh, GuideDays }

/**
 * Live TV's Options, as the web's tab: who watches (the rule, profiles
 * with an age limit, and each person's own), how channels play, and how
 * often channels and guides are read. Each value is sent as it is chosen.
 */
@Composable
internal fun LiveTvOptionsScreen() {
    val model: LiveTvOptionsViewModel = viewModel(factory = LiveTvOptionsViewModel.factory)
    val load by model.options.state.collectAsStateWithLifecycle()
    val people by model.people.state.collectAsStateWithLifecycle()
    var picking by rememberSaveable { mutableStateOf<Pick?>(null) }
    var person by rememberSaveable { mutableStateOf<Long?>(null) }
    AdminEvents(model)
    AdminPage(
        title = stringResource(R.string.admin_livetv_tab_options),
        load = load,
        onRetry = model.options::reload,
        tag = LIVETV_OPTIONS_PAGE,
    ) { options ->
        val s = options.settings
        item(key = "who") {
            SettingsGroup(stringResource(R.string.admin_livetv_options_who), Modifier.adminWidth, footer = stringResource(R.string.admin_livetv_options_who_hint)) {
                SettingsRow(
                    stringResource(R.string.admin_livetv_options_open_to),
                    value = accessName(s.access),
                    onClick = { picking = Pick.Access },
                )
                RowDivider(inset = false)
                // Administrators always watch: the rule for age limits says nothing then, as the web's switch is off.
                EnabledSwitchRow(
                    stringResource(R.string.admin_livetv_options_age_limited),
                    subtitle = stringResource(R.string.admin_livetv_options_age_limited_hint),
                    checked = s.ageLimited,
                    enabled = s.access != LiveTvAccess.Admins,
                    onChange = model::setAgeLimited,
                )
            }
        }
        val listed = people.valueOrNull.orEmpty()
        if (listed.isNotEmpty()) {
            item(key = "people") { People(listed, s, onPick = { person = it }) }
        }
        item(key = "playing") {
            SettingsGroup(
                stringResource(R.string.admin_livetv_options_playing),
                Modifier.adminWidth,
                footer = stringResource(R.string.admin_livetv_options_playing_hint) + " " + stringResource(R.string.admin_livetv_options_h264_note),
            ) {
                SwitchRow(
                    stringResource(R.string.admin_livetv_options_deinterlace),
                    checked = s.deinterlace,
                    onChange = model::setDeinterlace,
                    subtitle = stringResource(R.string.admin_livetv_options_deinterlace_hint),
                )
                RowDivider(inset = false)
                SettingsRow(
                    stringResource(R.string.admin_livetv_options_transcoded),
                    value = heightName(s.maxHeight),
                    onClick = { picking = Pick.Height },
                )
            }
        }
        item(key = "guide") {
            SettingsGroup(stringResource(R.string.admin_livetv_options_channels_and_guide), Modifier.adminWidth) {
                SettingsRow(
                    stringResource(R.string.admin_livetv_options_read_every),
                    value = everyName(s.refreshHours),
                    onClick = { picking = Pick.Refresh },
                )
                RowDivider(inset = false)
                SwitchRow(
                    stringResource(R.string.admin_livetv_options_hide_offline),
                    checked = s.hideOffline,
                    onChange = model::setHideOffline,
                    subtitle = stringResource(R.string.admin_livetv_options_hide_offline_hint),
                )
                RowDivider(inset = false)
                SettingsRow(
                    stringResource(R.string.admin_livetv_options_guide_days),
                    value = pluralStringResource(R.plurals.admin_livetv_options_days, s.guideDays, s.guideDays),
                    onClick = { picking = Pick.GuideDays },
                )
            }
        }
    }
    val settings = load.valueOrNull?.settings ?: return
    when (picking) {
        Pick.Access -> ChoiceSheet(
            title = stringResource(R.string.admin_livetv_options_open_to),
            choices = listOf(
                Choice(LiveTvAccess.Everyone, accessName(LiveTvAccess.Everyone)),
                Choice(LiveTvAccess.Home, accessName(LiveTvAccess.Home), stringResource(R.string.admin_livetv_options_home_note)),
                Choice(LiveTvAccess.Admins, accessName(LiveTvAccess.Admins)),
            ),
            selected = settings.access,
            onPick = model::setAccess,
            onDismiss = { picking = null },
        )
        Pick.Height -> ChoiceSheet(
            title = stringResource(R.string.admin_livetv_options_transcoded),
            choices = heightChoices(settings.maxHeight).map { Choice(it, heightName(it)) },
            selected = settings.maxHeight,
            onPick = model::setMaxHeight,
            onDismiss = { picking = null },
        )
        Pick.Refresh -> ChoiceSheet(
            title = stringResource(R.string.admin_livetv_options_read_every),
            choices = refreshChoices(settings.refreshHours).map { Choice(it, everyName(it)) },
            selected = settings.refreshHours,
            onPick = model::setRefreshHours,
            onDismiss = { picking = null },
        )
        Pick.GuideDays -> ChoiceSheet(
            title = stringResource(R.string.admin_livetv_options_guide_days),
            choices = guideDayChoices(settings.guideDays).map { Choice(it, pluralStringResource(R.plurals.admin_livetv_options_days, it, it)) },
            selected = settings.guideDays,
            onPick = model::setGuideDays,
            onDismiss = { picking = null },
        )
        null -> Unit
    }
    val chosen = person?.let { id -> people.valueOrNull?.firstOrNull { it.id == id } }
    if (chosen != null) {
        ChoiceSheet(
            title = stringResource(R.string.admin_livetv_options_person_access, chosen.name),
            choices = listOf(
                Choice(PersonAccess.AsOpenTo, stringResource(R.string.admin_livetv_options_as_open_to)),
                Choice(PersonAccess.Always, stringResource(R.string.admin_livetv_options_always)),
                Choice(PersonAccess.Never, stringResource(R.string.admin_livetv_options_never)),
            ),
            selected = personAccess(settings, chosen.id),
            onPick = { model.setPerson(chosen.id, it) },
            onDismiss = { person = null },
        )
    }
}

/** Each person's own access, whatever "Open to" says: as the rule says, always, or never. */
@Composable
private fun People(people: List<LiveTvPerson>, settings: LiveTvSettings, onPick: (Long) -> Unit) {
    SettingsGroup(stringResource(R.string.admin_livetv_options_people), Modifier.adminWidth, footer = stringResource(R.string.admin_livetv_options_people_hint)) {
        people.forEachIndexed { i, p ->
            if (i > 0) RowDivider(inset = false)
            val access = when (personAccess(settings, p.id)) {
                PersonAccess.AsOpenTo -> stringResource(R.string.admin_livetv_options_as_open_to)
                PersonAccess.Always -> stringResource(R.string.admin_livetv_options_always)
                PersonAccess.Never -> stringResource(R.string.admin_livetv_options_never)
            }
            SettingsRow(
                if (p.home) p.name else p.name + " · " + stringResource(R.string.admin_livetv_options_not_in_home),
                value = access,
                onClick = { onPick(p.id) },
            )
        }
    }
}

/**
 * [SwitchRow] that can be off limits: shown at 38 % and doing nothing
 * while [enabled] is false, as the web's switch for profiles with an age
 * limit while Live TV is open to administrators only.
 */
@Composable
private fun EnabledSwitchRow(title: String, subtitle: String, checked: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = if (enabled) PleyColors.Fg else PleyColors.Disabled)
            Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = if (enabled) PleyColors.Muted else PleyColors.Disabled)
        }
        Spacer(Modifier.width(12.dp))
        Switch(
            checked = checked,
            onCheckedChange = null,
            enabled = enabled,
            colors = SwitchDefaults.colors(
                checkedThumbColor = PleyColors.OnAccent,
                checkedTrackColor = PleyColors.Accent,
                uncheckedThumbColor = PleyColors.Control,
                uncheckedTrackColor = PleyColors.Surface3,
                uncheckedBorderColor = PleyColors.Control,
            ),
        )
    }
}

@Composable
private fun accessName(access: LiveTvAccess): String = when (access) {
    LiveTvAccess.Home -> stringResource(R.string.admin_livetv_options_home_users)
    LiveTvAccess.Admins -> stringResource(R.string.admin_livetv_options_admins_only)
    else -> stringResource(R.string.admin_livetv_options_everyone)
}

/** "1080p", or as the channel sends it. */
@Composable
private fun heightName(height: Int?): String = height?.let { "${it}p" } ?: stringResource(R.string.admin_livetv_options_as_sent)

/** How often channels and guides are read, as "Read again every" ends: "6 hours", "day", "week". */
@Composable
private fun everyName(hours: Int): String = when (hours) {
    168 -> stringResource(R.string.admin_livetv_options_every_week)
    24 -> stringResource(R.string.admin_livetv_options_every_day)
    48 -> stringResource(R.string.admin_livetv_options_every_two_days)
    else -> pluralStringResource(R.plurals.admin_livetv_options_every_hours, hours, hours)
}

internal const val LIVETV_OPTIONS_PAGE = "pley.admin.livetv.options"
