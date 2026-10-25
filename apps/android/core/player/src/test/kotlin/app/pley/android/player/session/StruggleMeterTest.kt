package app.pley.android.player.session

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StruggleMeterTest {
    private fun counts(rendered: Long, dropped: Long, run: Int = 0) = FrameCounts(rendered, dropped, run)

    @Test
    fun `over 1 percent dropped in 10 s struggles, 1 percent or less doesn't`() {
        val m = StruggleMeter()
        assertFalse(m.sample(0, counts(0, 0)))
        assertFalse(m.sample(5_000, counts(118, 2)))
        // 240 frames over 10 s, 2 dropped: under 1 %.
        assertFalse(m.sample(10_000, counts(238, 2)))
        // The next 10 s: 240 frames, 5 dropped.
        assertFalse(m.sample(15_000, counts(356, 4)))
        assertTrue(m.sample(20_000, counts(473, 7)))
    }

    @Test
    fun `needs 10 s of samples`() {
        val m = StruggleMeter()
        m.sample(0, counts(0, 0))
        assertFalse(m.sample(9_000, counts(100, 50)))
        assertTrue(m.sample(10_000, counts(110, 50)))
    }

    @Test
    fun `ten frames dropped in a row struggle at once`() {
        val m = StruggleMeter()
        m.sample(0, counts(0, 0, run = 3))
        assertTrue(m.sample(2_000, counts(48, 12, run = 10)))
    }

    @Test
    fun `counts that start over with a new decoder start the window over`() {
        val m = StruggleMeter()
        m.sample(0, counts(1_000, 100))
        assertFalse(m.sample(10_000, counts(10, 0)))
        assertFalse(m.sample(15_000, counts(120, 0)))
        m.reset()
        assertFalse(m.sample(20_000, counts(240, 50)))
    }
}
