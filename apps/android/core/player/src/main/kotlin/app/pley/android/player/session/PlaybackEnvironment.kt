package app.pley.android.player.session

import android.annotation.SuppressLint
import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.SystemClock
import app.pley.android.caps.DeviceCapabilities
import app.pley.android.caps.PlaybackCapsPolicy
import app.pley.android.data.AppSettings
import app.pley.android.data.ItemRepository
import app.pley.android.data.Loadable
import app.pley.android.model.ItemDetail
import app.pley.android.network.PleyApi
import app.pley.android.player.subtitles.Media3VttParser
import app.pley.android.player.subtitles.SubtitleFetcher
import app.pley.android.player.subtitles.VttParser
import app.pley.android.player.trickplay.RegionSheetDecoder
import app.pley.android.player.trickplay.SheetDecoder
import app.pley.android.player.trickplay.SheetSource
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first

/** An item's details for playing: what was saved first, else the server's. */
fun interface ItemDetails {
    suspend fun detail(itemId: Long): ItemDetail

    companion object {
        /** From the server only. */
        fun of(api: PleyApi): ItemDetails = ItemDetails { api.item(it) }

        /**
         * The repository's first answer (its snapshot, so playing starts
         * without waiting on the network), else the server's.
         */
        fun snapshotFirst(items: ItemRepository, api: PleyApi): ItemDetails = ItemDetails { id ->
            val first = try {
                items.detail(id).first { it !is Loadable.Loading }
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
                null
            }
            when (first) {
                is Loadable.Ready -> first.value
                is Loadable.Failed -> first.stale ?: api.item(id)
                else -> api.item(id)
            }
        }
    }
}

/** Whether the device is on its home network (Wi-Fi, Ethernet) or on mobile data. */
enum class NetworkKind { Home, Mobile }

/** The quality chosen last, per device and network kind (Settings.qualityOnHomeNetwork, qualityOnMobile). */
interface QualityMemory {
    suspend fun remembered(): String?

    suspend fun remember(id: String)

    companion object {
        /** Remembers nothing: always the original. */
        val NONE = object : QualityMemory {
            override suspend fun remembered(): String? = null

            override suspend fun remember(id: String) = Unit
        }

        /** In the app's settings, one per network kind. */
        fun of(settings: AppSettings, network: () -> NetworkKind): QualityMemory = object : QualityMemory {
            override suspend fun remembered(): String? {
                val s = settings.settings.first()
                return if (network() == NetworkKind.Mobile) s.qualityOnMobile else s.qualityOnHomeNetwork
            }

            override suspend fun remember(id: String) {
                val mobile = network() == NetworkKind.Mobile
                try {
                    settings.update { if (mobile) it.copy(qualityOnMobile = id) else it.copy(qualityOnHomeNetwork = id) }
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    // Not saved: the next video plays the original.
                }
            }
        }
    }
}

/**
 * Android's view of the network: mobile data when the default network is
 * cellular. ACCESS_NETWORK_STATE is :core:network's (its manifest declares it).
 */
@SuppressLint("MissingPermission")
fun networkKindOf(context: Context): () -> NetworkKind {
    val connectivity = context.applicationContext.getSystemService(ConnectivityManager::class.java)
    return {
        val caps = connectivity?.activeNetwork?.let { connectivity.getNetworkCapabilities(it) }
        if (caps?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true) NetworkKind.Mobile else NetworkKind.Home
    }
}

/**
 * This installation, as the server tells players apart (starting a session
 * ends the earlier ones of the same client). The old player's id is kept:
 * it lives in the same place (the "device" preferences' "client_id").
 */
fun clientIdOf(context: Context): () -> String {
    val app = context.applicationContext
    val cached = lazy {
        val prefs = app.getSharedPreferences("device", Context.MODE_PRIVATE)
        prefs.getString("client_id", null) ?: "android-${UUID.randomUUID()}".also { prefs.edit().putString("client_id", it).apply() }
    }
    return { cached.value }
}

/**
 * Everything a playback session uses, so tests can give fakes: the server,
 * the device's capabilities, the engine, settings and the clock.
 */
class PlaybackEnvironment(
    val api: PleyApi,
    val capabilities: DeviceCapabilities,
    val policy: PlaybackCapsPolicy,
    val details: ItemDetails,
    /** Makes the engine link for a playback (on the main thread). */
    val engines: () -> EngineLink,
    val texts: PlayerTexts,
    val clientId: () -> String,
    val reporter: SessionReporter,
    val subtitles: SubtitleFetcher,
    val quality: QualityMemory = QualityMemory.NONE,
    /** Seek previews' sheets; `null` for none. */
    val sheets: SheetSource? = null,
    val sheetDecoder: SheetDecoder = RegionSheetDecoder,
    /** Whether what comes next starts by itself after its countdown (Settings.autoplayNext). */
    val autoplayNext: suspend () -> Boolean = { true },
    /** The smallest TVs: fewer tiles, in RGB_565. */
    val lowMemory: Boolean = false,
    val parser: VttParser = Media3VttParser,
    /** A monotonic clock in ms, for stalls and dropped frames. */
    val now: () -> Long = SystemClock::elapsedRealtime,
    /** Where subtitles are parsed and seek previews cut, off the main thread. */
    val work: CoroutineDispatcher = Dispatchers.Default,
)
