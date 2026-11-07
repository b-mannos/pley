package app.pley.android.model.admin

import app.pley.android.model.LenientEnumSerializer
import app.pley.android.model.WireEnum
import kotlinx.serialization.Serializable

/*
 * Settings → Transcoder (docs/API.md, "System"; apps/server/src/settings.rs
 * and api/admin.rs): the transcoder's settings, what its FFmpeg can do,
 * and the folder it works in.
 */

/** The hardware that encodes transcodes (`HwAccel`); [Auto] lets the transcoder pick. */
@Serializable(with = HwAccel.Serializer::class)
enum class HwAccel(override val wire: String) : WireEnum {
    Auto("auto"),
    None("none"),
    Nvenc("nvenc"),
    Qsv("qsv"),
    Amf("amf"),
    Vaapi("vaapi"),
    Videotoolbox("videotoolbox"),

    /** Set through the API only: offered when saved. */
    Vulkan("vulkan"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<HwAccel>("HwAccel", HwAccel.entries, HwAccel.Unknown)
}

/** How HDR is tone mapped to SDR (`TonemapMode`). */
@Serializable(with = ToneMapping.Serializer::class)
enum class ToneMapping(override val wire: String) : WireEnum {
    Auto("auto"),
    Libplacebo("libplacebo"),
    Opencl("opencl"),
    Zscale("zscale"),
    None("none"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<ToneMapping>("ToneMapping", ToneMapping.entries, ToneMapping.Unknown)
}

/** The transcoder's fields of GET /api/settings, and what PUT /api/settings answers. */
@Serializable
data class TranscoderSettings(
    val hwaccel: HwAccel = HwAccel.Auto,
    val hardwareDecoding: Boolean = true,
    val tonemapping: ToneMapping = ToneMapping.Auto,
    val maxTranscodes: Int = 4,
    /** x264's and x265's preset by its name, or "auto". */
    val transcodePreset: String = "auto",
    val throttleTranscodes: Boolean = true,
    val transcodeInMemory: Boolean = true,
)

/** What this part reads of GET /api/system (`SystemInfo`). */
@Serializable
data class TranscoderSystem(
    /** Pley's operating system: "windows", "linux", "macos". */
    val os: String = "",
    val transcodeDir: TranscodeFolder? = null,
    val transcoder: TranscoderState = TranscoderState(),
)

/** Where the transcoder works (`SystemInfo.transcode_dir`), and what PUT /api/system/transcode-dir answers. */
@Serializable
data class TranscodeFolder(
    /** The folder it works in now. */
    val inUse: String = "",
    /** The one Pley's next start uses, when that is another. */
    val next: String? = null,
    /** `transcode` in the data folder: used when none is chosen, or the one chosen can't be. */
    val default: String = "",
    /** The folder chosen, when it couldn't be used at start, and why. */
    val problem: TranscodeFolderProblem? = null,
    /** The command line or the environment chooses it. */
    val fixed: Boolean = false,
) {
    /** The folder Pley's next start tries: the one saved, even when it couldn't be used this time. */
    val chosen: String get() = next ?: problem?.folder ?: inUse
}

@Serializable
data class TranscodeFolderProblem(
    val folder: String,
    val reason: String,
)

/** The transcoder and its FFmpeg (`SystemInfo.transcoder`). */
@Serializable
data class TranscoderState(
    val available: Boolean = false,
    /** Started by Pley, else reached at [url]. */
    val managed: Boolean = true,
    val url: String? = null,
    val version: String? = null,
    val ffmpegVersion: String? = null,
    /** Working video encoders: "h264_nvenc", "libx264". */
    val encoders: List<String> = emptyList(),
    val activeEncoder: String? = null,
    /** Hardware with a working H.264 encoder, best first (the one Automatic uses); `null` from servers before it. */
    val accels: List<HwAccel>? = null,
    /** Some video is decoded on a GPU here. */
    val hardwareDecoding: Boolean = false,
    /** What each [HwAccel] choice does here, by its wire name; empty while unavailable. */
    val withAccel: Map<String, TranscoderAccelUse> = emptyMap(),
    /** Tone mappers that work: "libplacebo", "opencl", "zscale". */
    val tonemapping: List<String> = emptyList(),
    val dolbyVision: Boolean = false,
    /** FFmpeg can hold transcodes back (6.1 and newer). */
    val throttling: Boolean = false,
    /** Whether segments can be kept in memory; `null` while unavailable, or from a transcoder before it. */
    val memory: TranscoderMemory? = null,
    /** The most the transcoder itself runs at once. */
    val maxJobs: Int = 0,
    val error: String? = null,
) {
    /** What choosing [accel] does here, when the transcoder said. */
    fun with(accel: HwAccel): TranscoderAccelUse? = withAccel[accel.wire]
}

/** What a hardware acceleration choice does on this server (`TranscoderAccelUse`). */
@Serializable
data class TranscoderAccelUse(
    /** Some video is decoded on a GPU with it. */
    val hardwareDecoding: Boolean = true,
    /** What it leaves to software encoders: "h264", "hevc", "hdr" (HEVC in 10 bits). */
    val software: List<String> = emptyList(),
    /** The tone mappers that work with it, where the transcoder tried them; `null`: every one it has. */
    val tonemappers: List<String>? = null,
)

/** Keeping transcodes' segments in RAM (`TranscoderMemory`). */
@Serializable
data class TranscoderMemory(
    val available: Boolean = false,
    /** Why not, in the server's words ("works on Linux only"). */
    val reason: String? = null,
    /** The most segments take there. */
    val budgetBytes: Long = 0,
)

/** POST /api/settings/test's answer: whether the service answered, said for people, and how fast. */
@Serializable
data class TranscoderTestResult(
    val ok: Boolean = false,
    val message: String = "",
    val ms: Long = 0,
)
