package app.pley.android.player.engine

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.util.Util
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.ForwardingAudioSink
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.mediacodec.MediaCodecUtil
import java.util.concurrent.ConcurrentHashMap

/**
 * Whether encoded sound is decoded here rather than passed through to a
 * receiver. Set on the main thread, read on the playback thread.
 */
class AudioDecoding {
    @Volatile
    var on: Boolean = false

    /** The sounds the output took only as PCM while [on]: those it passes through otherwise. */
    private val turnedDown: MutableSet<String> = ConcurrentHashMap.newKeySet()

    internal fun turnDown(format: Format) {
        turnedDown += key(format)
    }

    /**
     * Whether [format] is decoded here only because [on] is: back at
     * passthrough, it goes to the receiver as it is again.
     */
    fun turnedDown(format: Format): Boolean = key(format) in turnedDown

    /** Forgets the sounds turned down: [on] goes off. */
    fun forget() = turnedDown.clear()

    /** Not the codecs string: a stream's playlist and its segments may say it otherwise. */
    private fun key(format: Format) = "${format.sampleMimeType} ${format.channelCount} ${format.sampleRate}"
}

/**
 * The audio output, taking encoded sound (AC3, E-AC3, TrueHD, DTS) only as
 * PCM while [decoding] is on: the audio renderer then decodes it, and a
 * speed other than 1× applies (Media3 changes the speed of PCM only).
 * Sound that no decoder here takes ([decodes]) still passes through: it
 * plays, at its own speed.
 */
@OptIn(markerClass = [UnstableApi::class])
internal class DecodingAudioSink(
    sink: AudioSink,
    private val decoding: AudioDecoding,
    private val decodes: (Format) -> Boolean,
) : ForwardingAudioSink(sink) {
    override fun supportsFormat(format: Format): Boolean = getFormatSupport(format) != AudioSink.SINK_FORMAT_UNSUPPORTED

    override fun getFormatSupport(format: Format): Int {
        val support = super.getFormatSupport(format)
        if (!decoding.on || support != AudioSink.SINK_FORMAT_SUPPORTED_DIRECTLY || format.sampleMimeType == MimeTypes.AUDIO_RAW) return support
        // The renderer's decoder gives 16-bit PCM of the same layout: the output must take that instead.
        val pcm = Util.getPcmFormat(C.ENCODING_PCM_16BIT, format.channelCount, format.sampleRate)
        if (!super.supportsFormat(pcm) || !decodes(format)) return support
        decoding.turnDown(format)
        return AudioSink.SINK_FORMAT_UNSUPPORTED
    }

    companion object {
        /**
         * A decoder for [format] that takes it as it is (its channels, rate
         * and profile), as the audio renderer looks for one (E-AC3 JOC's E-AC3
         * decoder included): one that doesn't would leave the title silent.
         */
        fun hasDecoder(context: Context, format: Format): Boolean = format.sampleMimeType != null &&
            try {
                MediaCodecUtil.getDecoderInfosSoftMatch(MediaCodecSelector.DEFAULT, format, false, false).any { it.isFormatSupported(context, format) }
            } catch (_: MediaCodecUtil.DecoderQueryException) {
                false
            }
    }
}
