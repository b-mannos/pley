package app.pley.android.player.engine

import android.os.Handler
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.Timeline
import androidx.media3.common.TrackGroup
import androidx.media3.datasource.TransferListener
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.chunk.MediaChunkIterator
import androidx.media3.exoplayer.trackselection.AdaptiveTrackSelection
import androidx.media3.exoplayer.trackselection.ExoTrackSelection
import androidx.media3.exoplayer.upstream.BandwidthMeter
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** How Media3 moves between the sizes of a stream in several (docs/QUALITY.md, "Several sizes"). */
@RunWith(RobolectricTestRunner::class)
class SizeSwitchingTest {
    /** A network measured at [kbps]. */
    private class Meter(var kbps: Long) : BandwidthMeter {
        override fun getBitrateEstimate(): Long = kbps * 1_000

        override fun getTransferListener(): TransferListener? = null

        override fun addEventListener(eventHandler: Handler, eventListener: BandwidthMeter.EventListener) = Unit

        override fun removeEventListener(eventListener: BandwidthMeter.EventListener) = Unit
    }

    private fun size(height: Int, kbps: Int): Format = Format.Builder()
        .setId("${height}p")
        .setSampleMimeType(MimeTypes.VIDEO_H264)
        .setAverageBitrate(kbps * 1_000)
        .setWidth(height * 16 / 9)
        .setHeight(height)
        .build()

    /** A transcode at 8 Mbps 1080p and its 360p size. */
    private val sizes = TrackGroup(size(1080, 8_000), size(360, 700))

    private fun selection(factory: ExoTrackSelection.Factory, meter: BandwidthMeter): ExoTrackSelection =
        factory.createTrackSelections(
            arrayOf(ExoTrackSelection.Definition(sizes, 0, 1)),
            meter,
            MediaSource.MediaPeriodId(Any()),
            Timeline.EMPTY,
        )[0]!!.also { it.enable() }

    /** Media3 looks again with [bufferedS] seconds buffered: the lines it picks. */
    private fun ExoTrackSelection.lines(bufferedS: Int): Int {
        updateSelectedTrack(0, bufferedS * 1_000_000L, C.TIME_UNSET, emptyList(), Array(length()) { MediaChunkIterator.EMPTY })
        return selectedFormat.height
    }

    /** Starts at 360p on a slow network, which then becomes fast: the buffer at which it moves up to 1080p. */
    private fun movesUpAt(factory: ExoTrackSelection.Factory): Int {
        val meter = Meter(kbps = 2_000)
        val s = selection(factory, meter)
        assertEquals(360, s.lines(0))
        meter.kbps = 50_000
        for (buffered in 0..60) {
            if (s.lines(buffered) == 1080) return buffered
        }
        return -1
    }

    @Test
    fun movesUpOnlyWith20SecondsBuffered() {
        assertEquals(20_000, Media3Engine.MIN_BUFFER_FOR_LARGER_SIZE_MS)
        assertEquals(20, movesUpAt(Media3Engine.sizeSwitching()))
        // Media3's own default: 10 s.
        assertEquals(10, movesUpAt(AdaptiveTrackSelection.Factory()))
    }

    @Test
    fun movesDownAsMedia3Does() {
        val meter = Meter(kbps = 50_000)
        val ours = selection(Media3Engine.sizeSwitching(), meter)
        val media3 = selection(AdaptiveTrackSelection.Factory(), meter)
        assertEquals(1080, ours.lines(0))
        assertEquals(1080, media3.lines(0))
        meter.kbps = 2_000
        // With 25 s buffered or more, Media3 keeps the size; below, it moves down.
        for (buffered in listOf(40, 25, 24, 10, 0)) {
            assertEquals(media3.lines(buffered), ours.lines(buffered))
        }
        assertEquals(360, ours.lines(10))
    }
}
