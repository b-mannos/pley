package app.pley.android.player.session

import app.pley.android.model.MediaFile
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** Automatic's decisions (docs/QUALITY.md), on a playback simulated half a second at a time. */
class AutoQualityTest {
    private val memory = CeilingMemory.memory()
    private val auto = AutoQuality(memory)

    /** The clock; far from 0, so a memory from "before" is in the past. */
    private var now = 10_000_000L
    private var position = 0L
    private var buffered = 30_000L
    private var duration = 3_600_000L

    private fun file(bitrate: Long, width: Int, height: Int) = MediaFile(id = 1, path = "a", bitrate = bitrate, width = width, height = height)

    private fun autoFile(f: MediaFile) = AutoFile(Qualities.forFile(f), Qualities.fileKbps(f), Qualities.linesOf(f), Qualities.closest(f))

    /** 1080p at 10 Mbps: offered 8, 4 and 2 Mbps; the server's quality is 12 Mbps 1080p. */
    private val hd = file(10_000_000, 1920, 1080)

    /** A 4K remux at 60 Mbps: offered everything; the server's quality is 30 Mbps 4K. */
    private val uhd = file(60_000_000, 3840, 2160)

    private fun begin(f: MediaFile): QualityPreset = auto.begin(autoFile(f), now)

    /**
     * Plays for [ms]: the playhead moves unless [waiting], and video arrives
     * at [rate] seconds a second. The first step asked for, or `null`.
     */
    private fun play(
        ms: Long,
        rate: Double = 1.0,
        waiting: Boolean = false,
        watching: Boolean = true,
        bandwidth: Long? = null,
        apply: Boolean = true,
    ): AutoStep? {
        var left = ms
        while (left > 0) {
            now += TICK
            left -= TICK
            if (watching && !waiting) position += TICK
            buffered = (buffered + (rate * TICK).toLong()).coerceAtMost(duration).coerceAtLeast(position)
            val step = auto.sample(AutoSample(now, watching, waiting, position, buffered, duration, bandwidth))
            if (step != null) {
                if (apply) applied(step)
                return step
            }
        }
        return null
    }

    /** The player plays [step] now: a new session, which buffers before it plays. */
    private fun applied(step: AutoStep) {
        auto.applied(step, now)
        settle()
    }

    /** A start, a seek, or a new session: a moment of buffering, which never counts, then [ahead] loaded. */
    private fun settle(ahead: Long = 30_000) {
        auto.jumped()
        assertNull(play(3_000, waiting = true))
        buffered = position + ahead
        assertNull(play(TICK))
    }

    private fun started(f: MediaFile, ahead: Long = 30_000): QualityPreset = begin(f).also { settle(ahead) }

    private fun ids(list: List<QualityPreset>) = list.map { it.id }

    private fun <T> present(value: T?): T = value ?: throw AssertionError("expected a value")

    // ---------------------------------------------------------------- the ladder and the start

    @Test
    fun `starts at the original, with the file's qualities up to the server's below it`() {
        assertSame(Qualities.ORIGINAL, begin(uhd))
        assertNull(auto.loweredFor)
        // 4K first, then 1080p and below, as the menu says them.
        assertEquals(listOf("30m", "15m", "20m", "12m", "8m", "4m", "2m"), ids(auto.ladder()))
        begin(hd)
        assertEquals(listOf("8m", "4m", "2m"), ids(auto.ladder()))
        // 1080p at 40 Mbps: the server converts it at 20 Mbps 1080p, so nothing above.
        begin(file(40_000_000, 1920, 1080))
        assertEquals(listOf("20m", "15m", "12m", "8m", "4m", "2m"), ids(auto.ladder()))
    }

    @Test
    fun `the server's quality for a file follows the server's rule`() {
        assertEquals("30m", Qualities.closest(uhd).id)
        assertEquals("12m", Qualities.closest(hd).id)
        assertEquals("20m", Qualities.closest(file(40_000_000, 1920, 1080)).id)
        // 4K at 6 Mbps: 4K's lowest.
        assertEquals("15m", Qualities.closest(file(6_000_000, 3840, 2160)).id)
        assertEquals("4m", Qualities.closest(file(3_000_000, 1280, 720)).id)
        assertEquals("20m", Qualities.closest(null).id)
    }

    @Test
    fun `the buffering of a start or a seek and waits while paused are no play issue`() {
        begin(hd)
        assertNull(play(10_000, waiting = true))
        assertNull(play(5_000))
        // Paused, the video waits for nothing.
        assertNull(play(10_000, waiting = true, watching = false))
        auto.jumped()
        assertNull(play(6_000, waiting = true))
        assertNull(play(5_000))
        assertSame(Qualities.ORIGINAL, auto.playing)
    }

    // ---------------------------------------------------------------- the triggers

    @Test
    fun `a wait of 2 s steps down`() {
        started(hd)
        assertNull(play(20_000))
        assertNull(play(1_500, waiting = true))
        val step = present(play(1_000, waiting = true))
        assertEquals(LoweredFor.Network, step.reason)
        assertFalse(step.up)
        assertEquals(LoweredFor.Network, auto.loweredFor)
    }

    @Test
    fun `two short waits within a minute step down, further apart they don't`() {
        started(hd)
        assertNull(play(10_000))
        assertNull(play(1_000, waiting = true))
        assertNull(play(70_000))
        assertNull(play(1_000, waiting = true))
        assertNull(play(TICK))
        assertNull(play(40_000))
        assertNull(play(1_000, waiting = true))
        present(play(TICK))
    }

    @Test
    fun `video arriving slower than it plays, with little ahead and falling, steps down before it waits`() {
        started(hd, ahead = 12_000)
        // 0.5 s a second: ahead falls under 10 s after 4 s, and it is a play issue 15 s later.
        assertNull(play(18_000, rate = 0.5))
        val step = present(play(3_000, rate = 0.5))
        assertEquals(LoweredFor.Network, step.reason)
        // 10 Mbps × 0.5 × 0.75 = 3.75 Mbps: 2 Mbps 480p.
        assertEquals("2m", step.to.id)
    }

    @Test
    fun `slow delivery with plenty ahead, steady, or loaded to the end is no play issue`() {
        // 30 s ahead: it has time.
        started(hd)
        assertNull(play(30_000, rate = 0.8))
        // 5 s ahead and steady (a buffer full by its bytes).
        started(hd, ahead = 5_000)
        assertNull(play(40_000, rate = 1.0))
        // Loaded to the end: nothing more arrives, nothing is late.
        started(hd, ahead = 8_000)
        duration = buffered
        assertNull(play(7_000, rate = 0.0))
    }

    // ---------------------------------------------------------------- the formula

    @Test
    fun `the ceiling is the bitrate playing times the delivery seen times 0_75`() {
        started(hd)
        // 0.6 s a second for 20 s, then a wait: 10 Mbps × 0.6 × 0.75 = 4.5 Mbps.
        assertNull(play(20_000, rate = 0.6))
        assertEquals("4m", play(3_000, rate = 0.6, waiting = true)!!.to.id)
        assertEquals(4_000, memory.read()!!.first)
    }

    @Test
    fun `it plays the highest of the menu under the ceiling, 4K first`() {
        started(uhd)
        // 60 Mbps × 0.5 × 0.75 = 22.5 Mbps: 15 Mbps 4K, above 20 Mbps 1080p in the menu.
        assertNull(play(20_000, rate = 0.5))
        assertEquals("15m", play(3_000, rate = 0.5, waiting = true)!!.to.id)
    }

    @Test
    fun `never above the server's quality for the file`() {
        started(uhd)
        // 60 Mbps × 1.2 × 0.75 = 54 Mbps: 30 Mbps 4K, the server's own.
        assertNull(play(20_000, rate = 1.2))
        assertEquals("30m", play(3_000, rate = 1.2, waiting = true)!!.to.id)
    }

    @Test
    fun `never below 2 Mbps 480p`() {
        started(hd)
        assertNull(play(20_000, rate = 0.1))
        assertEquals("2m", play(3_000, rate = 0.1, waiting = true)!!.to.id)
        // Already there: nothing lower, the recovery ladder takes over.
        now += AutoQuality.CHANGE_GAP_MS
        assertNull(play(20_000, waiting = true))
        assertEquals("2m", auto.playing.id)
    }

    @Test
    fun `a step down is one quality at least, even on a fast network`() {
        started(hd)
        assertNull(play(20_000, rate = 1.0))
        assertEquals("8m", play(3_000, rate = 3.0, waiting = true)!!.to.id)
        assertNull(play(30_000, rate = 3.0))
        assertEquals("4m", play(3_000, rate = 3.0, waiting = true)!!.to.id)
    }

    @Test
    fun `changes are 20 s apart at least`() {
        started(hd)
        assertNull(play(20_000, rate = 1.2))
        assertEquals("8m", play(3_000, rate = 1.2, waiting = true)!!.to.id)
        // A wait right after the change: too soon, it waits on.
        assertNull(play(5_000))
        assertNull(play(10_000, waiting = true))
        // 20 s after the change: the same wait steps down now.
        assertEquals("4m", play(10_000, waiting = true)!!.to.id)
    }

    // ---------------------------------------------------------------- the device

    @Test
    fun `dropped frames at a transcode go to fewer lines, and never back up to it`() {
        started(uhd)
        assertNull(play(20_000, rate = 0.5))
        assertEquals("15m", play(3_000, rate = 0.5, waiting = true)!!.to.id)
        now += AutoQuality.CHANGE_GAP_MS
        val step = present(auto.deviceFellBehind(now))
        assertEquals("20m", step.to.id)
        assertEquals(LoweredFor.Device, step.reason)
        applied(step)
        assertEquals(LoweredFor.Device, auto.loweredFor)
        // Calm and a fast network: 1080p stays, 4K made the device fall behind.
        assertNull(play(10 * 60_000L, bandwidth = 200_000))
        assertEquals("20m", auto.playing.id)
    }

    @Test
    fun `dropped frames at the original are the recovery ladder's, and the original stays behind`() {
        started(hd)
        assertNull(auto.deviceFellBehind(now))
        auto.deviceFellBehindAtOriginal(now, convertedKbps = 12_000)
        assertEquals(LoweredFor.Device, auto.loweredFor)
        settle()
        assertNull(play(10 * 60_000L, bandwidth = 200_000))
        assertSame(Qualities.ORIGINAL, auto.playing)
        // Dropped frames again, at the server's conversion: one step lower.
        assertEquals("8m", auto.deviceFellBehind(now)!!.to.id)
    }

    @Test
    fun `a server too slow to convert is a play issue`() {
        started(hd)
        auto.originalPlaysAt(12_000, converted = true)
        assertEquals("8m", auto.serverTooSlow(now)!!.to.id)
    }

    // ---------------------------------------------------------------- stepping back up

    private fun downTo4m() {
        started(hd)
        assertNull(play(20_000, rate = 0.6))
        assertEquals("4m", play(3_000, rate = 0.6, waiting = true)!!.to.id)
    }

    @Test
    fun `steps back up one at a time after calm minutes with 1_6 times the next quality`() {
        downTo4m()
        // Calm for 3 minutes, but not 5 minutes after the step down yet.
        assertNull(play(4 * 60_000L + 50_000, bandwidth = 20_000))
        val up = present(play(60_000, bandwidth = 20_000))
        assertEquals("8m", up.to.id)
        assertTrue(up.up)
        assertNull(up.reason)
        assertEquals(8_000, memory.read()!!.first)
        // The next one 3 minutes later at least: the original, at 1.6 × 10 Mbps.
        assertNull(play(2 * 60_000L + 50_000, bandwidth = 20_000))
        val original = present(play(60_000, bandwidth = 20_000))
        assertSame(Qualities.ORIGINAL, original.to)
        assertNull(auto.loweredFor)
        // Back at the original, the ceiling is forgotten.
        assertNull(memory.read())
    }

    @Test
    fun `it stays down without the headroom, a measure, or calm`() {
        downTo4m()
        // 1.6 × 8 Mbps is 12.8: 12 isn't enough.
        assertNull(play(10 * 60_000L, bandwidth = 12_000))
        // A network it can't measure.
        assertNull(play(10 * 60_000L, bandwidth = null))
        // A dip during the calm minutes.
        assertNull(play(2 * 60_000L, bandwidth = 50_000))
        assertNull(play(5_000, bandwidth = 9_000))
        assertNull(play(2 * 60_000L, bandwidth = 50_000))
        assertEquals("4m", auto.playing.id)
    }

    @Test
    fun `pauses don't count as calm`() {
        downTo4m()
        assertNull(play(10 * 60_000L, bandwidth = 50_000, watching = false))
        assertNull(play(2 * 60_000L, bandwidth = 50_000))
        assertEquals("4m", auto.playing.id)
        assertEquals("8m", play(2 * 60_000L, bandwidth = 50_000)!!.to.id)
    }

    @Test
    fun `a play issue starts the calm over`() {
        downTo4m()
        assertNull(play(6 * 60_000L, bandwidth = 5_000))
        // Two short waits: back to 2 Mbps; the calm counts from there.
        assertNull(play(1_000, waiting = true, bandwidth = 50_000))
        assertNull(play(10_000, bandwidth = 50_000))
        assertNull(play(1_000, waiting = true, bandwidth = 50_000))
        assertEquals("2m", play(TICK, bandwidth = 50_000)!!.to.id)
        assertNull(play(4 * 60_000L, bandwidth = 50_000))
        assertEquals("4m", play(2 * 60_000L, bandwidth = 50_000)!!.to.id)
    }

    // ---------------------------------------------------------------- the memory

    @Test
    fun `the next title starts at the remembered ceiling for 30 minutes`() {
        downTo4m()
        assertEquals(4_000, memory.read()!!.first)
        now += 60_000
        // A new title (Up Next's): at most what the network carried.
        assertEquals("4m", begin(uhd).id)
        assertEquals(LoweredFor.Network, auto.loweredFor)
        // A file below the ceiling plays as it is.
        assertSame(Qualities.ORIGINAL, begin(file(3_000_000, 1280, 720)))
        // 30 minutes on, forgotten.
        now += AutoQuality.MEMORY_MS
        assertSame(Qualities.ORIGINAL, begin(uhd))
        assertNull(memory.read())
    }

    @Test
    fun `a ceiling from the future is from before the device restarted, and forgotten`() {
        memory.write(4_000, now + 60_000)
        assertSame(Qualities.ORIGINAL, begin(hd))
        assertNull(memory.read())
    }

    @Test
    fun `a ceiling under every quality starts at the lowest`() {
        memory.write(1_000, now)
        assertEquals("2m", begin(uhd).id)
    }

    @Test
    fun `stopped, it watches nothing`() {
        started(hd)
        auto.stop()
        assertFalse(auto.active)
        assertNull(play(10_000, waiting = true))
        assertNull(auto.deviceFellBehind(now))
        assertNull(auto.serverTooSlow(now))
    }

    private companion object {
        const val TICK = 500L
    }
}
