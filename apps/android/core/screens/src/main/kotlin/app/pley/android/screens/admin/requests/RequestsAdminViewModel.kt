package app.pley.android.screens.admin.requests

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.ArrService
import app.pley.android.model.admin.ArrOffer
import app.pley.android.model.admin.RadarrAvailability
import app.pley.android.model.admin.RequestsSettings
import app.pley.android.screens.admin.server.SettingsPageViewModel
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonObjectBuilder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.net.URI

/** What Radarr or Sonarr answered: nothing asked yet, on its way, what it offers, or why it couldn't. */
sealed interface ArrAnswer {
    data object None : ArrAnswer

    data object Asking : ArrAnswer

    data class Ready(val offer: ArrOffer) : ArrAnswer

    data class Failed(val error: Throwable) : ArrAnswer
}

/** Connect pressed with [url] and [key] typed in (not saved), and what came of it. */
data class ArrTrial(val url: String, val key: String, val answer: ArrAnswer)

/**
 * Settings → Requests (apps/web/src/pages/settings/RequestsSettings.tsx):
 * taking requests and how many each account may make; with a [service],
 * that service's page: its address and key (sent together, as the server
 * takes a new address only with its key), Connect to try them before
 * saving, and what requests are sent with, from what it offers.
 */
class RequestsAdminViewModel(
    admin: ServerAdmin,
    val service: ArrService?,
) : SettingsPageViewModel<RequestsSettings>(admin, RequestsSettings.serializer()) {
    private val offerNow = MutableStateFlow<ArrAnswer>(ArrAnswer.None)

    /** What the saved address offers, asked once the settings say there is one with a key. */
    val offer: StateFlow<ArrAnswer> = offerNow.asStateFlow()

    private val trialNow = MutableStateFlow<ArrTrial?>(null)

    /** The last Connect. */
    val trial: StateFlow<ArrTrial?> = trialNow.asStateFlow()

    private var asking: Job? = null

    init {
        if (service != null) {
            // The saved address and whether a key goes with it: asked when they change.
            viewModelScope.launch {
                settings.state
                    .map { it.valueOrNull?.of(service)?.let { saved -> saved.url to saved.keySet } }
                    .filterNotNull()
                    .distinctUntilChanged()
                    .collect { (url, keySet) -> askSaved(url, keySet) }
            }
        }
    }

    private fun askSaved(url: String, keySet: Boolean) {
        val service = service ?: return
        asking?.cancel()
        if (url.isEmpty() || !keySet) {
            offerNow.value = ArrAnswer.None
            return
        }
        offerNow.value = ArrAnswer.Asking
        asking = viewModelScope.launch { offerNow.value = ask(buildJsonObject { put("service", service.wire) }) }
    }

    private suspend fun ask(body: JsonObject): ArrAnswer = try {
        ArrAnswer.Ready(admin.api().send("POST", ArrOffer.serializer(), listOf("api", "settings", "arr"), body))
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        ArrAnswer.Failed(e)
    }

    /** Asks the saved service again (Retry, or a new key for the same address). */
    fun askAgain() {
        val service = service ?: return
        settings.state.value.valueOrNull?.of(service)?.let { askSaved(it.url, it.keySet) }
    }

    fun setRequests(on: Boolean) = save(buildJsonObject { put("requests", on) })

    /** 0 (no limit) to 1000. */
    fun setMovieLimit(limit: Int) = save(buildJsonObject { put("request_limit_movies", limit) })

    fun setSeasonLimit(limit: Int) = save(buildJsonObject { put("request_limit_seasons", limit) })

    /** Tries [url] with [key] (empty: the saved key, which goes only to the saved address). */
    fun connect(url: String, key: String): Job = viewModelScope.launch {
        val service = service ?: return@launch
        val address = arrAddress(url)
        trialNow.value = ArrTrial(address, key, ArrAnswer.Asking)
        val answer = ask(
            buildJsonObject {
                put("service", service.wire)
                put("url", address)
                if (key.isNotEmpty()) put("api_key", key)
            },
        )
        trialNow.value = ArrTrial(address, key, answer)
    }

    /**
     * Saves the address and key together; a key left empty keeps the saved
     * one (for the same address), and no address forgets it. Shown once the
     * server took them, so what is asked is what it saved.
     */
    fun saveConnection(url: String, key: String): Job {
        val service = onePage()
        val wire = service.wire
        val address = arrAddress(url)
        val sameAddress = settings.state.value.valueOrNull?.of(service)?.url == address
        return save(
            buildJsonObject {
                put("${wire}_url", address)
                if (key.isNotEmpty()) put("${wire}_api_key", key)
            },
            shownFirst = false,
            // Another address is asked as the settings show it; a new key for the same one isn't seen there.
            then = { if (sameAddress && key.isNotEmpty()) askAgain() },
        )
    }

    /** Radarr's or Sonarr's id; 0: none chosen. */
    fun setQualityProfile(id: Long) = saveArr("quality_profile") { put(it, id) }

    /** Empty: none chosen. */
    fun setRootFolder(path: String) = saveArr("root_folder") { put(it, path) }

    /** 0: the quality profile above. */
    fun setAnimeQualityProfile(id: Long) = saveArr("anime_quality_profile") { put(it, id) }

    /** Empty: the root folder above. */
    fun setAnimeRootFolder(path: String) = saveArr("anime_root_folder") { put(it, path) }

    fun setSearch(on: Boolean) = saveArr("search") { put(it, on) }

    fun setMinimumAvailability(availability: RadarrAvailability) = save(buildJsonObject { put("radarr_minimum_availability", availability.wire) })

    /** Sonarr 3's; 0: its first. */
    fun setLanguageProfile(id: Long) = save(buildJsonObject { put("sonarr_language_profile", id) })

    fun setSeasonFolders(on: Boolean) = save(buildJsonObject { put("sonarr_season_folders", on) })

    /** This service's [field] ("quality_profile" is "radarr_quality_profile" on Radarr's page). */
    private fun saveArr(field: String, value: JsonObjectBuilder.(String) -> Unit): Job =
        save(buildJsonObject { value("${onePage().wire}_$field") })

    /** The service of this page: its connection and choices are only ever changed there. */
    private fun onePage(): ArrService = checkNotNull(service) { "only a service's page changes it" }

    companion object {
        fun factory(service: ArrService?): ViewModelProvider.Factory = viewModelFactory {
            initializer { RequestsAdminViewModel(screensGraph().data.admin, service) }
        }
    }
}

/**
 * An address as the server keeps it: trimmed, as URLs are written
 * ("HTTP://Host/" is "http://host"), less a trailing `/`. What isn't one is
 * left as typed, for the server to say why.
 */
fun arrAddress(url: String): String {
    val value = url.trim()
    if (value.isEmpty()) return ""
    val written = try {
        val uri = URI(value)
        if (uri.scheme == null || uri.host == null) {
            value
        } else {
            URI(uri.scheme.lowercase(), uri.userInfo, uri.host.lowercase(), uri.port, uri.path, uri.query, uri.fragment).toString()
        }
    } catch (_: Exception) {
        value
    }
    return written.trimEnd('/')
}

/**
 * Whether Save needs a key typed: the saved one goes only to the address it
 * was saved with, so another address needs its own (the server won't save
 * it without).
 */
fun arrNeedsKey(typedUrl: String, typedKey: String, savedUrl: String, keySet: Boolean): Boolean {
    val address = arrAddress(typedUrl)
    return address.isNotEmpty() && address != arrAddress(savedUrl) && keySet && typedKey.isEmpty()
}
