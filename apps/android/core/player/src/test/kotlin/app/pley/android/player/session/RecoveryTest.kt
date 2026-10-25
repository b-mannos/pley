package app.pley.android.player.session

import app.pley.android.model.PlaybackMethod
import app.pley.android.model.PlaybackMode
import app.pley.android.player.session.Recovery.Action
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The recovery ladder's transitions and budgets. */
class RecoveryTest {
    private val unsupportedAudio = EngineError.Unsupported(audio = true, container = false, detail = "audio")
    private val unsupportedContainer = EngineError.Unsupported(audio = false, container = true, detail = "container")
    private val unsupportedVideo = EngineError.Unsupported(audio = false, container = false, detail = "video")

    @Test
    fun `direct play falls back to a remux for H_264 and HEVC when only the audio or container failed, then a transcode`() {
        val r = Recovery()
        assertEquals(Action.NewSession(PlaybackMode.Remux, FallbackReason.Unplayable), r.decide(unsupportedAudio, PlaybackMethod.Direct, "hevc"))
        assertEquals(PlaybackMode.Remux, r.fallbackMode)
        // The remux fails too: a transcode, once.
        assertEquals(Action.NewSession(PlaybackMode.Transcode, FallbackReason.Unplayable), r.decide(unsupportedAudio, PlaybackMethod.Remux, "hevc"))
        assertEquals(Action.Fail(Failure.Unplayable, "audio"), r.decide(unsupportedAudio, PlaybackMethod.Transcode, "hevc"))
    }

    @Test
    fun `a container failure of H_264 remuxes, of other video transcodes, as does a video failure`() {
        assertEquals(PlaybackMode.Remux, (Recovery().decide(unsupportedContainer, PlaybackMethod.Direct, "h264") as Action.NewSession).mode)
        assertEquals(PlaybackMode.Transcode, (Recovery().decide(unsupportedContainer, PlaybackMethod.Direct, "vp9") as Action.NewSession).mode)
        assertEquals(PlaybackMode.Transcode, (Recovery().decide(unsupportedVideo, PlaybackMethod.Direct, "h264") as Action.NewSession).mode)
    }

    @Test
    fun `each fallback happens once`() {
        val r = Recovery()
        r.decide(unsupportedVideo, PlaybackMethod.Direct, "h264")
        assertTrue(r.decide(unsupportedVideo, PlaybackMethod.Direct, "h264") is Action.Fail)
        val s = Recovery()
        s.decide(unsupportedAudio, PlaybackMethod.Direct, "h264")
        s.decide(unsupportedAudio, PlaybackMethod.Direct, "h264")
        assertTrue(s.decide(unsupportedAudio, PlaybackMethod.Direct, "h264") is Action.Fail)
    }

    @Test
    fun `expired sessions and network errors get three new sessions each`() {
        val r = Recovery()
        repeat(3) { assertEquals(Action.NewSession(null, null), r.decide(EngineError.Http(404, "x"), PlaybackMethod.Remux, "h264")) }
        assertEquals(Action.Fail(Failure.Stopped, "x"), r.decide(EngineError.Http(410, "x"), PlaybackMethod.Remux, "h264"))
        repeat(3) { assertEquals(Action.NewSession(null, null), r.decide(EngineError.Network("n"), PlaybackMethod.Remux, "h264")) }
        assertEquals(Action.Fail(Failure.Network, "n"), r.decide(EngineError.Http(500, "n"), PlaybackMethod.Remux, "h264"))
        assertTrue(r.decide(EngineError.Other("o"), PlaybackMethod.Remux, "h264") is Action.Fail)
    }

    @Test
    fun `five seconds of progress and Retry give the budgets back`() {
        val r = Recovery()
        repeat(3) { r.decide(EngineError.Playlist("p"), PlaybackMethod.Remux, null) }
        assertEquals(3, r.recreate)
        r.onPosition(10_000)
        r.onPosition(14_999)
        assertEquals(3, r.recreate)
        r.onPosition(15_000)
        assertEquals(0, r.recreate)
        repeat(3) { r.decide(EngineError.Playlist("p"), PlaybackMethod.Remux, null) }
        // A seek: progress counts from where it lands.
        r.onJump()
        r.onPosition(100_000)
        r.onPosition(104_000)
        assertEquals(3, r.recreate)
        r.retry()
        assertEquals(0, r.recreate)
    }

    @Test
    fun `stalls transcode a direct play with data, ask the server about streams, and count`() {
        val r = Recovery()
        assertEquals(Action.NewSession(PlaybackMode.Transcode, FallbackReason.NotSmooth), r.onStall(PlaybackMethod.Direct, starving = false))
        assertEquals(Action.CheckServer, r.onStall(PlaybackMethod.Transcode, starving = true))
        assertEquals(Action.SuggestLowerQuality, r.afterServerCheck(0.8, starving = true))
        assertEquals(Action.NewSession(null, null), r.afterServerCheck(2.0, starving = false))
        assertEquals(Action.Fail(Failure.Stuck, null), r.afterServerCheck(null, starving = false))
        repeat(3) { assertEquals(Action.NewSession(null, null), r.afterServerCheck(null, starving = true)) }
        assertEquals(Action.Fail(Failure.Stopped, null), r.afterServerCheck(null, starving = true))
        // A starving direct play gets new sessions too.
        val s = Recovery()
        assertEquals(Action.NewSession(null, null), s.onStall(PlaybackMethod.Direct, starving = true))
    }

    @Test
    fun `dropped frames switch once per item, Play original goes back, a new item starts over`() {
        val r = Recovery()
        assertNull(r.onStruggling(PlaybackMethod.Transcode))
        assertEquals(Action.NewSession(PlaybackMode.Transcode, FallbackReason.NotSmooth), r.onStruggling(PlaybackMethod.Direct))
        assertTrue(r.struggleSwitched)
        r.playOriginal()
        assertNull(r.fallbackMode)
        assertNull(r.onStruggling(PlaybackMethod.Direct))
        r.newItem()
        assertEquals(Action.NewSession(PlaybackMode.Transcode, FallbackReason.NotSmooth), r.onStruggling(PlaybackMethod.Remux))
    }
}
