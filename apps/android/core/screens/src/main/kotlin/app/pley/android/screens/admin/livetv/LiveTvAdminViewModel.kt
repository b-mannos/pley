package app.pley.android.screens.admin.livetv

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.admin.LiveSource
import app.pley.android.model.admin.TunerStatus
import app.pley.android.network.AdminApi
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.AdminViewModel
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer

/**
 * Live TV's first page, as the web's Sources tab: the sources with their
 * state and counts, and the connections in use, which are read again
 * every few seconds while the page shows ([watch]), as the web's are.
 */
class LiveTvAdminViewModel(admin: ServerAdmin) : AdminViewModel(admin) {
    val sources = resource { liveSources() }

    private val tunersNow = MutableStateFlow<List<TunerStatus>?>(null)

    /** The connections in use; `null` until first read. */
    val tuners: StateFlow<List<TunerStatus>?> = tunersNow.asStateFlow()

    /**
     * Reads the connections in use every [TUNERS_EVERY] ms, and the
     * sources every [READING_EVERY] ms while one is being read, until
     * cancelled (the page goes off screen). A read that fails keeps what
     * was shown; the next one tries again.
     */
    suspend fun watch() = coroutineScope {
        launch {
            while (true) {
                readTuners()
                delay(TUNERS_EVERY)
            }
        }
        launch {
            while (true) {
                delay(READING_EVERY)
                if (sources.state.value.valueOrNull?.any { it.refreshing } == true) sources.reload()
            }
        }
    }

    private suspend fun readTuners() {
        try {
            tunersNow.value = admin.api().get(ListSerializer(TunerStatus.serializer()), listOf("api", "live", "tuners"))
        } catch (e: CancellationException) {
            throw e
        } catch (_: Exception) {
            // The next read tries again; what was shown stays.
        }
    }

    /** Stops pulling [tuner]'s channel, for everyone watching it. */
    fun stop(tuner: TunerStatus) = change(said = AdminDone(STOPPED, listOf(tuner.channelName))) {
        call("DELETE", listOf("api", "live", "tuners", tuner.channelId))
        tunersNow.value = tunersNow.value?.filterNot { it.channelId == tuner.channelId }
        readTuners()
    }

    companion object {
        const val STOPPED = "livetv.tuner.stopped"
        const val TUNERS_EVERY = 5_000L
        const val READING_EVERY = 2_000L

        val factory: ViewModelProvider.Factory = viewModelFactory {
            initializer { LiveTvAdminViewModel(screensGraph().data.admin) }
        }
    }
}

/** GET /api/live/sources. */
internal suspend fun AdminApi.liveSources(): List<LiveSource> =
    get(ListSerializer(LiveSource.serializer()), listOf("api", "live", "sources"))
