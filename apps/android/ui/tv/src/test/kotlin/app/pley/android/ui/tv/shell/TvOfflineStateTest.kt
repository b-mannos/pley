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

    @Test
    fun theServerOutOfReachIsSaidOnceUntilItAnswersAgain() {
        val state = TvOfflineState()
        state.report(Loadable.Failed(away, Unit))
        assertTrue("said, and marked", state.offline && state.announcing)

        state.announced()
        state.report(Loadable.Failed(away, Unit))
        assertEquals("another page out of reach says nothing new", true to false, state.offline to state.announcing)

        state.report(Loadable.Ready(Unit, fresh = false))
        assertEquals("what was saved changes nothing", true, state.offline)

        state.report(Loadable.Ready(Unit, fresh = true))
        assertEquals(false to false, state.offline to state.announcing)

        state.report(Loadable.Failed(away, Unit))
        assertEquals("away again: said again", true to true, state.offline to state.announcing)
    }

    @Test
    fun otherFailuresAreNoServerOutOfReach() {
        val state = TvOfflineState()
        state.report(Loadable.Failed(ApiException(500, "internal", "it broke"), Unit))
        assertEquals(false to false, state.offline to state.announcing)
    }
}
