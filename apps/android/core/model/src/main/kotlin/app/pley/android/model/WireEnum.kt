package app.pley.android.model

import kotlinx.serialization.KSerializer
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

/** A value the server names with a string: [wire] is that string ("movie", "dolby_vision"). */
interface WireEnum {
    val wire: String
}

/**
 * Reads a server enum by its wire name, and any name this app doesn't know
 * as [unknown] instead of failing: a newer server can add a kind, a state
 * or a phase without breaking the answers an older app reads. Writes the
 * wire name back; [unknown] goes out as its own wire name ("unknown"),
 * which no route takes, so the app never sends it on purpose.
 *
 * Every enum of the server's uses one, as a nested object:
 *
 * ```
 * @Serializable(with = ItemKind.Serializer::class)
 * enum class ItemKind(override val wire: String) : WireEnum {
 *     Movie("movie"), …, Unknown("unknown");
 *
 *     object Serializer : LenientEnumSerializer<ItemKind>("ItemKind", ItemKind.entries, ItemKind.Unknown)
 * }
 * ```
 */
open class LenientEnumSerializer<E>(
    serialName: String,
    values: List<E>,
    private val unknown: E,
) : KSerializer<E> where E : Enum<E>, E : WireEnum {
    private val byWire: Map<String, E> = values.associateBy { it.wire }

    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("app.pley.android.model.$serialName", PrimitiveKind.STRING)

    override fun deserialize(decoder: Decoder): E = byWire[decoder.decodeString()] ?: unknown

    override fun serialize(encoder: Encoder, value: E) = encoder.encodeString(value.wire)
}
