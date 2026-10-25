package app.pley.android.caps

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Everything the probe reads from the device, before any rule is applied:
 * the device itself, its decoders, its screen and its audio output. Plain
 * data, so tests build it from fixtures (real devices' dumps among them)
 * and the cache keeps it as JSON.
 */
@Serializable
data class ProbeInput(
    val device: DeviceFacts,
    val decoders: List<DecoderInfo>,
    val display: DisplayState? = null,
    val audio: AudioState = AudioState(),
)

/** The device, as the quirk table and the tiers see it. */
@Serializable
data class DeviceFacts(
    /** Build.MODEL: "AFTKRT", "SHIELD Android TV", "Google TV Streamer"… */
    val model: String,
    /** Build.MANUFACTURER, for quirks that name a maker (TCL televisions). */
    val manufacturer: String = "",
    /** Build.VERSION.SDK_INT. */
    val sdk: Int,
    /** Build.FINGERPRINT: a new system build means a new probe. */
    val fingerprint: String = "",
    val form: Form,
    /** Amazon's feature amazon.hardware.fire_tv. */
    val fireTv: Boolean = false,
    val totalRamBytes: Long = 0,
    /** ActivityManager.isLowRamDevice. */
    val lowRam: Boolean = false,
) {
    @Serializable
    enum class Form {
        @SerialName("phone")
        Phone,

        @SerialName("tablet")
        Tablet,

        @SerialName("tv")
        Tv,
    }

    val isTv: Boolean get() = form == Form.Tv
}

/**
 * One decoder for one MIME type, as MediaCodecList describes it. Aliases
 * and decoders that only play protected or tunneled video aren't listed.
 */
@Serializable
data class DecoderInfo(
    val name: String,
    /** Lower case: "video/hevc", "audio/eac3"… */
    val mime: String,
    /** isHardwareAccelerated on Android 10 and up, else Media3's rules on the name. */
    val hardware: Boolean,
    val vendor: Boolean = false,
    /** MediaCodecInfo.CodecProfileLevel pairs, as the decoder lists them. */
    val profileLevels: List<ProfileLevel> = emptyList(),
    // Video.
    val maxWidth: Int? = null,
    val maxHeight: Int? = null,
    val maxFps: Double? = null,
    /** The probe's sizes and rates ([SizeRate.PROBED]) the decoder says it handles (areSizeAndRateSupported). */
    val sizeRates: List<SizeRate> = emptyList(),
    /** The probe's sizes and rates its performance points cover (Android 10 and up; empty when it lists none). */
    val performancePoints: List<SizeRate> = emptyList(),
    // Audio.
    val maxChannels: Int? = null,
) {
    val isVideo: Boolean get() = mime.startsWith("video/")
}

/** A MediaCodecInfo.CodecProfileLevel: Android's constants, not ffprobe's numbers. */
@Serializable
data class ProfileLevel(val profile: Int, val level: Int)

/** A picture size at a frame rate. */
@Serializable
data class SizeRate(val width: Int, val height: Int, val fps: Double) {
    companion object {
        val SIZES: List<Pair<Int, Int>> = listOf(1920 to 1080, 3840 to 2160)
        val RATES: List<Double> = listOf(23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0)

        /** What the probe asks each video decoder about: 1080p and 2160p at the usual film and TV rates. */
        val PROBED: List<SizeRate> = SIZES.flatMap { (w, h) -> RATES.map { SizeRate(w, h, it) } }
    }
}

/**
 * The screen the player shows on: its modes, the one in use, and the HDR
 * types it shows.
 */
@Serializable
data class DisplayState(
    val modes: List<DisplayModeInfo>,
    val activeModeId: Int,
    /** Display.getHdrCapabilities(): what the screen shows in its current mode (all Android versions). */
    val hdrCapabilities: List<Int> = emptyList(),
    /** The person's choice for matching the content's frame rate (Android 12 and up). */
    val matchContent: MatchContent = MatchContent.Unknown,
) {
    val activeMode: DisplayModeInfo? get() = modes.firstOrNull { it.id == activeModeId } ?: modes.firstOrNull()
}

/** One mode of the screen. */
@Serializable
data class DisplayModeInfo(
    val id: Int,
    /** Physical pixels, not the app's window. */
    val width: Int,
    val height: Int,
    val refreshRate: Float,
    /** Rates the screen switches to without a blank (Android 12 and up). */
    val alternativeRefreshRates: List<Float> = emptyList(),
    /** Mode.getSupportedHdrTypes() on Android 14 and up; `null` before. */
    val hdrTypes: List<Int>? = null,
)

/** DisplayManager.getMatchContentFrameRateUserPreference(). */
@Serializable
enum class MatchContent {
    @SerialName("never")
    Never,

    /** SEAMLESSS_ONLY: only to the mode's alternative rates. */
    @SerialName("seamless_only")
    SeamlessOnly,

    @SerialName("always")
    Always,

    /** Below Android 12, or not said. */
    @SerialName("unknown")
    Unknown,
}

/** The audio output in use, as Media3's AudioCapabilities sees it. */
@Serializable
data class AudioState(
    /** Media3's C.ENCODING_* values the output takes as they are. */
    val passthroughEncodings: List<Int> = emptyList(),
    /** The most channels the output takes. */
    val maxChannels: Int = 2,
    /** Where sound goes: "hdmi", "bluetooth", "speaker", "wired", "usb", "other". */
    val route: String = ROUTE_SPEAKER,
) {
    companion object {
        const val ROUTE_HDMI = "hdmi"
        const val ROUTE_BLUETOOTH = "bluetooth"
        const val ROUTE_SPEAKER = "speaker"
        const val ROUTE_WIRED = "wired"
        const val ROUTE_USB = "usb"
        const val ROUTE_OTHER = "other"
    }
}
