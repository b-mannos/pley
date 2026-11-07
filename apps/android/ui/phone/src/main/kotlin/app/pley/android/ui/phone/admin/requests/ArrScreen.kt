package app.pley.android.ui.phone.admin.requests

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedSecureTextField
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.errorSentence
import app.pley.android.common.resolve
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.text.formatWords
import app.pley.android.model.ArrService
import app.pley.android.model.QualityProfile
import app.pley.android.model.RootFolder
import app.pley.android.model.admin.RadarrAvailability
import app.pley.android.model.admin.RequestsSettings
import app.pley.android.model.format.formatBytes
import app.pley.android.screens.admin.requests.ArrAnswer
import app.pley.android.screens.admin.requests.ArrTrial
import app.pley.android.screens.admin.requests.RequestsAdminViewModel
import app.pley.android.screens.admin.requests.arrAddress
import app.pley.android.screens.admin.requests.arrNeedsKey
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.Choice
import app.pley.android.ui.phone.account.ChoiceSheet
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.admin.server.GroupLine
import app.pley.android.ui.phone.admin.server.NoteLine
import app.pley.android.ui.phone.admin.server.RowNote
import app.pley.android.ui.phone.admin.server.ToggleRow
import app.pley.android.ui.phone.admin.server.ValueRow

/** Where a service is reached, as the web's examples say. */
private fun example(service: ArrService): String = if (service == ArrService.Sonarr) "http://localhost:8989" else "http://localhost:7878"

/** What is being chosen or typed on a service's page; one at a time. */
private enum class Pick { Connection, Profile, Folder, Availability, Language, AnimeProfile, AnimeFolder }

/**
 * Radarr's or Sonarr's page: its address and key, typed in together and
 * tried with Connect before they are saved, then what requests are sent
 * with, chosen from what the saved address offers.
 */
@Composable
internal fun ArrScreen(service: ArrService) {
    val model: RequestsAdminViewModel = viewModel(key = "admin-requests-${service.wire}", factory = RequestsAdminViewModel.factory(service))
    val load by model.settings.state.collectAsStateWithLifecycle()
    val answer by model.offer.collectAsStateWithLifecycle()
    val trial by model.trial.collectAsStateWithLifecycle()
    var pick by rememberSaveable { mutableStateOf<Pick?>(null) }
    val name = serviceName(service)
    AdminEvents(model)
    val offer = (answer as? ArrAnswer.Ready)?.offer
    AdminPage(title = name, load = load, onRetry = model.settings::reload, tag = "$REQUESTS_PAGE.${service.wire}") { s ->
        val saved = s.of(service)
        item(key = "connection") {
            SettingsGroup(null, Modifier.adminWidth) {
                GroupLine(stringResource(if (service == ArrService.Sonarr) R.string.admin_requests_sonarr_description else R.string.admin_requests_radarr_description))
                ValueRow(
                    stringResource(R.string.admin_requests_address),
                    saved.url.ifEmpty { stringResource(R.string.admin_requests_not_set) },
                    hint = stringResource(R.string.admin_requests_address_hint, name, example(service)),
                    onClick = { pick = Pick.Connection },
                )
                RowDivider(inset = false)
                ValueRow(
                    stringResource(R.string.admin_requests_api_key),
                    stringResource(if (saved.keySet) R.string.admin_requests_key_saved else R.string.admin_requests_not_set),
                    hint = stringResource(R.string.admin_requests_api_key_hint, name),
                    onClick = { pick = Pick.Connection },
                )
                if (saved.url.isNotEmpty()) {
                    RowDivider(inset = false)
                    val (line, color) = answerLine(answer, name)
                    val failed = answer is ArrAnswer.Failed
                    ValueRow(
                        line,
                        if (failed) stringResource(R.string.admin_requests_ask_again) else null,
                        // What went wrong is the server's sentence; touching it asks again.
                        onClick = model::askAgain,
                        enabled = failed,
                        valueColor = color,
                        trailing = when (answer) {
                            ArrAnswer.Asking -> ({ Spinner() })
                            else -> ({})
                        },
                    )
                }
            }
        }
        if (saved.url.isNotEmpty()) {
            item(key = "choices") {
                SettingsGroup(null, Modifier.adminWidth) {
                    val ready = offer != null
                    ValueRow(
                        stringResource(R.string.admin_requests_quality_profile),
                        profileLabel(saved.qualityProfile, offer?.qualityProfiles, ready, stringResource(R.string.admin_requests_none_chosen)),
                        enabled = ready,
                        note = if (saved.qualityProfile == 0L) RowNote(stringResource(R.string.admin_requests_approving_fails), notice = true) else null,
                        onClick = { pick = Pick.Profile },
                    )
                    RowDivider(inset = false)
                    ValueRow(
                        stringResource(R.string.admin_requests_root_folder),
                        folderLabel(saved.rootFolder, offer?.rootFolders, ready, stringResource(R.string.admin_requests_none_chosen)),
                        enabled = ready,
                        note = if (saved.rootFolder.isEmpty()) RowNote(stringResource(R.string.admin_requests_approving_fails), notice = true) else null,
                        onClick = { pick = Pick.Folder },
                    )
                    if (service == ArrService.Radarr) {
                        RowDivider(inset = false)
                        ValueRow(
                            stringResource(R.string.admin_requests_minimum_availability),
                            availabilityLabel(s.radarrMinimumAvailability),
                            hint = stringResource(R.string.admin_requests_minimum_availability_hint),
                            onClick = { pick = Pick.Availability },
                        )
                    }
                    if (service == ArrService.Sonarr && (offer?.languageProfiles?.isNotEmpty() == true || s.sonarrLanguageProfile != 0L)) {
                        RowDivider(inset = false)
                        ValueRow(
                            stringResource(R.string.admin_requests_language_profile),
                            profileLabel(s.sonarrLanguageProfile, offer?.languageProfiles, ready, stringResource(R.string.admin_requests_sonarr_first)),
                            enabled = ready,
                            hint = stringResource(R.string.admin_requests_language_profile_hint),
                            onClick = { pick = Pick.Language },
                        )
                    }
                    RowDivider(inset = false)
                    ValueRow(
                        stringResource(R.string.admin_requests_anime_profile),
                        profileLabel(saved.animeQualityProfile, offer?.qualityProfiles, ready, stringResource(R.string.admin_requests_profile_above)),
                        enabled = ready,
                        hint = stringResource(R.string.admin_requests_anime_profile_hint),
                        onClick = { pick = Pick.AnimeProfile },
                    )
                    RowDivider(inset = false)
                    ValueRow(
                        stringResource(R.string.admin_requests_anime_folder),
                        folderLabel(saved.animeRootFolder, offer?.rootFolders, ready, stringResource(R.string.admin_requests_folder_above)),
                        enabled = ready,
                        onClick = { pick = Pick.AnimeFolder },
                    )
                    if (service == ArrService.Sonarr) {
                        RowDivider(inset = false)
                        ToggleRow(
                            stringResource(R.string.admin_requests_season_folders),
                            checked = s.sonarrSeasonFolders,
                            onChange = model::setSeasonFolders,
                            hint = stringResource(R.string.admin_requests_season_folders_hint),
                        )
                    }
                    RowDivider(inset = false)
                    ToggleRow(
                        stringResource(R.string.admin_requests_search),
                        checked = saved.search,
                        onChange = model::setSearch,
                        hint = stringResource(if (service == ArrService.Sonarr) R.string.admin_requests_search_seasons else R.string.admin_requests_search_movie),
                    )
                }
            }
        }
    }

    val s = load.valueOrNull ?: return
    val saved = s.of(service)
    val close = { pick = null }
    val words = formatWords()
    when (pick) {
        Pick.Connection -> ConnectionDialog(
            service = service,
            settings = s,
            trial = trial,
            onConnect = model::connect,
            onSave = { url, key ->
                model.saveConnection(url, key)
                close()
            },
            onDismiss = close,
        )
        Pick.Profile -> ChoiceSheet(
            title = stringResource(R.string.admin_requests_quality_profile),
            choices = profileChoices(saved.qualityProfile, offer?.qualityProfiles, stringResource(R.string.admin_requests_none_chosen)),
            selected = saved.qualityProfile,
            onPick = model::setQualityProfile,
            onDismiss = close,
        )
        Pick.AnimeProfile -> ChoiceSheet(
            title = stringResource(R.string.admin_requests_anime_profile),
            choices = profileChoices(saved.animeQualityProfile, offer?.qualityProfiles, stringResource(R.string.admin_requests_profile_above)),
            selected = saved.animeQualityProfile,
            onPick = model::setAnimeQualityProfile,
            onDismiss = close,
        )
        Pick.Language -> ChoiceSheet(
            title = stringResource(R.string.admin_requests_language_profile),
            choices = profileChoices(s.sonarrLanguageProfile, offer?.languageProfiles, stringResource(R.string.admin_requests_sonarr_first)),
            selected = s.sonarrLanguageProfile,
            onPick = model::setLanguageProfile,
            onDismiss = close,
        )
        Pick.Folder -> ChoiceSheet(
            title = stringResource(R.string.admin_requests_root_folder),
            choices = folderChoices(saved.rootFolder, offer?.rootFolders, stringResource(R.string.admin_requests_none_chosen)) { formatBytes(it, words) },
            selected = saved.rootFolder,
            onPick = model::setRootFolder,
            onDismiss = close,
        )
        Pick.AnimeFolder -> ChoiceSheet(
            title = stringResource(R.string.admin_requests_anime_folder),
            choices = folderChoices(saved.animeRootFolder, offer?.rootFolders, stringResource(R.string.admin_requests_folder_above)) { formatBytes(it, words) },
            selected = saved.animeRootFolder,
            onPick = model::setAnimeRootFolder,
            onDismiss = close,
        )
        Pick.Availability -> ChoiceSheet(
            title = stringResource(R.string.admin_requests_minimum_availability),
            choices = AVAILABILITY.map { Choice(it, availabilityLabel(it)) },
            selected = s.radarrMinimumAvailability,
            onPick = model::setMinimumAvailability,
            onDismiss = close,
        )
        null -> Unit
    }
}

private val AVAILABILITY = listOf(RadarrAvailability.Announced, RadarrAvailability.InCinemas, RadarrAvailability.Released)

@Composable
private fun availabilityLabel(availability: RadarrAvailability): String = stringResource(
    when (availability) {
        RadarrAvailability.Announced -> R.string.admin_requests_availability_announced
        RadarrAvailability.InCinemas -> R.string.admin_requests_availability_in_cinemas
        else -> R.string.admin_requests_availability_released
    },
)

/** What the saved service answered, in a line, and its colour. */
@Composable
private fun answerLine(answer: ArrAnswer, name: String): Pair<String, Color> {
    val resources = LocalResources.current
    return when (answer) {
        ArrAnswer.Asking -> stringResource(R.string.admin_requests_asking, name) to PleyColors.Muted
        is ArrAnswer.Ready -> "$name ${answer.offer.version}" to PleyColors.Success
        is ArrAnswer.Failed -> errorSentence(answer.error).resolve(resources) to PleyColors.DangerText
        ArrAnswer.None -> stringResource(R.string.admin_requests_connect_to_choose) to PleyColors.Muted
    }
}

/**
 * A profile's name; one saved that the service no longer has, or that
 * isn't known while it hasn't answered, by its id.
 */
@Composable
private fun profileLabel(id: Long, profiles: List<QualityProfile>?, ready: Boolean, none: String): String = when {
    id == 0L -> none
    else -> profiles?.firstOrNull { it.id == id }?.name
        ?: stringResource(if (ready) R.string.admin_requests_profile_gone else R.string.admin_requests_profile, id)
}

@Composable
private fun profileChoices(id: Long, profiles: List<QualityProfile>?, none: String): List<Choice<Long>> = buildList {
    add(Choice(0L, none))
    if (id != 0L && profiles?.none { it.id == id } != false) add(Choice(id, stringResource(R.string.admin_requests_profile_gone, id)))
    profiles?.forEach { add(Choice(it.id, it.name)) }
}

@Composable
private fun folderLabel(path: String, folders: List<RootFolder>?, ready: Boolean, none: String): String = when {
    path.isEmpty() -> none
    ready && folders?.none { it.path == path } == true -> stringResource(R.string.admin_requests_folder_gone, path)
    else -> path
}

@Composable
private fun folderChoices(path: String, folders: List<RootFolder>?, none: String, size: (Long) -> String): List<Choice<String>> = buildList {
    add(Choice("", none))
    if (path.isNotEmpty() && folders?.none { it.path == path } != false) add(Choice(path, stringResource(R.string.admin_requests_folder_gone, path)))
    folders?.forEach { f ->
        val free = f.freeBytes
        add(Choice(f.path, if (free != null) stringResource(R.string.admin_requests_folder_free, f.path, size(free)) else f.path))
    }
}

/**
 * The address and key, typed in together: the server takes a new address
 * only with its key. Connect tries them without saving; Save keeps them
 * (a key left empty keeps the saved one, for the same address).
 */
@Composable
private fun ConnectionDialog(
    service: ArrService,
    settings: RequestsSettings,
    trial: ArrTrial?,
    onConnect: (String, String) -> Unit,
    onSave: (String, String) -> Unit,
    onDismiss: () -> Unit,
) {
    val saved = settings.of(service)
    val name = serviceName(service)
    val url = rememberTextFieldState(saved.url)
    val key = rememberTextFieldState("")
    val typedUrl = url.text.toString()
    val typedKey = key.text.toString().trim()
    val address = arrAddress(typedUrl)
    val moved = address != arrAddress(saved.url)
    val needsKey = arrNeedsKey(typedUrl, typedKey, saved.url, saved.keySet)
    // An answer about what the fields held when Connect was pressed, while they still hold that.
    val fresh = trial?.takeIf { it.url == address && it.key == typedKey }
    val colors = OutlinedTextFieldDefaults.colors(
        focusedBorderColor = PleyColors.AccentBright,
        unfocusedBorderColor = PleyColors.Control,
        focusedLabelColor = PleyColors.AccentBright,
        unfocusedLabelColor = PleyColors.Muted,
        cursorColor = PleyColors.AccentBright,
        focusedSupportingTextColor = PleyColors.Muted,
        unfocusedSupportingTextColor = PleyColors.Muted,
        focusedPlaceholderColor = PleyColors.Subtle,
        unfocusedPlaceholderColor = PleyColors.Subtle,
    )
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            TextButton(
                onClick = { onSave(typedUrl, typedKey) },
                enabled = !needsKey,
                colors = ButtonDefaults.textButtonColors(contentColor = PleyColors.AccentBright),
                modifier = Modifier.testTag(CONNECTION_SAVE),
            ) { Text(stringResource(R.string.admin_save), style = MaterialTheme.typography.labelLarge) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, colors = ButtonDefaults.textButtonColors(contentColor = PleyColors.Fg)) {
                Text(stringResource(R.string.admin_cancel), style = MaterialTheme.typography.labelLarge)
            }
        },
        title = { Text(name, style = MaterialTheme.typography.headlineSmall) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    state = url,
                    modifier = Modifier.fillMaxWidth().testTag(CONNECTION_URL),
                    label = { Text(stringResource(R.string.admin_requests_address)) },
                    placeholder = { Text(example(service)) },
                    lineLimits = TextFieldLineLimits.SingleLine,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                    shape = PleyShape.Field,
                    colors = colors,
                )
                if (saved.url.isNotEmpty() && address.isEmpty() && saved.keySet) {
                    NoteLine(RowNote(stringResource(R.string.admin_requests_forgets_key)))
                }
                OutlinedSecureTextField(
                    state = key,
                    modifier = Modifier.fillMaxWidth().testTag(CONNECTION_KEY),
                    label = { Text(stringResource(R.string.admin_requests_api_key)) },
                    placeholder = {
                        Text(stringResource(if (saved.keySet && !moved) R.string.admin_requests_key_saved else R.string.admin_requests_not_set))
                    },
                    supportingText = { Text(stringResource(R.string.admin_requests_api_key_hint, name)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    shape = PleyShape.Field,
                    colors = colors,
                )
                if (needsKey) NoteLine(RowNote(stringResource(R.string.admin_requests_type_key, name), notice = true))
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    TextButton(
                        onClick = { onConnect(typedUrl, typedKey) },
                        enabled = address.isNotEmpty() && !needsKey && fresh?.answer != ArrAnswer.Asking,
                        colors = ButtonDefaults.textButtonColors(contentColor = PleyColors.AccentBright),
                    ) { Text(stringResource(R.string.admin_requests_connect), style = MaterialTheme.typography.labelLarge) }
                    val resources = LocalResources.current
                    when (val answer = fresh?.answer) {
                        ArrAnswer.Asking -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Spinner()
                            Text(stringResource(R.string.admin_requests_connecting, name), style = MaterialTheme.typography.bodySmall, color = PleyColors.Muted)
                        }
                        is ArrAnswer.Ready -> Text("$name ${answer.offer.version}", style = MaterialTheme.typography.bodySmall, color = PleyColors.Success)
                        is ArrAnswer.Failed -> Text(errorSentence(answer.error).resolve(resources), style = MaterialTheme.typography.bodySmall, color = PleyColors.DangerText)
                        else -> Unit
                    }
                }
            }
        },
        shape = PleyShape.Dialog,
        containerColor = PleyColors.Surface2,
        titleContentColor = PleyColors.Fg,
        textContentColor = PleyColors.Muted,
        tonalElevation = 0.dp,
    )
}

@Composable
private fun Spinner() {
    CircularProgressIndicator(Modifier.size(20.dp), color = PleyColors.Accent, trackColor = PleyColors.Surface4, strokeWidth = 2.dp)
}

internal const val CONNECTION_URL = "pley.admin.requests.url"
internal const val CONNECTION_KEY = "pley.admin.requests.key"
internal const val CONNECTION_SAVE = "pley.admin.requests.save"
