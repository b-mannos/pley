package app.pley.android.player.engine

import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.ForwardingAudioSink

/**
 * How much later than the picture the sound plays, in µs (negative:
 * earlier). Set on the main thread, read on the playback thread.
 */
class AudioDelay {
    @Volatile
    var us: Long = 0
}

/**
 * The audio output, its clock moved by [delay]: the audio renderer is the
 * player's clock, and the video renderer shows the frame of the time it
 * says. Saying [AudioDelay.us] more than the sound heard shows the picture
 * that much ahead of it, so the sound comes that much later; less, earlier.
 * The sound itself goes out as before, decoded or passed through to a
 * receiver (its position comes from the output's own timestamps either
 * way), so a delay holds as steadily as the output's sync. Only a tunneled
 * output, where the TV times the frames against the sound itself, can't
 * be moved this way.
 */
@OptIn(markerClass = [UnstableApi::class])
internal class DelayedAudioSink(sink: AudioSink, private val delay: AudioDelay) : ForwardingAudioSink(sink) {
    override fun getCurrentPositionUs(sourceEnded: Boolean): Long {
        val position = super.getCurrentPositionUs(sourceEnded)
        return if (position == AudioSink.CURRENT_POSITION_NOT_SET) position else position + delay.us
    }
}
