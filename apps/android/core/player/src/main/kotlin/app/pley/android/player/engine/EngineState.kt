package app.pley.android.player.engine

/**
 * What an engine knows beyond the player's own state, in one immutable
 * value: the session reads it to show "Matching refresh rate…", to ask the
 * server for help, and to fill Playback details.
 */
data class EngineState(
    /** The TV is changing its display mode for the video: playback waits, the picture may go dark. */
    val switchingDisplayMode: Boolean = false,
    /**
     * The TV didn't change its mode twice in a row on this display, so Auto
     * stopped asking: show once where the device's own setting is.
     */
    val frameRateHint: Boolean = false,
    /** Tracks this device can't play as they are: the server has to remux or transcode. */
    val needsServerHelp: NeedsServerHelp? = null,
    /** The video decoder drops frames: over 1 % in 10 s, or 10 in a row. */
    val struggling: Boolean = false,
    /** The audio output failed and the engine started it again at the same position. */
    val audioRestarts: Int = 0,
    val stats: PlaybackStats = PlaybackStats(),
)

/**
 * The file has a video or an audio stream, and the device can play none of
 * them: the server must remux (audio) or transcode (video).
 */
data class NeedsServerHelp(val video: Boolean, val audio: Boolean)

/** What Playback details and Diagnostics show of a playback, and what the benchmarks read. */
data class PlaybackStats(
    /** From [app.pley.android.player.PlayerEngine.prepare] to the first frame drawn. */
    val firstFrameMs: Long? = null,
    val videoDecoder: String? = null,
    /** Whether [videoDecoder] is a hardware decoder; `null` when unknown. */
    val videoDecoderHardware: Boolean? = null,
    val audioDecoder: String? = null,
    /** The video's MIME type and codec string: `video/hevc hvc1.2.4.L153`. */
    val videoCodec: String? = null,
    val width: Int? = null,
    val height: Int? = null,
    val frameRate: Float? = null,
    /** "HDR10", "HLG", "Dolby Vision", "SDR"; `null` before the video's format is known. */
    val dynamicRange: String? = null,
    /** The colour information as Media3 reads it (primaries, transfer, range). */
    val colorInfo: String? = null,
    val audioCodec: String? = null,
    val audioChannels: Int? = null,
    /** Media3's C.ENCODING_* of the audio output. */
    val audioEncoding: Int? = null,
    /** Encoded audio (AC3, E-AC3, TrueHD, DTS…) goes to the TV or receiver as it is. */
    val passthrough: Boolean = false,
    val renderedFrames: Int = 0,
    val droppedFrames: Int = 0,
    val maxConsecutiveDropped: Int = 0,
    /** Stalls while playing, seeks aside. */
    val rebuffers: Int = 0,
    /** The display mode the engine asked for (its id), `null` when it left the mode alone. */
    val displayModeId: Int? = null,
)
