package app.pley.android.player.session

import app.pley.android.fixtures.FixtureIds
import app.pley.android.model.PlaybackMode
import app.pley.android.player.PlayRequest
import app.pley.android.player.PlayerPhase
import app.pley.android.testing.caps.TestDevices
import app.pley.android.testing.caps.capsOf
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The recovery ladder, played against the FixtureServer with a fake engine. */
@RunWith(RobolectricTestRunner::class)
class RecoveryFlowTest {
    private var h = SessionHarness()

    @After
    fun close() = h.close()

    private fun starts() = h.timeline().count { it == "start" }

    @Test
    fun `direct play whose audio fails becomes a remux, then a transcode, then an error`() {
        val c = h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.ready()
        h.advance(2_000)
        h.engine.fail(EngineError.Unsupported(audio = true, container = false, detail = "ERROR_CODE_DECODER_INIT_FAILED"))
        h.settle()
        val remux = h.playbackRequests()[1]
        assertEquals(PlaybackMode.Remux, remux.mode)
        assertEquals(2_000L, remux.startMs)
        assertTrue(h.engine.prepared.last().source.hls)
        assertEquals("This device couldn't play the original file.", c.state.value.note)
        assertEquals("Direct stream", c.state.value.method)

        h.engine.ready()
        h.engine.fail(EngineError.Unsupported(audio = false, container = false, detail = "ERROR_CODE_DECODING_FAILED"))
        h.settle()
        assertEquals(PlaybackMode.Transcode, h.playbackRequests()[2].mode)
        assertEquals("Transcoding", c.state.value.method)

        h.engine.ready()
        h.engine.fail(EngineError.Unsupported(audio = false, container = false, detail = "ERROR_CODE_DECODING_FAILED"))
        h.settle()
        assertEquals(PlayerPhase.Error("This video can't be played on this device.", canRetry = false), c.state.value.phase)
        assertEquals(3, starts())
        // Each new session came after the one before ended.
        assertEquals(
            listOf("start", "progress stopped", "delete", "start", "progress stopped", "delete", "start"),
            h.timeline(),
        )
    }

    @Test
    fun `a video the device can't decode is transcoded at once`() {
        h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.ready()
        h.engine.fail(EngineError.Unsupported(audio = false, container = false, detail = "ERROR_CODE_DECODING_FORMAT_UNSUPPORTED"))
        h.settle()
        assertEquals(listOf(PlaybackMode.Auto, PlaybackMode.Transcode), h.playbackRequests().map { it.mode })
    }

    @Test
    fun `a track no decoder takes counts as a decoder error`() {
        h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.tracks(
            EngineTracks(
                video = listOf(EngineTrack("und", "video/avc", null, supported = true)),
                audio = listOf(EngineTrack("en", "audio/ac3", 6, supported = false)),
            ),
        )
        h.settle()
        assertEquals(listOf(PlaybackMode.Auto, PlaybackMode.Remux), h.playbackRequests().map { it.mode })
    }

    @Test
    fun `an expired session starts again at the position, three times, then Retry starts over`() {
        val c = h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.ready()
        h.advance(3_000)
        repeat(3) {
            h.engine.fail(EngineError.Http(404, "ERROR_CODE_IO_BAD_HTTP_STATUS"))
            h.settle()
            h.engine.ready()
        }
        assertEquals(4, starts())
        assertTrue(h.playbackRequests().drop(1).all { it.mode == PlaybackMode.Auto && it.startMs == 3_000L })
        h.engine.fail(EngineError.Http(410, "ERROR_CODE_IO_BAD_HTTP_STATUS"))
        h.settle()
        assertEquals(PlayerPhase.Error("Playback stopped and couldn't get going again.", canRetry = true), c.state.value.phase)
        assertEquals(4, starts())

        c.retry()
        h.settle()
        assertEquals(5, starts())
        assertEquals(3_000L, h.playbackRequests().last().startMs)
        h.engine.ready()
        assertEquals(PlayerPhase.Playing, c.state.value.phase)
        // The budget is whole again.
        h.engine.fail(EngineError.Playlist("ERROR_CODE_PARSING_MANIFEST_MALFORMED"))
        h.settle()
        assertEquals(6, starts())
    }

    @Test
    fun `five seconds of playback give every recovery back`() {
        val c = h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.durationMs = 120_000
        repeat(5) {
            h.engine.ready()
            h.advance(6_000)
            h.engine.fail(EngineError.Network("ERROR_CODE_IO_NETWORK_CONNECTION_FAILED"))
            h.settle()
        }
        assertEquals(6, starts())
        assertFalse(c.state.value.phase is PlayerPhase.Error)
    }

    @Test
    fun `network errors start new sessions three times, then say the server can't be reached`() {
        val c = h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        repeat(4) {
            h.engine.ready()
            h.engine.fail(EngineError.Network("ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT"))
            h.settle()
        }
        assertEquals(4, starts())
        assertEquals(PlayerPhase.Error("The server can't be reached.", canRetry = true), c.state.value.phase)
    }

    @Test
    fun `a direct play that stands still with data becomes a transcode after 12 s`() {
        val c = h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.ready()
        h.advance(1_000)
        h.engine.stall()
        h.advance(11_500)
        assertEquals(1, starts())
        h.advance(1_000)
        assertEquals(listOf(PlaybackMode.Auto, PlaybackMode.Transcode), h.playbackRequests().map { it.mode })
        assertEquals(1_000L, h.playbackRequests().last().startMs)
        assertEquals("The original didn't play smoothly on this device.", c.state.value.note)
    }

    @Test
    fun `a stream that stops asks the server how its transcode does, then starts a new session`() {
        h.close()
        h = SessionHarness(caps = capsOf(TestDevices.sdrPhone))
        h.play(PlayRequest(FixtureIds.SAMPLE_HDR10))
        assertTrue(h.engine.source!!.hls)
        h.engine.ready()
        h.advance(2_000)
        h.engine.starve()
        h.advance(12_500)
        // The FixtureServer's transcode runs at 4×: not too slow, so a new session.
        assertEquals(1, h.api.count("playbackStatus"))
        assertEquals(2, starts())
        assertEquals(2_000L, h.playbackRequests().last().startMs)
    }

    @Test
    fun `dropped frames switch to a transcode once, and Play original goes back`() {
        val c = h.play(PlayRequest(FixtureIds.SAMPLE_1080P))
        h.engine.durationMs = 600_000
        h.engine.ready()
        h.engine.frames = FrameCounts(rendered = 0, dropped = 0, maxConsecutiveDropped = 0)
        h.advance(2_000)
        // 240 frames in 10 s, 12 of them dropped: 5 %.
        h.engine.frames = FrameCounts(rendered = 228, dropped = 12, maxConsecutiveDropped = 2)
        h.advance(10_000)
        assertEquals(listOf(PlaybackMode.Auto, PlaybackMode.Transcode), h.playbackRequests().map { it.mode })
        assertTrue(c.state.value.canPlayOriginal)
        assertEquals("The original didn't play smoothly on this device.", c.state.value.note)

        c.playOriginal()
        h.settle()
        assertEquals(PlaybackMode.Auto, h.playbackRequests().last().mode)
        assertFalse(c.state.value.canPlayOriginal)
        assertNull(c.state.value.note)

        // Dropping frames again doesn't switch a second time for this item.
        h.engine.ready()
        h.engine.frames = FrameCounts(rendered = 0, dropped = 0, maxConsecutiveDropped = 0)
        h.advance(2_000)
        h.engine.frames = FrameCounts(rendered = 200, dropped = 40, maxConsecutiveDropped = 20)
        h.advance(12_000)
        assertEquals(3, starts())
    }
}
