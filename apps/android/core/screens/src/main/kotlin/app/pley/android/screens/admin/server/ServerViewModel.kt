package app.pley.android.screens.admin.server

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.Me
import app.pley.android.model.admin.DatabaseBackup
import app.pley.android.model.admin.LibraryWorkMode
import app.pley.android.model.admin.LibraryWorkWeek
import app.pley.android.model.admin.DatabaseBackups
import app.pley.android.model.admin.ServerSettings
import app.pley.android.model.admin.ServerSystem
import app.pley.android.model.admin.ServiceTestResult
import app.pley.android.model.admin.SignInAccount
import app.pley.android.model.admin.TestedService
import app.pley.android.model.admin.TvMetadataSource
import app.pley.android.screens.admin.AdminResource
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** How a test of a service went: on its way, the server's answer, or no answer from the server. */
sealed interface ServiceTestState {
    data object Testing : ServiceTestState

    data class Done(val result: ServiceTestResult) : ServiceTestState

    data class Failed(val error: Throwable) : ServiceTestState
}

/**
 * Settings → Server (apps/web/src/pages/settings/ServerSettings.tsx): the
 * server's name and language, metadata sources and their keys, subtitles,
 * library scanning and when library work runs, sign-in, apps by other
 * publishers, and the database's backups. Each value is sent as it is
 * chosen. Keys are tested as saved: the phone saves a key as it is typed in.
 */
class ServerViewModel(admin: ServerAdmin) : SettingsPageViewModel<ServerSettings>(admin, ServerSettings.serializer()) {
    /** What the transcoder can do (intros by fingerprint) and whether two-step sign-in can be checked. */
    val system: AdminResource<ServerSystem> = resource { get(ServerSystem.serializer(), listOf("api", "system")) }

    /** Who manages: requiring two-step sign-in is for an administrator who uses it. */
    val me: AdminResource<Me> = resource { get(Me.serializer(), listOf("api", "auth", "me")) }

    /** The accounts, to say how many requiring two-step sign-in signs out. */
    val accounts: AdminResource<List<SignInAccount>> = resource { get(ListSerializer(SignInAccount.serializer()), listOf("api", "users")) }

    /** When people watch, as learned, for library work's Automatically. */
    val week: AdminResource<LibraryWorkWeek> = resource { get(LibraryWorkWeek.serializer(), listOf("api", "system", "library-work")) }

    val backups: AdminResource<DatabaseBackups> = resource { get(DatabaseBackups.serializer(), BACKUPS) }

    private val testsNow = MutableStateFlow<Map<TestedService, ServiceTestState>>(emptyMap())

    /** The last test of each service since its key changed. */
    val tests: StateFlow<Map<TestedService, ServiceTestState>> = testsNow.asStateFlow()

    private val backedUpNow = Channel<DatabaseBackup>(Channel.BUFFERED)

    /** Backups made from this page, for a toast with their size. */
    val backedUp: Flow<DatabaseBackup> = backedUpNow.receiveAsFlow()

    fun rename(name: String) = save(buildJsonObject { put("server_name", name) })

    /** "en", "fr", "es". */
    fun setLanguage(code: String) = save(buildJsonObject { put("language", code) })

    /** A source can't stay chosen once its key is gone: Automatic takes over, as on the web. */
    fun setTmdbKey(key: String) = setSourceKey("tmdb_api_key", key, TvMetadataSource.Tmdb, TestedService.Tmdb)

    fun setTvdbKey(key: String) = setSourceKey("tvdb_api_key", key, TvMetadataSource.Tvdb, TestedService.Tvdb)

    private fun setSourceKey(field: String, key: String, source: TvMetadataSource, service: TestedService): Job {
        val orphaned = key.isBlank() && settings.state.value.valueOrNull?.tvMetadata == source
        forget(service)
        return save(
            buildJsonObject {
                put(field, key)
                if (orphaned) put("tv_metadata", TvMetadataSource.Auto.wire)
            },
        )
    }

    fun setTvdbPin(pin: String): Job {
        forget(TestedService.Tvdb)
        return save(buildJsonObject { put("tvdb_pin", pin) })
    }

    fun setTvSource(source: TvMetadataSource) = save(buildJsonObject { put("tv_metadata", source.wire) })

    fun setMusicMetadata(on: Boolean) = save(buildJsonObject { put("music_metadata", on) })

    fun setReadNfo(on: Boolean) = save(buildJsonObject { put("read_nfo", on) })

    /** "en-US". */
    fun setMetadataLanguage(code: String) = save(buildJsonObject { put("metadata_language", code) })

    fun setOpenSubtitlesKey(key: String): Job {
        forget(TestedService.Opensubtitles)
        return save(buildJsonObject { put("opensubtitles_api_key", key) })
    }

    /** Another account's password isn't this one's: the server forgets the saved one. */
    fun setOpenSubtitlesUsername(username: String): Job {
        forget(TestedService.Opensubtitles)
        return save(buildJsonObject { put("opensubtitles_username", username) })
    }

    /** Left empty, the saved password stays, as on the web. */
    fun setOpenSubtitlesPassword(password: String): Job? {
        if (password.isEmpty()) return null
        forget(TestedService.Opensubtitles)
        return save(buildJsonObject { put("opensubtitles_password", password) })
    }

    fun setWatchLibraries(on: Boolean) = save(buildJsonObject { put("watch_libraries", on) })

    fun setDetectMarkers(on: Boolean) = saveWork(buildJsonObject { put("detect_markers", on) })

    fun setSeekPreviews(on: Boolean) = saveWork(buildJsonObject { put("seek_previews", on) })

    fun setPreviewsInBackground(on: Boolean) = saveWork(buildJsonObject { put("previews_in_background", on) })

    fun setLibraryWork(mode: LibraryWorkMode) = saveWork(buildJsonObject { put("library_work", mode.wire) })

    /** The hours of Between set hours, 0 to 23 in the server's time. */
    fun setLibraryWorkFrom(hour: Int) = saveWork(buildJsonObject { put("library_work_from", hour) })

    fun setLibraryWorkTo(hour: Int) = saveWork(buildJsonObject { put("library_work_to", hour) })

    /** Percent, 10 to 100 (never waits). */
    fun setLibraryWorkMaxCpu(percent: Int) = saveWork(buildJsonObject { put("library_work_max_cpu", percent) })

    /** Minutes; 0: no periodic scans. */
    fun setScanInterval(minutes: Int) = save(buildJsonObject { put("scan_interval_minutes", minutes) })

    /** Turned on, it signs out everyone without it: the accounts are read again to say so. */
    fun setRequireTwoStep(on: Boolean) = save(buildJsonObject { put("require_two_step", on) }, then = accounts::reload)

    fun setThirdPartyApps(on: Boolean) = save(buildJsonObject { put("third_party_apps", on) })

    /** The hours library work may begin in follow its settings. */
    private fun saveWork(body: JsonObject) = save(body, then = week::reload)

    /** Tests [service] with what is saved, as the server reads it when no key is given. */
    fun test(service: TestedService): Job = viewModelScope.launch {
        testsNow.update { it + (service to ServiceTestState.Testing) }
        val state = try {
            val body = buildJsonObject { put("service", service.wire) }
            ServiceTestState.Done(admin.api().send("POST", ServiceTestResult.serializer(), listOf("api", "settings", "test"), body))
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            ServiceTestState.Failed(e)
        }
        testsNow.update { it + (service to state) }
    }

    /** An answer about another key doesn't linger. */
    private fun forget(service: TestedService) = testsNow.update { it - service }

    /** Backs the database up now; the list is read again either way. */
    fun backUp(): Job = change(after = backups::reload) {
        backedUpNow.send(send("POST", DatabaseBackup.serializer(), BACKUPS))
    }

    /** Reads the week again every minute while the chart shows, as the web does; cancel to stop. */
    suspend fun followWeek(): Nothing {
        while (true) {
            delay(WEEK_EVERY_MS)
            week.reload()
        }
    }

    companion object {
        private val BACKUPS: List<Any> = listOf("api", "system", "backups")

        const val WEEK_EVERY_MS = 60_000L

        val factory: ViewModelProvider.Factory = viewModelFactory {
            initializer { ServerViewModel(screensGraph().data.admin) }
        }
    }
}

/** The source Automatic stands for with these keys: the server decides the same way. */
fun automaticTvSource(settings: ServerSettings): TvMetadataSource = when {
    settings.tmdbApiKey.isNotBlank() -> TvMetadataSource.Tmdb
    settings.tvdbApiKey.isNotBlank() -> TvMetadataSource.Tvdb
    else -> TvMetadataSource.Tvmaze
}

/** Library work has something to do: intros and credits, or previews of whole libraries. */
val ServerSettings.libraryWorkToDo: Boolean get() = detectMarkers || (seekPreviews && previewsInBackground)

/** Accounts requiring two-step sign-in signs out: those with a password, without it, not disabled. */
fun withoutTwoStep(accounts: List<SignInAccount>): Int = accounts.count { it.hasPassword && !it.twoStep && !it.disabled }

/** The metadata languages offered, as the web's; one saved that isn't among them comes first. */
fun metadataLanguages(saved: String): List<String> = if (saved in METADATA_LANGUAGES) METADATA_LANGUAGES else listOf(saved) + METADATA_LANGUAGES

private val METADATA_LANGUAGES = listOf(
    "en-US", "en-GB", "fr-FR", "de-DE", "es-ES", "es-MX", "it-IT", "pt-BR", "pt-PT", "nl-NL", "sv-SE", "da-DK", "fi-FI",
    "nb-NO", "pl-PL", "cs-CZ", "ru-RU", "uk-UA", "tr-TR", "ar-SA", "he-IL", "hi-IN", "ja-JP", "ko-KR", "zh-CN", "zh-TW",
)

/** Shares of the processors other programs may keep busy before library work waits (100: never); one saved besides them is kept in order. */
fun maxCpuChoices(saved: Int): List<Int> = (MAX_CPU + saved).distinct().sorted()

private val MAX_CPU = listOf(25, 50, 75, 100)

/** The most minutes between periodic scans the server keeps: a week. */
const val MAX_SCAN_INTERVAL = 7 * 24 * 60
