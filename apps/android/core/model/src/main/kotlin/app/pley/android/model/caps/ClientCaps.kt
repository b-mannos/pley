package app.pley.android.model.caps

import app.pley.android.model.DynamicRange
import app.pley.android.model.LenientEnumSerializer
import app.pley.android.model.WireEnum
import kotlinx.serialization.Serializable

/**
 * What this device plays, sent as `caps` with POST /api/playback.
 *
 * Two versions travel in this one object. The first five fields are
 * version 1, what the server reads today (apps/server/src/playback.rs,
 * `Caps`); they are always sent, and only they, while [capsVersion] is
 * `null`, so the body is the one the server has always taken. Version 2
 * is a proposal the server doesn't read yet: set [capsVersion] to
 * [VERSION_2] and fill the fields after it. The server ignores fields it
 * doesn't know, so a version 2 report still plays by its version 1 fields
 * on today's servers; keep both filled.
 *
 * Codec and container names are the server's (ffprobe's): "h264", "hevc",
 * "av1", "aac", "eac3", "mkv"…
 */
@Serializable
data class ClientCaps(
    /** Containers played as they are: "mp4", "mkv", "webm"… */
    val containers: List<String> = emptyList(),
    val videoCodecs: List<String> = emptyList(),
    val audioCodecs: List<String> = emptyList(),
    /** Shows HDR10 and HLG as they are, without tone mapping. */
    val hdr: Boolean = false,
    /** Decodes Dolby Vision, profile 5 included (it has no HDR10 or SDR base layer). */
    val dolbyVision: Boolean = false,
    // Version 2: left out while null.
    /** [VERSION_2] for a version 2 report; `null`: version 1 only. */
    val capsVersion: Int? = null,
    /** Each video decoder, with its limits. */
    val video: List<VideoDecoderCaps>? = null,
    /** The dynamic ranges the screen shows as they are. */
    val hdrTypes: List<DynamicRange>? = null,
    /** The Dolby Vision profiles decoded, each with its highest level. */
    val dolbyVisionProfiles: List<DolbyVisionCaps>? = null,
    /** Audio codecs decoded on the device. */
    val audioDecode: List<String>? = null,
    /** Audio codecs sent as they are to a receiver or a TV (HDMI, S/PDIF): "ac3", "eac3", "truehd", "dts"… */
    val audioPassthrough: List<String>? = null,
    /** The most audio channels the output takes: 2, 6, 8. */
    val audioChannels: Int? = null,
    /** Subtitle codecs shown by the player itself: "subrip", "ass", "webvtt", "hdmv_pgs_subtitle"… */
    val subtitleCodecs: List<String>? = null,
    /** The player switches audio tracks of a file itself, without a new session. */
    val audioTrackSwitching: Boolean? = null,
    val hls: HlsCaps? = null,
    val display: DisplayCaps? = null,
    val device: DeviceCaps? = null,
) {
    companion object {
        /** [capsVersion] of a version 2 report. */
        const val VERSION_2: Int = 2
    }
}

/** One video decoder (version 2). Limits left `null` aren't known. */
@Serializable
data class VideoDecoderCaps(
    val codec: String,
    /** A hardware decoder; software ones are slow for big pictures. */
    val hardware: Boolean = false,
    /** The codec's profiles decoded, as ffprobe names them: "Main", "Main 10", "High"… */
    val profiles: List<String> = emptyList(),
    val maxWidth: Int? = null,
    val maxHeight: Int? = null,
    val maxFps: Double? = null,
    val maxBitDepth: Int? = null,
    /** The highest level, numbered as ffprobe does ([app.pley.android.model.MediaStream.level]). */
    val maxLevel: Int? = null,
)

/** A Dolby Vision profile decoded (version 2). */
@Serializable
data class DolbyVisionCaps(
    /** 5, 7, 8… */
    val profile: Int,
    val maxLevel: Int? = null,
)

/** What the player takes of HLS (version 2). */
@Serializable
data class HlsCaps(
    /** Reads a master playlist with several variants, not only one media playlist. */
    val masterPlaylist: Boolean = false,
    /** The segment formats it plays: "ts", "fmp4". */
    val segments: List<String> = emptyList(),
)

/** The screen (version 2). */
@Serializable
data class DisplayCaps(
    /** Pixels of the screen's mode, not of the app's window. */
    val width: Int,
    val height: Int,
    /** The refresh rates the screen can switch to, in Hz: 23.976, 50, 60… */
    val refreshRates: List<Float> = emptyList(),
)

/** What kind of device it is. */
@Serializable(with = DeviceForm.Serializer::class)
enum class DeviceForm(override val wire: String) : WireEnum {
    Phone("phone"),
    Tablet("tablet"),
    Tv("tv"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<DeviceForm>("DeviceForm", DeviceForm.entries, DeviceForm.Unknown)
}

/** The device (version 2). */
@Serializable
data class DeviceCaps(
    val form: DeviceForm = DeviceForm.Unknown,
    /** How capable the device is, as the app ranks it ("low", "mid", "high"). */
    val tier: String? = null,
    /** An Amazon Fire TV device. */
    val fireTv: Boolean = false,
    /** "Chromecast HD", "Pixel 9"… */
    val model: String? = null,
)
