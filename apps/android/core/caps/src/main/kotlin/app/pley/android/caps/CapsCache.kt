package app.pley.android.caps

import app.pley.android.model.PleyJson
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import java.io.File
import java.io.IOException
import java.security.MessageDigest

/**
 * The last probe, kept as JSON so the app knows what the device plays
 * from its first playback on. It is kept for one key: the system build
 * (Build.FINGERPRINT), the app's install, the screen's modes and HDR, the
 * audio output, the quirks and the person's settings. Any of them changing
 * means a new probe.
 */
class CapsCache(private val file: File) {
    /** A saved probe and the key it was saved for. */
    @Serializable
    data class Entry(val key: String, val snapshot: CapsSnapshot) {
        /** The decoders it read, if it was on this system build: they don't change with the screen or output. */
        fun decoders(fingerprint: String): List<DecoderInfo>? =
            snapshot.details.input?.takeIf { it.device.fingerprint == fingerprint }?.decoders
    }

    /** Whether a probe is saved at all. */
    val exists: Boolean get() = file.isFile

    /** The saved probe; `null` when there is none or it can't be read. */
    fun load(): Entry? {
        if (!file.isFile) return null
        return try {
            PleyJson.decodeFromString(Entry.serializer(), file.readText())
        } catch (_: IOException) {
            null
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
        }
    }

    fun write(key: String, snapshot: CapsSnapshot) {
        try {
            file.parentFile?.mkdirs()
            val temp = File(file.parentFile, file.name + ".tmp")
            temp.writeText(PleyJson.encodeToString(Entry.serializer(), Entry(key, snapshot)))
            if (!temp.renameTo(file)) {
                file.delete()
                temp.renameTo(file)
            }
        } catch (_: IOException) {
            // A cache: the next start probes again.
        }
    }

    companion object {
        /** Changes when the rules that build a snapshot do, so an update never reads an old one. */
        const val FORMAT = 1

        /**
         * The key of a probe with these inputs. Data classes print the same
         * way in every process (enums by name), so their text is the key,
         * without the cost of JSON on the probe's path.
         */
        fun key(
            device: DeviceFacts,
            installedAt: Long,
            display: DisplayState?,
            audio: AudioState,
            quirks: QuirkTable,
            settings: CapsSettings,
        ): String {
            val parts = listOf(FORMAT, device, installedAt, display, audio, quirks, settings).map { it.toString() }
            val digest = MessageDigest.getInstance("SHA-256").digest(parts.joinToString("\n").toByteArray())
            return digest.joinToString("") { "%02x".format(it) }
        }
    }
}
