package app.pley.android.ui.tv.shell

import app.pley.android.data.Loadable
import app.pley.android.network.ApiException
import app.pley.android.network.OfflineException
import java.io.IOException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** When the TV says its server is out of reach, and for how long. */
class TvOfflineStateTest {
    private val away = OfflineException(IOException("no answer in time"))
    private val failed = Loadable.Failed(away, Unit)

    @Test
    fun theServerOutOfReachIsSaidOnceUntilItAnswersAgain() {
        val state = TvOfflineState()
        state.report(failed, before = null)
        assertTrue("said, and marked", state.offline && state.announcing)

        state.announced()
        state.report(Loadable.Failed(OfflineException(IOException("again")), Unit), before = null)
        assertEquals("another page out of reach says nothing new", true to false, state.offline to state.announcing)

        // Another page's answer from the cache of a minute ago: no sign the server is back.
        state.report(Loadable.Ready(Unit, fresh = true), before = null)
        assertEquals(true, state.offline)
        state.report(Loadable.Ready(Unit, fresh = false), before = failed)
        assertEquals("what was saved changes nothing", true, state.offline)

        // The page that failed has the server's answer.
        state.report(Loadable.Ready(Unit, fresh = true), before = failed)
        assertEquals(false to false, state.offline to state.announcing)

        state.report(failed, before = null)
        assertEquals("away again: said again", true to true, state.offline to state.announcing)
        state.answered()
        assertEquals("the server's events connected", false to false, state.offline to state.announcing)
    }

    @Test
    fun otherFailuresAreNoServerOutOfReach() {
        val state = TvOfflineState()
        state.report(Loadable.Failed(ApiException(500, "internal", "it broke"), Unit), before = null)
        assertEquals(false to false, state.offline to state.announcing)
    }
}
