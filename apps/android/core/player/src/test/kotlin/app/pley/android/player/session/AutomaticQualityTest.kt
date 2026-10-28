package app.pley.android.player.session

import app.pley.android.fixtures.FixtureIds
import app.pley.android.model.NetworkReport
import app.pley.android.model.PlaybackMode
import kotlinx.coroutines.runBlocking
import app.pley.android.player.PlayRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Automatic (docs/QUALITY.md) in a playback, against the FixtureServer, with the fake engine and virtual time. */
@RunWith(RobolectricTestRunner::class)
class AutomaticQualityTest {
    private var h = SessionHarness(quality = null)

    @After
    fun close() = h.close()

    private fun use(harness: SessionHarness) {
        h.close()
        h = harness
    }

    /** A long episode (1080p, 6.5 Mbps: Automatic goes down to 4 or 2 Mbps), 20 s into it. */
    private fun playing(id: Long = EPISODE) = h.play(PlayRequest(id, startMs = 0)).also {
        h.engine.ready()
        h.advance(20_000)
    }

    /** The video waits for data until Automatic steps down. */
    private fun starveUntilLowered() {
        h.engine.starve()
        h.advance(2_500)
    }

    @Test
    fun `nothing chosen plays Automatic, which starts at the original`() {
        val c = playing()
        val state = c.state.value
        assertEquals("Automatic · Original", state.quality)
        assertEquals(listOf("original", "auto"), state.qualities.take(2).map { it.id })
        assertTrue(state.qualities.first { it.id == "auto" }.selected)
        assertEquals("Dynamically adjust to conditions", state.qualities.first { it.id == "auto" }.hint)
        val asked = h.playbackRequests().single()
        assertEquals(PlaybackMode.Auto, asked.mode)
        assertNull(asked.maxBitrate)
    }

    @Test
    fun `a rebuffer steps down once, at the position, and says so once`() {
        val c = playing()
        starveUntilLowered()
        val requests = h.playbackRequests()
        assertEquals(2, requests.size)
        val asked = requests.last()
        assertEquals(PlaybackMode.Transcode, asked.mode)
        // Nothing arrived while it waited: as low as it goes.
        assertEquals(2_000, asked.maxBitrate)
        assertEquals(480, asked.maxHeight)
        assertTrue(asked.startMs!! in 19_000L..21_000L)
        // The server picks the audio again, for a stream: one it copies.
        assertNull(asked.audioIndex)
        val state = c.state.value
        assertEquals("Automatic · 2 Mbps 480p", state.quality)
        assertEquals("Quality lowered to 2 Mbps 480p", state.notice)
        assertEquals("Lowered for your connection", state.note)
        assertFalse(state.canPlayOriginal)
        assertTrue(state.qualities.first { it.id == "auto" }.selected)
        assertEquals("Now 2 Mbps 480p", state.qualities.first { it.id == "auto" }.hint)
        c.dismissNotice()
        // The new session's own buffering is no play issue, and nothing starts twice.
        h.advance(5_000)
        h.engine.ready()
        h.advance(30_000)
        assertEquals(2, h.playbackRequests().size)
        assertNull(c.state.value.notice)
        // The details say why.
        c.setDetailsOpen(true)
        h.advance(2_500)
        assertEquals("Lowered for your connection", c.state.value.details!!.why)
    }

    /** Plays on for [ms], each new session loading at once. */
    private fun playOn(ms: Long) {
        var left = ms
        while (left > 0) {
            if (h.engine.status == EngineStatus.Buffering) h.engine.ready()
            h.advance(5_000)
            left -= 5_000
        }
    }

    @Test
    fun `it steps back up to the original after calm minutes on a fast network, silently`() {
        val c = playing()
        starveUntilLowered()
        c.dismissNotice()
        h.engine.bandwidthKbps = 100_000
        playOn(4 * 60_000L)
        // Not within 5 minutes of the step down.
        assertEquals(2, h.playbackRequests().size)
        playOn(15 * 60_000L)
        val asked = h.playbackRequests()
        assertEquals(PlaybackMode.Auto, asked.last().mode)
        assertNull(asked.last().maxBitrate)
        assertEquals("Automatic · Original", c.state.value.quality)
        assertNull(c.state.value.notice)
        assertNull(c.state.value.note)
    }

    @Test
    fun `without a measure of the network it stays down`() {
        playing()
        starveUntilLowered()
        h.engine.ready()
        h.advance(10 * 60_000L)
        assertEquals(2, h.playbackRequests().size)
    }

    @Test
    fun `the next episode starts at the ceiling found`() {
        val c = playing()
        starveUntilLowered()
        h.engine.ready()
        c.playNext()
        h.settle()
        val asked = h.playbackRequests().last()
        assertEquals(NEXT_EPISODE, asked.itemId)
        assertEquals(PlaybackMode.Transcode, asked.mode)
        assertEquals(2_000, asked.maxBitrate)
        assertEquals("Automatic · 2 Mbps 480p", c.state.value.quality)
    }

    @Test
    fun `seeking and pausing aren't play issues`() {
        val c = playing()
        c.seekTo(60_000)
        h.engine.starve()
        h.advance(6_000)
        h.engine.ready()
        h.advance(5_000)
        c.pause()
        h.engine.starve()
        h.advance(10_000)
        assertEquals(1, h.playbackRequests().size)
        assertEquals("Automatic · Original", c.state.value.quality)
    }

    @Test
    fun `Original never lowers for the network`() {
        use(SessionHarness(quality = "original"))
        val c = playing()
        h.engine.starve()
        h.advance(8_000)
        assertEquals(1, h.playbackRequests().size)
        assertEquals("Original", c.state.value.quality)
    }

    @Test
    fun `picking Original while Automatic plays the original plays on, and is remembered`() {
        val c = playing()
        c.selectQuality("original")
        h.settle()
        assertEquals(1, h.playbackRequests().size)
        assertEquals(listOf("original"), h.remembered)
        assertEquals("Original", c.state.value.quality)
        // Automatic again: nothing to change either; then it watches again.
        c.selectQuality("auto")
        h.settle()
        assertEquals(1, h.playbackRequests().size)
        assertEquals(listOf("original", "auto"), h.remembered)
        h.advance(5_000)
        starveUntilLowered()
        assertEquals(2, h.playbackRequests().size)
    }

    @Test
    fun `dropped frames at the original go to the server's quality once, as Automatic's step down`() {
        val c = playing()
        h.engine.frames = FrameCounts(rendered = 500, dropped = 0, maxConsecutiveDropped = 0)
        h.advance(2_500)
        h.engine.frames = FrameCounts(rendered = 560, dropped = 12, maxConsecutiveDropped = 12)
        h.advance(2_500)
        val asked = h.playbackRequests()
        assertEquals(2, asked.size)
        assertEquals(PlaybackMode.Transcode, asked.last().mode)
        // The server's own quality for the file.
        assertNull(asked.last().maxBitrate)
        val state = c.state.value
        assertEquals("Automatic · 8 Mbps 1080p", state.quality)
        assertEquals("Quality lowered to 8 Mbps 1080p", state.notice)
        assertEquals("Lowered for this device", state.note)
        // Automatic has Original in its menu: no second offer to play it.
        assertFalse(state.canPlayOriginal)
    }

    // ---------------------------------------------------------------- what the server knows of the network

    /** The bodies of PUT /api/playback/network, in order. */
    private fun networkReports(): List<String> = h.journal.writes("PUT", "/api/playback/network").map { it.body }

    @Test
    fun `on a network the server knows, Automatic starts at most at its ceiling`() {
        h.server.networkRemote = true
        runBlocking { h.api.reportNetwork(NetworkReport.Ceiling(SessionHarness.CLIENT_ID, 4_000)) }
        val c = playing()
        val asked = h.playbackRequests().single()
        assertEquals(PlaybackMode.Transcode, asked.mode)
        assertEquals(4_000, asked.maxBitrate)
        assertEquals("Automatic · 4 Mbps 720p", c.state.value.quality)
        assertEquals("Lowered for your connection", c.state.value.note)
        assertEquals(1, h.api.count("playbackNetwork"))
        assertEquals(0, h.api.count("speedTest"))
    }

    @Test
    fun `a network the server doesn't know gets a speed test first, reported`() {
        h.server.networkRemote = true
        val c = playing()
        assertEquals(1, h.api.count("speedTest"))
        val report = networkReports().single()
        assertTrue(report, report.contains("\"probe_kbps\""))
        // Loopback brings it all at once: no limit, the original.
        assertEquals("Automatic · Original", c.state.value.quality)
        assertEquals(PlaybackMode.Auto, h.playbackRequests().single().mode)
    }

    @Test
    fun `a step down is told to the server, and Automatic picked again forgets it`() {
        val c = playing()
        starveUntilLowered()
        assertEquals("""{"client_id":"android-test","ceiling_kbps":2000}""", networkReports().single())
        c.selectQuality("original")
        h.settle()
        c.selectQuality("auto")
        h.settle()
        assertEquals("""{"client_id":"android-test","ceiling_kbps":null}""", networkReports().last())
        assertEquals("Automatic · Original", c.state.value.quality)
    }

    @Test
    fun `an older server leaves the ceiling to this device, as before`() {
        h.server.networkRoutes = false
        val c = playing()
        starveUntilLowered()
        h.engine.ready()
        c.playNext()
        h.settle()
        val asked = h.playbackRequests().last()
        assertEquals(NEXT_EPISODE, asked.itemId)
        assertEquals(2_000, asked.maxBitrate)
        // Asked once: the routes aren't there, so nothing more goes to them.
        assertEquals(1, h.api.count("playbackNetwork"))
        assertTrue(networkReports().isEmpty())
    }

    private companion object {
        /** The second show's first episode, after the Fixture Show's six (its episodes last 22 minutes or more). */
        const val EPISODE: Long = FixtureIds.SHOW_S1E1 + 6
        const val NEXT_EPISODE: Long = EPISODE + 1
    }
}
