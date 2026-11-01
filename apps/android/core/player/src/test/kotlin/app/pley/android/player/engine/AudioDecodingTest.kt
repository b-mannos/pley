package app.pley.android.player.engine

import android.content.Context
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.Util
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import androidx.media3.exoplayer.audio.ForwardingAudioSink
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class AudioDecodingTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val decoding = AudioDecoding()
    private val ac3 = Format.Builder().setSampleMimeType(MimeTypes.AUDIO_AC3).setChannelCount(6).setSampleRate(48_000).build()

    /** An output that takes AC3 as it is, as a receiver behind HDMI does. */
    private class Receiver(sink: AudioSink) : ForwardingAudioSink(sink) {
        override fun supportsFormat(format: Format): Boolean = getFormatSupport(format) != AudioSink.SINK_FORMAT_UNSUPPORTED

        override fun getFormatSupport(format: Format): Int =
            if (format.sampleMimeType == MimeTypes.AUDIO_AC3) AudioSink.SINK_FORMAT_SUPPORTED_DIRECTLY else super.getFormatSupport(format)
    }

    private fun sink(decodes: Boolean) = DecodingAudioSink(Receiver(DefaultAudioSink.Builder(context).build()), decoding) { decodes }

    @Test
    fun `encoded sound passes through until it is to be decoded here`() {
        val sink = sink(decodes = true)
        assertTrue(sink.supportsFormat(ac3))
        decoding.on = true
        assertFalse(sink.supportsFormat(ac3))
        assertEquals(AudioSink.SINK_FORMAT_UNSUPPORTED, sink.getFormatSupport(ac3))
        // The decoder's PCM goes out as before.
        assertTrue(sink.supportsFormat(Util.getPcmFormat(C.ENCODING_PCM_16BIT, 6, 48_000)))
        decoding.on = false
        assertTrue(sink.supportsFormat(ac3))
    }

    @Test
    fun `sound that nothing here decodes still passes through`() {
        decoding.on = true
        assertEquals(AudioSink.SINK_FORMAT_SUPPORTED_DIRECTLY, sink(decodes = false).getFormatSupport(ac3))
        assertFalse(decoding.turnedDown(ac3))
    }

    @Test
    fun `sound taken as PCM only for the speed is known, to pass it through again`() {
        val sink = sink(decodes = true)
        sink.getFormatSupport(ac3)
        assertFalse("passed through", decoding.turnedDown(ac3))
        decoding.on = true
        sink.getFormatSupport(ac3)
        assertTrue(decoding.turnedDown(ac3))
        // Another layout of it wasn't asked for.
        assertFalse(decoding.turnedDown(ac3.buildUpon().setChannelCount(2).build()))
        decoding.forget()
        assertFalse(decoding.turnedDown(ac3))
    }
}
