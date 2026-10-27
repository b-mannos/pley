package app.pley.android.ui.tv.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Left and Right: presses that add up and seek once, and a held key that scrubs and seeks on release. */
class TvSeekerTest {
    private var position = 600_000L
    private val duration = 3_600_000L
    private val seeks = mutableListOf<Long>()
    private val seeker = TvSeeker({ position }, { duration }, { seeks += it })

    @Test
    fun `presses in a row add up and seek once when they stop`() {
        seeker.press(1)
        seeker.press(1)
        seeker.press(1)
        assertEquals(630_000L, seeker.target)
        assertEquals(30_000L, seeker.badgeMs)
        assertTrue(seeks.isEmpty())
        seeker.settle(seeker.presses - 1)
        assertTrue("an earlier press's wait doesn't seek", seeks.isEmpty())
        seeker.settle(seeker.presses)
        assertEquals(listOf(630_000L), seeks)
        assertNull(seeker.target)
    }

    @Test
    fun `a held key scrubs faster the longer it's held and seeks once on release`() {
        seeker.press(1)
        var t = 1_000L
        seeker.repeat(1, t)
        assertTrue(seeker.holding)
        repeat(100) {
            t += 50
            seeker.repeat(1, t)
        }
        val target = seeker.target!!
        assertTrue("5 s held goes well past one step: $target", target > 610_000L + 150_000L)
        seeker.settle(seeker.presses)
        assertTrue("nothing seeks while the key is held", seeks.isEmpty())
        assertTrue(seeker.release())
        assertEquals(listOf(target), seeks)
        assertFalse(seeker.holding)
        assertFalse(seeker.release())
        assertEquals(1, seeks.size)
    }

    @Test
    fun `OK confirms a seek to come or one just made, once`() {
        assertFalse("no seek: OK plays or pauses", seeker.confirm())
        seeker.press(1)
        assertTrue(seeker.confirm())
        assertEquals(listOf(610_000L), seeks)
        assertNull(seeker.target)
        assertFalse("confirmed: the next OK pauses", seeker.confirm())

        seeker.press(1)
        seeker.settle(seeker.presses)
        assertTrue("a seek just made", seeker.confirm())
        assertFalse(seeker.confirm())

        seeker.press(1)
        seeker.settle(seeker.presses)
        seeker.settled(seeker.seeks - 1)
        assertTrue("an earlier seek's wait doesn't end this one's", seeker.confirm())

        seeker.press(1)
        seeker.settle(seeker.presses)
        seeker.settled(seeker.seeks)
        assertFalse("long after the seek", seeker.confirm())
        assertEquals(4, seeks.size)
    }

    @Test
    fun `seeks stay inside the video`() {
        position = 5_000
        seeker.press(-1)
        assertEquals(0L, seeker.target)
        seeker.cancel()
        position = duration - 2_000
        seeker.press(1)
        assertEquals(duration - TvSeeker.END_MARGIN_MS, seeker.target)
    }

    @Test
    fun `the badge goes after its own press only`() {
        seeker.press(-1)
        val first = seeker.presses
        seeker.press(-1)
        seeker.hideBadge(first)
        assertEquals(-20_000L, seeker.badgeMs)
        seeker.hideBadge(seeker.presses)
        assertNull(seeker.badgeMs)
    }
}
