package app.pley.android.model

import app.pley.android.model.caps.ClientCaps
import kotlinx.serialization.Serializable

/** How the app asks for an item to be delivered (apps/server/src/playback.rs, `PlaybackMode`). */
@Serializable(with = PlaybackMode.Serializer::class)
enum class PlaybackMode(override val wire: String) : WireEnum {
    /** The cheapest the caps allow: direct play, then remux, then transcode. */
    Auto("auto"),

    /** The original file, even when the caps don't claim it plays. */
    Direct("direct"),

    /** HLS, the video copied when it can be. */
    Remux("remux"),

    /** HLS, the video always converted. */
    Transcode("transcode"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<PlaybackMode>("PlaybackMode", PlaybackMode.entries, PlaybackMode.Unknown)
}

/** How the server delivers it (apps/server/src/playback.rs, `Method`). */
@Serializable(with = PlaybackMethod.Serializer::class)
enum class PlaybackMethod(override val wire: String) : WireEnum {
    /** The file as it is, from GET /api/files/{id}/stream. */
    Direct("direct"),

    /** HLS with the video copied. */
    Remux("remux"),

    /** HLS with the video converted to H.264. */
    Transcode("transcode"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<PlaybackMethod>("PlaybackMethod", PlaybackMethod.entries, PlaybackMethod.Unknown)
}

/**
 * The body of POST /api/playback (apps/server/src/playback.rs,
 * `PlaybackRequest`). Fields left `null` are left out, and the server
 * picks: the item's first file, the file's default audio, no subtitles
 * burned in, the original quality, the start.
 */
@Serializable
data class PlaybackRequest(
    val itemId: Long,
    val fileId: Long? = null,
    /** [MediaStream.index] of the audio. */
    val audioIndex: Int? = null,
    /** Only for subtitles shown as pictures (PGS, VobSub), which are burned in by a transcode. */
    val subtitleIndex: Int? = null,
    /** kbps; `null`: the original quality. */
    val maxBitrate: Int? = null,
    /** Lines: 1080, 720, 480. */
    val maxHeight: Int? = null,
    /** Where to start, in ms; a transcode starts producing there. */
    val startMs: Long? = null,
    val mode: PlaybackMode = PlaybackMode.Auto,
    /** The player: starting a session ends this profile's earlier ones on the same client. */
    val clientId: String? = null,
    /** What this device decodes. */
    val caps: ClientCaps = ClientCaps(),
)

/** A subtitle track of the file being played. */
@Serializable
data class SubtitleTrack(
    val index: Int,
    val codec: String,
    val language: String? = null,
    val title: String? = null,
    val default: Boolean = false,
    val forced: Boolean = false,
    val external: Boolean = false,
    val textBased: Boolean = false,
    /** WebVTT: "/api/files/7/subtitles/3.vtt"; `null` for subtitles shown as pictures. */
    val url: String? = null,
)

/**
 * The answer of POST /api/playback (apps/server/src/playback.rs,
 * `PlaybackInfo`): how the item plays and from where.
 */
@Serializable
data class PlaybackInfo(
    /** For progress reports, the session's status and DELETE /api/playback/{session} when done. */
    val sessionId: String,
    val method: PlaybackMethod,
    /** Direct: "/api/files/{id}/stream"; otherwise the HLS playlist, "/api/playback/{session}/index.m3u8". */
    val url: String,
    val hls: Boolean = false,
    val fileId: Long,
    val durationMs: Long = 0,
    val audioIndex: Int? = null,
    /** The subtitle burned in, only when [burnSubtitles]. */
    val subtitleIndex: Int? = null,
    val burnSubtitles: Boolean = false,
    /** The audio is converted, also in a remux, where the video is copied. */
    val audioConverted: Boolean = false,
    /** Every subtitle track of the file. */
    val subtitles: List<SubtitleTrack> = emptyList(),
    /** Why it isn't played as it is, one short sentence each. */
    val reasons: List<String> = emptyList(),
    /** The same in one line. */
    val reason: String? = null,
    /**
     * When the original is converted: the closest quality players offer
     * ("30m" = 30 Mbps 4K, "20m", "15m" = 15 Mbps 4K, "12m", "8m", "4m",
     * "2m"); `null` when a quality was asked for.
     */
    val autoQuality: String? = null,
)

/** A transcode job's state (packages/transcoder-api, `JobState`). */
@Serializable(with = JobState.Serializer::class)
enum class JobState(override val wire: String) : WireEnum {
    Starting("starting"),

    /** An ffmpeg process is making segments. */
    Running("running"),

    /** Nothing running: the player paused, or everything asked for is ready. */
    Idle("idle"),

    /** The whole file is made. */
    Finished("finished"),

    /** The last process failed: see [TranscodeInfo.lastError]. */
    Failed("failed"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<JobState>("JobState", JobState.entries, JobState.Unknown)
}

/** What a transcode does with a stream. */
@Serializable(with = StreamAction.Serializer::class)
enum class StreamAction(override val wire: String) : WireEnum {
    Copy("copy"),
    Transcode("transcode"),

    /** The file has no such stream, or it isn't played. */
    None("none"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<StreamAction>("StreamAction", StreamAction.entries, StreamAction.Unknown)
}

/**
 * The live numbers of a session's transcode, for the player's details
 * (apps/server/src/playback.rs, `TranscodeInfo`).
 */
@Serializable
data class TranscodeInfo(
    val jobId: String,
    val state: JobState = JobState.Unknown,
    val video: StreamAction = StreamAction.Unknown,
    val audio: StreamAction = StreamAction.Unknown,
    /** "HEVC 3840×2160 Dolby Vision P8.1". */
    val videoIn: String? = null,
    /** "H.264 1920×1080". */
    val videoOut: String? = null,
    /** "EAC3 5.1". */
    val audioIn: String? = null,
    /** "AAC 2.0". */
    val audioOut: String? = null,
    /** "h264_nvenc", "libx264"… */
    val encoder: String? = null,
    /** The software encoder's preset now ("veryfast"…); the automatic one changes. */
    val preset: String? = null,
    /** "cuda", "vulkan", "cpu"… */
    val decoder: String? = null,
    /** "gpu" (decode, filters and encode on one GPU), "hybrid", "cpu" or "copy". */
    val pipeline: String? = null,
    /** "libplacebo", "opencl" or "zscale" when HDR or Dolby Vision is tone-mapped to SDR. */
    val tonemap: String? = null,
    val subtitleBurn: Boolean = false,
    /** ffmpeg's speed: 2.5 is two and a half times real time. */
    val speed: Double? = null,
    val fps: Double? = null,
    /** How far ffmpeg got, in ms of the media. */
    val transcodedMs: Long = 0,
    val segmentsReady: Int = 0,
    val segmentsTotal: Int = 0,
    /** What the transcode aims for, video and audio together, in bits per second. */
    val bitrateOut: Long? = null,
    val lastError: String? = null,
)

/**
 * GET /api/playback/{session}: how the session plays now, with its
 * transcode's live numbers (apps/server/src/playback.rs, `SessionStatus`).
 */
@Serializable
data class SessionStatus(
    val sessionId: String,
    val method: PlaybackMethod,
    /** `null` for direct play, or when the transcoder can't say. */
    val transcode: TranscodeInfo? = null,
)

/** The state a progress report gives. */
@Serializable(with = ProgressState.Serializer::class)
enum class ProgressState(override val wire: String) : WireEnum {
    Playing("playing"),
    Paused("paused"),

    /** Waiting for data. The server takes it (apps/server/src/api/items.rs) though docs/API.md doesn't list it. */
    Buffering("buffering"),

    /** The player closed. */
    Stopped("stopped"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<ProgressState>("ProgressState", ProgressState.entries, ProgressState.Unknown)
}

/**
 * The body of POST /api/items/{id}/progress (apps/server/src/api/items.rs,
 * `Progress`): every ~10 s while playing, on pause, and [ProgressState.Stopped]
 * on closing. At 90 % the item is watched and its resume point cleared.
 */
@Serializable
data class ProgressReport(
    val positionMs: Long,
    /** The player's own duration; `null`: the item's. */
    val durationMs: Long? = null,
    val state: ProgressState = ProgressState.Playing,
    /** Keeps the session, and its transcode, alive. */
    val sessionId: String? = null,
    /**
     * How long ago the report was made, for one sent late (a download played
     * while the server was out of reach): the server keeps whatever happened
     * to the item since.
     */
    val ageMs: Long? = null,
)

/**
 * The body of PUT /api/items/{id}/streams: the tracks picked, remembered
 * for the whole show (episodes) or the movie, by language, title and
 * forced, so every episode starts with its own matching tracks.
 */
@Serializable
data class StreamChoice(
    /** The file the indexes are of; `null`: the item's first. */
    val fileId: Long? = null,
    val audioIndex: Int? = null,
    /** `null`: subtitles off. */
    val subtitleIndex: Int? = null,
)

/** The body of PUT /api/items/{id}/rating: 0 to 10, rounded to half stars; `null` clears it. */
@Serializable
data class RatingRequest(
    val rating: Double? = null,
)
