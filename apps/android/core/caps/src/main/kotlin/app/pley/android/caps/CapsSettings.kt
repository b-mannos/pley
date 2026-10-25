package app.pley.android.caps

import androidx.media3.common.C
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Auto: what the device says; On: always; Off: never. */
@Serializable
enum class Override {
    @SerialName("auto")
    Auto,

    @SerialName("on")
    On,

    @SerialName("off")
    Off,
}

/**
 * The surround formats a person can send to a receiver or TV as they are
 * (Settings → Audio), each with the Media3 encodings it covers and the
 * server's codec name.
 */
@Serializable
enum class PassthroughFormat(val encodings: List<Int>, val codec: String) {
    @SerialName("dolby_digital")
    DolbyDigital(listOf(C.ENCODING_AC3), "ac3"),

    @SerialName("dolby_digital_plus")
    DolbyDigitalPlus(listOf(C.ENCODING_E_AC3), "eac3"),

    /** Dolby Atmos in E-AC-3 (JOC). */
    @SerialName("atmos")
    Atmos(listOf(C.ENCODING_E_AC3_JOC), "eac3"),

    @SerialName("truehd")
    TrueHd(listOf(C.ENCODING_DOLBY_TRUEHD), "truehd"),

    @SerialName("dts")
    Dts(listOf(C.ENCODING_DTS), "dts"),

    /** DTS-HD and DTS:X. */
    @SerialName("dts_hd")
    DtsHd(listOf(C.ENCODING_DTS_HD, C.ENCODING_DTS_UHD_P2), "dts"),
    ;

    companion object {
        /** The format an encoding belongs to; `null` for PCM and the rest. */
        fun of(encoding: Int): PassthroughFormat? = entries.firstOrNull { encoding in it.encodings }
    }
}

/**
 * What the person chose that changes the report: passthrough per format,
 * and (debug builds) software decoders for emulators.
 */
@Serializable
data class CapsSettings(
    val passthrough: Map<PassthroughFormat, Override> = emptyMap(),
    /** Debug builds only: software video decoders allowed at any size, for tests on emulators. */
    val allowSoftwareDecoders: Boolean = false,
) {
    fun passthrough(format: PassthroughFormat): Override = passthrough[format] ?: Override.Auto
}
