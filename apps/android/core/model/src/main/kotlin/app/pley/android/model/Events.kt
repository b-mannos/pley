package app.pley.android.model

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.descriptors.element
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject

/** Where a library scan is (apps/server/src/events.rs, `ScanPhase`). */
@Serializable(with = ScanPhase.Serializer::class)
enum class ScanPhase(override val wire: String) : WireEnum {
    Queued("queued"),

    /** Looking through the folders. */
    Walking("walking"),

    /** Reading the new and changed files. */
    Probing("probing"),

    /** Fetching titles, artwork and people. */
    Metadata("metadata"),
    Done("done"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<ScanPhase>("ScanPhase", ScanPhase.entries, ScanPhase.Unknown)
}

/**
 * One message of GET /api/events, the server's stream of Server-Sent
 * Events: each `data:` is a JSON object whose `type` says which
 * (apps/server/src/events.rs, `ServerEvent`). Events about libraries the
 * profile can't open never come. A `type` this app doesn't know is
 * [Unknown], so a newer server can add events. [parse] reads one.
 */
@Serializable(with = ServerEvent.Serializer::class)
sealed interface ServerEvent {
    /** The `type` on the wire. */
    val type: String

    /** The library the event is about, if it is about one. */
    val libraryId: Long? get() = null

    @Serializable
    data class ScanStarted(override val libraryId: Long) : ServerEvent {
        override val type: String get() = TYPE

        companion object {
            const val TYPE = "scan_started"
        }
    }

    @Serializable
    data class ScanProgress(
        override val libraryId: Long,
        val phase: ScanPhase = ScanPhase.Unknown,
        val processed: Long = 0,
        val total: Long = 0,
    ) : ServerEvent {
        override val type: String get() = TYPE

        companion object {
            const val TYPE = "scan_progress"
        }
    }

    @Serializable
    data class ScanFinished(
        override val libraryId: Long,
        val added: Long = 0,
        val updated: Long = 0,
        val removed: Long = 0,
    ) : ServerEvent {
        override val type: String get() = TYPE

        companion object {
            const val TYPE = "scan_finished"
        }
    }

    /** Items or their details changed (a scan, markers found, something watched): read them again. */
    @Serializable
    data class LibraryChanged(override val libraryId: Long) : ServerEvent {
        override val type: String get() = TYPE

        companion object {
            const val TYPE = "library_changed"
        }
    }

    /** A library was made, changed or deleted, or their order changed. */
    data object LibrariesChanged : ServerEvent {
        const val TYPE = "libraries_changed"
        override val type: String get() = TYPE
    }

    /** Playback sessions started, stopped or moved on. */
    data object SessionsChanged : ServerEvent {
        const val TYPE = "sessions_changed"
        override val type: String get() = TYPE
    }

    /** An event this app doesn't know: skipped, but not an error. */
    data class Unknown(override val type: String) : ServerEvent

    /**
     * Reads and writes events by their `type`; the other fields are read
     * by each event's own serializer. JSON only.
     */
    object Serializer : KSerializer<ServerEvent> {
        override val descriptor: SerialDescriptor =
            buildClassSerialDescriptor("app.pley.android.model.ServerEvent") { element<String>("type") }

        override fun deserialize(decoder: Decoder): ServerEvent {
            val input = decoder as? JsonDecoder ?: throw SerializationException("server events are read from JSON")
            val tree = input.decodeJsonElement() as? JsonObject
                ?: throw SerializationException("a server event is a JSON object")
            val type = (tree["type"] as? JsonPrimitive)?.takeIf { it.isString }?.content.orEmpty()
            val fields = JsonObject(tree - "type")
            val json = input.json
            return when (type) {
                ScanStarted.TYPE -> json.decodeFromJsonElement(ScanStarted.serializer(), fields)
                ScanProgress.TYPE -> json.decodeFromJsonElement(ScanProgress.serializer(), fields)
                ScanFinished.TYPE -> json.decodeFromJsonElement(ScanFinished.serializer(), fields)
                LibraryChanged.TYPE -> json.decodeFromJsonElement(LibraryChanged.serializer(), fields)
                LibrariesChanged.TYPE -> LibrariesChanged
                SessionsChanged.TYPE -> SessionsChanged
                else -> Unknown(type)
            }
        }

        override fun serialize(encoder: Encoder, value: ServerEvent) {
            val output = encoder as? JsonEncoder ?: throw SerializationException("server events are written as JSON")
            val json = output.json
            val fields: Map<String, JsonElement> = when (value) {
                is ScanStarted -> json.encodeToJsonElement(ScanStarted.serializer(), value).jsonObject
                is ScanProgress -> json.encodeToJsonElement(ScanProgress.serializer(), value).jsonObject
                is ScanFinished -> json.encodeToJsonElement(ScanFinished.serializer(), value).jsonObject
                is LibraryChanged -> json.encodeToJsonElement(LibraryChanged.serializer(), value).jsonObject
                LibrariesChanged, SessionsChanged, is Unknown -> emptyMap()
            }
            output.encodeJsonElement(JsonObject(mapOf<String, JsonElement>("type" to JsonPrimitive(value.type)) + fields))
        }
    }

    companion object {
        /**
         * One event from the `data:` of a Server-Sent Event. Throws
         * [SerializationException] for text that isn't a JSON object, or a
         * known event missing a field it needs.
         */
        fun parse(text: String): ServerEvent = PleyJson.decodeFromString(Serializer, text)
    }
}
