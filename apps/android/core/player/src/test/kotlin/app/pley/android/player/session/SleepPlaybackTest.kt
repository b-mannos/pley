package app.pley.android.player.session

import app.pley.android.player.PlayRequest
import app.pley.android.player.PlayerPhase
import app.pley.android.player.sleep.SleepChoice
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The sleep timer in a playback: it pauses, and holds what plays next. */
@RunWith(RobolectricTestRunner::class)
class SleepPlaybackTest {
    private val h = SessionHarness()

    @After
    fun close() = h.close()

    @Test
    fun `at the end of the episode it pauses and the next one waits`() {
        // The fixtures' episodes are 20 s long, their credits from 15.5 s.
        val c = h.play(PlayRequest(SessionHarness.S1E1, startMs = 15_500))
        h.engine.durationMs = 20_000
        h.engine.ready()
        c.setSleep(SleepChoice.EndOfItem)
        h.advance(1_000)
        assertEquals(SleepChoice.EndOfItem, c.state.value.sleep.choice)
        // The credits bring Up Next, without its countdown; the notice counts the episode's last seconds.
        assertNotNull(c.state.value.upNextCard)
        assertNull(c.state.value.upNextSecondsLeft)
        assertEquals(4, c.state.value.sleep.secondsLeft)
        h.engine.end()
        h.advance(15_000)
        assertTrue(c.state.value.sleep.fired)
        assertEquals(SleepChoice.Off, c.state.value.sleep.choice)
        assertNull(c.state.value.upNextCard?.secondsLeft)
        assertEquals(1, h.playbackRequests().size)
        assertEquals(PlayerPhase.Paused, c.state.value.phase)
    }

    @Test
    fun `Keep watching lets the next episode come as usual`() {
        val c = h.play(PlayRequest(SessionHarness.S1E1, startMs = 15_500))
        h.engine.durationMs = 20_000
        h.engine.ready()
        c.setSleep(SleepChoice.EndOfItem)
        h.advance(1_000)
        assertNull(c.state.value.upNextSecondsLeft)
        c.keepWatching()
        h.advance(1_000)
        assertNull(c.state.value.sleep.secondsLeft)
        assertNotNull(c.state.value.upNextSecondsLeft)
        h.advance(12_000)
        assertEquals(2, h.playbackRequests().size)
    }

    @Test
    fun `minutes pause the playback, and playing again ends that`() {
        val movie = Catalogue.movieWithTracks(h.api)
        val c = h.play(PlayRequest(movie.id, fromStart = true))
        h.engine.durationMs = movie.files.first().durationMs ?: 7_200_000
        h.engine.ready()
        c.setSleep(SleepChoice.Minutes(15))
        h.advance(14 * 60_000 + 31_000)
        assertEquals(29, c.state.value.sleep.secondsLeft)
        assertTrue(h.engine.playWhenReady)
        h.advance(30_000)
        assertFalse(h.engine.playWhenReady)
        assertTrue(c.state.value.sleep.fired)
        assertEquals(PlayerPhase.Paused, c.state.value.phase)
        c.play()
        h.settle()
        assertFalse(c.state.value.sleep.fired)
    }
}
