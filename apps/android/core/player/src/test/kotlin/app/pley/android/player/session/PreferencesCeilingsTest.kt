package app.pley.android.player.session

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

/** Automatic's ceilings in the app's preferences: one per server, kept across the app's restarts. */
@RunWith(RobolectricTestRunner::class)
class PreferencesCeilingsTest {
    private val app = RuntimeEnvironment.getApplication()

    @Test
    fun `each server keeps its own ceiling, read again by the next run of the app`() {
        val first = PreferencesCeilings(app)
        first.settle()
        first.forServer("http://a:7539").write(4_000, 123)
        // Held at once, written behind.
        assertEquals(4_000 to 123L, first.forServer("http://a:7539").read())
        assertNull(first.forServer("http://b:7539").read())
        first.settle()

        val again = PreferencesCeilings(app)
        again.settle()
        assertEquals(4_000 to 123L, again.forServer("http://a:7539").read())
        assertNull(again.forServer("http://b:7539").read())

        again.forServer("http://a:7539").forget()
        assertNull(again.forServer("http://a:7539").read())
        again.settle()
        val third = PreferencesCeilings(app)
        third.settle()
        assertNull(third.forServer("http://a:7539").read())
    }
}
