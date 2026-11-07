package app.pley.android.ui.phone.admin.requests

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.ArrService
import app.pley.android.model.admin.RequestsSettings
import app.pley.android.screens.PleyKey
import app.pley.android.screens.admin.AdminSection
import app.pley.android.screens.admin.requests.RequestsAdminViewModel
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.Entry
import app.pley.android.ui.phone.admin.TextEntryDialog
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.admin.numberProblem
import app.pley.android.ui.phone.admin.server.GroupLine
import app.pley.android.ui.phone.admin.server.RowNote
import app.pley.android.ui.phone.admin.server.ToggleRow
import app.pley.android.ui.phone.admin.server.ValueRow
import app.pley.android.ui.phone.kit.EmptyState
import app.pley.android.ui.phone.shell.LocalPhoneNavigator

/** The most requests of each kind an account may be allowed in 7 days; 0 is no limit. */
private val LIMITS = 0L..1000L

/**
 * Settings → Requests on a phone: taking requests, how many each account
 * may make, and Radarr and Sonarr, each on its own page ([id]: "radarr",
 * "sonarr").
 */
@Composable
internal fun RequestsAdminScreen(id: String?) {
    val service = ArrService.entries.firstOrNull { it != ArrService.Unknown && it.wire == id }
    if (service != null) ArrScreen(service) else RequestsPage()
}

/** Which limit is being typed in. */
private enum class Limit { Movies, Seasons }

@Composable
private fun RequestsPage() {
    val model: RequestsAdminViewModel = viewModel(key = "admin-requests", factory = RequestsAdminViewModel.factory(null))
    val load by model.settings.state.collectAsStateWithLifecycle()
    val navigator = LocalPhoneNavigator.current
    var typing by rememberSaveable { mutableStateOf<Limit?>(null) }
    AdminEvents(model)
    AdminPage(
        title = stringResource(R.string.admin_requests_title),
        load = load,
        onRetry = model.settings::reload,
        tag = REQUESTS_PAGE,
    ) { s ->
        val on = s.requests
        if (on == null) {
            item(key = "unsupported") {
                EmptyState(
                    title = stringResource(R.string.admin_requests_unsupported),
                    body = stringResource(R.string.admin_requests_unsupported_hint),
                    icon = PleyIcons.Compass,
                )
            }
            return@AdminPage
        }
        item(key = "requests") {
            val hasTmdb = s.tmdbApiKey.isNotBlank()
            SettingsGroup(
                null,
                Modifier.adminWidth,
                footer = stringResource(R.string.admin_requests_who_may_ask),
            ) {
                GroupLine(stringResource(R.string.admin_requests_description))
                ToggleRow(
                    stringResource(R.string.admin_requests_take),
                    checked = on,
                    onChange = model::setRequests,
                    // Titles are found on TMDB: without a key, requests can only go off.
                    enabled = hasTmdb || on,
                    hint = stringResource(R.string.admin_requests_take_hint),
                    note = if (hasTmdb) {
                        null
                    } else {
                        RowNote(
                            stringResource(R.string.admin_requests_needs_tmdb),
                            notice = on,
                            onClick = { navigator.open(PleyKey.Manage(AdminSection.Server)) },
                        )
                    },
                )
                RowDivider(inset = false)
                ValueRow(
                    stringResource(R.string.admin_requests_movies),
                    limitLabel(s.requestLimitMovies, R.plurals.admin_requests_movie_count),
                    hint = stringResource(R.string.admin_requests_movies_hint),
                    onClick = { typing = Limit.Movies },
                )
                RowDivider(inset = false)
                ValueRow(
                    stringResource(R.string.admin_requests_seasons),
                    limitLabel(s.requestLimitSeasons, R.plurals.admin_requests_season_count),
                    hint = stringResource(R.string.admin_requests_seasons_hint),
                    onClick = { typing = Limit.Seasons },
                )
            }
        }
        item(key = "services") {
            SettingsGroup(null, Modifier.adminWidth) {
                for ((i, svc) in listOf(ArrService.Radarr, ArrService.Sonarr).withIndex()) {
                    if (i > 0) RowDivider(inset = false)
                    ServiceRow(svc, s) { navigator.open(PleyKey.Manage(AdminSection.Requests, svc.wire)) }
                }
            }
        }
    }

    val s = load.valueOrNull ?: return
    when (typing) {
        Limit.Movies -> TextEntryDialog(
            title = stringResource(R.string.admin_requests_movies),
            initial = s.requestLimitMovies.toString(),
            hint = stringResource(R.string.admin_requests_movies_hint),
            entry = Entry.Number,
            check = numberProblem(LIMITS),
            onSave = { model.setMovieLimit(it.toInt()); typing = null },
            onDismiss = { typing = null },
        )
        Limit.Seasons -> TextEntryDialog(
            title = stringResource(R.string.admin_requests_seasons),
            initial = s.requestLimitSeasons.toString(),
            hint = stringResource(R.string.admin_requests_seasons_hint),
            entry = Entry.Number,
            check = numberProblem(LIMITS),
            onSave = { model.setSeasonLimit(it.toInt()); typing = null },
            onDismiss = { typing = null },
        )
        null -> Unit
    }
}

/** Radarr or Sonarr on the page of requests: where it is, and whether approving can work. */
@Composable
private fun ServiceRow(service: ArrService, s: RequestsSettings, onClick: () -> Unit) {
    val saved = s.of(service)
    val incomplete = saved.url.isNotEmpty() && (saved.qualityProfile == 0L || saved.rootFolder.isEmpty())
    ValueRow(
        serviceName(service),
        saved.url.ifEmpty { stringResource(R.string.admin_requests_not_set) },
        hint = stringResource(if (service == ArrService.Sonarr) R.string.admin_requests_sonarr_description else R.string.admin_requests_radarr_description),
        note = if (incomplete) RowNote(stringResource(R.string.admin_requests_approving_fails), notice = true) else null,
        onClick = onClick,
    )
}

/** "No limit", "5 movies". */
@Composable
private fun limitLabel(limit: Int, unit: Int): String =
    if (limit == 0) stringResource(R.string.admin_requests_no_limit) else pluralStringResource(unit, limit, limit)

internal fun serviceName(service: ArrService): String = if (service == ArrService.Sonarr) "Sonarr" else "Radarr"

internal const val REQUESTS_PAGE = "pley.admin.requests"
