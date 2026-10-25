package app.pley.android.player

import android.content.Context
import androidx.media3.common.Player
import app.pley.android.common.DeviceProfile

/** Where the engine reads a video from: a file as it is, or the server's HLS playlist. */
data class EngineSource(
    val url: String,
    val hls: Boolean,
    /** The container's MIME type when the address doesn't tell (`video/x-matroska`…). */
    val mimeType: String? = null,
)

/**
 * Media3 as pley plays with it: the decoders this device should use, buffers
 * sized for its memory, the TV's display mode matched to the video, and
 * audio that follows the output. One engine per player screen; music has
 * its own.
 */
interface PlayerEngine {
    /** The player, for surfaces, subtitles and the media session. */
    val player: Player

    /** Loads [source] at [startMs]; plays at once when [playWhenReady]. */
    fun prepare(source: EngineSource, startMs: Long, playWhenReady: Boolean)

    /** Lets go of the decoders and the audio output (the TV's player does in onStop). */
    fun release()

    /** Makes engines for this device. */
    fun interface Factory {
        /** An engine for video, or for music when [music] (no video decoders, offload with the screen off). */
        fun create(context: Context, profile: DeviceProfile, music: Boolean): PlayerEngine
    }
}
