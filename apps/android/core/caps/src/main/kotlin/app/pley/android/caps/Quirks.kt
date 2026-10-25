package app.pley.android.caps

import android.content.res.Resources
import app.pley.android.model.DynamicRange
import app.pley.android.model.PleyJson
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException

/**
 * What known models get wrong about themselves, and how to correct it. The
 * app ships one (res/raw/quirks.json); a server may send its own
 * later, which wins quirk by quirk ([overriddenBy]). A quirk can add a
 * capability a device doesn't list (Dolby Vision profile 7 on the Shield)
 * as well as remove one it lists but doesn't deliver.
 */
@Serializable
data class QuirkTable(
    val version: Int = 1,
    val quirks: List<Quirk> = emptyList(),
) {
    /** The quirks of [device], in table order. */
    fun matching(device: DeviceFacts): List<Quirk> = quirks.filter { !it.disabled && it.matches(device) }

    /**
     * This table with [other]'s quirks laid over it: a quirk with the same
     * id replaces this one's (`"disabled": true` takes it away), a new id
     * is added.
     */
    fun overriddenBy(other: QuirkTable): QuirkTable {
        val replaced = other.quirks.associateBy { it.id }
        val kept = quirks.map { replaced[it.id] ?: it }
        val added = other.quirks.filter { new -> quirks.none { it.id == new.id } }
        return QuirkTable(version = maxOf(version, other.version), quirks = kept + added)
    }

    companion object {
        /**
         * The table the app ships with, res/raw/quirks.json, read with
         * Android's org.json: the first kotlinx.serialization read of these
         * classes costs a cold probe about 100 ms on an emulator, more than
         * the decoders. A test checks both readers agree on the file.
         */
        fun load(resources: Resources): QuirkTable =
            QuirkTableReader.read(resources.openRawResource(R.raw.quirks).bufferedReader().use { it.readText() })

        /** A table in the same JSON (a server's, or the file in tests). */
        fun parse(json: String): QuirkTable = PleyJson.decodeFromString(serializer(), json)

        /** A server's table; `null` when it can't be read, so the shipped one stays. */
        fun parseOrNull(json: String): QuirkTable? = try {
            parse(json)
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
        }
    }
}

/**
 * One correction for some models: they are named by Build.MODEL
 * (exactly, ignoring case), or by Build.MANUFACTURER when a whole maker's
 * devices share the fault.
 */
@Serializable
data class Quirk(
    /** Stable, so a server's table can replace or switch off this quirk. */
    val id: String,
    /** Why, for Diagnostics and the next person to read the table. */
    val why: String = "",
    val models: List<String> = emptyList(),
    val manufacturers: List<String> = emptyList(),
    /** Only for TVs (a maker's phones may be fine). */
    val tvOnly: Boolean = false,
    /** A server's way to switch a shipped quirk off. */
    val disabled: Boolean = false,
    val add: CapsChange = CapsChange(),
    val remove: CapsChange = CapsChange(),
    val flags: QuirkFlags = QuirkFlags(),
) {
    fun matches(device: DeviceFacts): Boolean {
        if (tvOnly && !device.isTv) return false
        return models.any { it.equals(device.model, ignoreCase = true) } ||
            manufacturers.any { it.equals(device.manufacturer, ignoreCase = true) }
    }
}

/** Capabilities a quirk adds or removes. */
@Serializable
data class CapsChange(
    /** Dolby Vision profile numbers: 5, 7, 8… */
    val dolbyVisionProfiles: List<Int> = emptyList(),
    val passthrough: List<PassthroughFormat> = emptyList(),
    /** The screen's HDR types. */
    val hdrTypes: List<DynamicRange> = emptyList(),
    /** Video codecs, by the server's names ("av1"): decoders that list a codec they can't play. */
    val videoCodecs: List<String> = emptyList(),
)

/**
 * Switches for the player, per model. `null` leaves the app's default:
 * Fire TV's forced async queueing on, tunneling off, Dolby Vision mode
 * switching below Android 14 off.
 */
@Serializable
data class QuirkFlags(
    /** Dolby Vision streams that also carry HDR10+ fail here: the server strips the HDR10+. */
    val dvHdr10PlusHybridsBroken: Boolean? = null,
    /** `false`: don't force async MediaCodec queueing (a kill switch for Fire TV's default). */
    val asyncQueueing: Boolean? = null,
    /** `true`: tunneled playback allowed (an allowlist, empty until device tests). */
    val tunneling: Boolean? = null,
    /** `true`: switching the screen's mode for Dolby Vision below Android 14 is safe here. */
    val dvModeSwitch: Boolean? = null,
)
