package app.pley.android.caps

import android.content.Context
import app.pley.android.common.DeviceProfile
import app.pley.android.model.ItemDetail
import app.pley.android.model.caps.ClientCaps
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * What this device plays right now, for the screen and audio output in
 * use: the report the server decides from, and the lines Diagnostics shows.
 */
data class CapsSnapshot(
    /** Sent with every playback request (version 1 and 2 fields filled). */
    val report: ClientCaps,
    /** For Diagnostics: decoders, HDR types, Dolby Vision profiles, passthrough, display modes, quirks. */
    val lines: List<String>,
)

/**
 * The capability probe: hardware decoders and their limits, Dolby Vision
 * profiles, the screen's HDR types and modes, passthrough, and the quirks
 * of known models, which can add capabilities as well as remove them.
 * Probed after the first frame, never on the main thread, and again when
 * the screen or audio output changes.
 */
interface DeviceCapabilities {
    /** The latest probe; `null` until the first one ends. */
    val snapshot: StateFlow<CapsSnapshot?>

    /** Probes now (off the main thread) unless a probe for this screen and output is saved. */
    suspend fun warmUp(): CapsSnapshot
}

/** Whether a video can play as it is here, and if not, what the server will do about it. */
sealed interface CapsDecision {
    /** It plays as it is. */
    data object Direct : CapsDecision

    /** Dolby Vision this device can't decode, on an HDR10 screen: the server sends the HDR10 base layer. */
    data object RemuxBaseLayer : CapsDecision

    /** The screen can't show its HDR: the server tone-maps it. */
    data object ServerToneMaps : CapsDecision

    data class Unsupported(val reason: String) : CapsDecision
}

/**
 * The client's side of the Dolby Vision and HDR rules: what this device does
 * with each version of an item, and which version plays best here.
 */
interface PlaybackCapsPolicy {
    fun decide(item: ItemDetail, fileId: Long?, caps: CapsSnapshot): CapsDecision

    /** The file of [item] that plays best here; `null` when it has none. */
    fun bestFile(item: ItemDetail, caps: CapsSnapshot): Long?
}

/** The capability side of the app. */
interface CapsGraph {
    val capabilities: DeviceCapabilities
    val policy: PlaybackCapsPolicy

    companion object {
        /** Not built yet: the probe never ends, and the policy plays everything as it is. */
        fun create(context: Context, profile: DeviceProfile): CapsGraph = NotBuiltCaps
    }
}

private object NotBuiltCaps : CapsGraph {
    override val capabilities = object : DeviceCapabilities {
        override val snapshot: StateFlow<CapsSnapshot?> = MutableStateFlow(null)

        override suspend fun warmUp(): CapsSnapshot = CapsSnapshot(ClientCaps(), listOf("The capability probe isn't built yet"))
    }

    override val policy = object : PlaybackCapsPolicy {
        override fun decide(item: ItemDetail, fileId: Long?, caps: CapsSnapshot): CapsDecision = CapsDecision.Direct

        override fun bestFile(item: ItemDetail, caps: CapsSnapshot): Long? = null
    }
}
