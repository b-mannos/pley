package app.pley.android.player.session

import app.pley.android.fixtures.FixtureIds
import app.pley.android.player.PlayRequest
import app.pley.android.player.SubtitleSearchUi
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Speed, audio delay and Find subtitles… in a playback, against the FixtureServer. */
@RunWith(RobolectricTestRunner::class)
class PlayerExtrasTest {
    private val h = SessionHarness()

    @After
    fun close() = h.close()

    private fun playing(id: Long = FixtureIds.SHOW_S1E1) = h.play(PlayRequest(id, startMs = 0)).also {
        h.engine.ready()
        h.advance(2_000)
    }

    @Test
    fun `the speed is this title's, kept by a new session and gone with the next title`() {
        val c = playing()
        c.setSpeed(1.5f)
        assertEquals(1.5f, h.engine.speed)
        assertEquals(1.5f, c.state.value.speed)
        // Off the list: the nearest offered.
        c.setSpeed(1.6f)
        assertEquals(1.5f, h.engine.speed)
        c.selectQuality("4m")
        h.settle()
        assertEquals(2, h.engine.prepared.size)
        assertEquals(1.5f, h.engine.speed)
        c.playNext()
        h.settle()
        assertEquals(1f, h.engine.speed)
        assertEquals(1f, c.state.value.speed)
    }

    @Test
    fun `sound passed through to a receiver plays at its own speed, and says so`() {
        val c = playing()
        c.setSpeed(2f)
        h.engine.audioOut = AudioOut.Passthrough
        h.advance(1_000)
        val state = c.state.value
        assertNotNull(state.speedNote)
        assertEquals(1f, state.speed)
        assertEquals(1f, h.engine.speed)
        c.setSpeed(2f)
        assertEquals(1f, h.engine.speed)
        // The delay still moves passed-through sound.
        assertNull(state.audioDelayNote)
        c.setAudioDelay(200)
        assertEquals(200L, h.engine.audioDelayMs)
        h.engine.audioOut = AudioOut.Decoded
        h.advance(1_000)
        assertNull(c.state.value.speedNote)
    }

    @Test
    fun `tunneled, neither the speed nor the delay moves`() {
        val c = playing()
        h.engine.audioOut = AudioOut.Tunneled
        h.advance(1_000)
        assertNotNull(c.state.value.speedNote)
        assertNotNull(c.state.value.audioDelayNote)
        c.setAudioDelay(500)
        assertEquals(0L, h.engine.audioDelayMs)
    }

    @Test
    fun `the audio delay goes in 50 ms steps within 5 s either way, and resets with the next title`() {
        val c = playing()
        c.setAudioDelay(123)
        assertEquals(100L, h.engine.audioDelayMs)
        c.setAudioDelay(-7_000)
        assertEquals(-5_000L, h.engine.audioDelayMs)
        assertEquals(-5_000L, c.state.value.audioDelayMs)
        c.setAudioDelay(5_000)
        c.selectQuality("4m")
        h.settle()
        assertEquals(5_000L, h.engine.audioDelayMs)
        c.playNext()
        h.settle()
        assertEquals(0L, h.engine.audioDelayMs)
        assertEquals(0L, c.state.value.audioDelayMs)
    }

    @Test
    fun `a subtitle found online is added, listed and shown`() {
        val c = playing()
        assertTrue(c.state.value.canFindSubtitles)
        c.searchSubtitles()
        h.settle()
        val found = c.state.value.subtitleSearch as SubtitleSearchUi.Found
        assertEquals(2, found.results.size)
        assertTrue(found.results.first().hint!!.contains("Made for this file"))
        c.addFoundSubtitle(found.results.first().id)
        h.settle()
        val state = c.state.value
        assertNull(state.subtitleSearch)
        assertEquals("Subtitles added", state.notice)
        val added = state.subtitles.single { it.selected }
        assertEquals(5_000L, added.index)
        // Shown as a subtitle file beside the media: the server's WebVTT.
        h.advance(1_000)
        assertTrue(h.api.count("subtitles") >= 1)
        val post = h.journal.entries().single { it.method == "POST" && it.path.endsWith("/subtitles") }
        assertTrue(post.body, post.body.contains("\"result\":\"opensubtitles:"))
    }

    @Test
    fun `a server without subtitle search says it isn't set up`() {
        h.server.subtitleSearch = false
        val c = playing()
        c.searchSubtitles()
        h.settle()
        assertEquals(SubtitleSearchUi.Unavailable("Subtitle search isn't set up on this server"), c.state.value.subtitleSearch)
        c.closeSubtitleSearch()
        assertNull(c.state.value.subtitleSearch)
    }
}
