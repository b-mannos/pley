package app.pley.android.ui.tv.account

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.navigation3.runtime.NavKey
import app.pley.android.common.Tier
import app.pley.android.screens.PleyKey
import app.pley.android.ui.tv.kit.focused
import app.pley.android.ui.tv.kit.press
import app.pley.android.ui.tv.settings.TvAccountTestApplication
import app.pley.android.ui.tv.settings.TvAccountTestGraph
import app.pley.android.ui.tv.shell.LocalTvNavigator
import app.pley.android.ui.tv.shell.RailState
import app.pley.android.ui.tv.shell.TvNavigator
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Watch history with the D-pad: removing a play, from its cross or with a
 * long press, gives focus to the same place in the play below it, or
 * above it for the last, never to nothing.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi", application = TvAccountTestApplication::class)
class TvHistoryFocusTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val graph = TvAccountTestGraph()

    @Before
    fun graph() {
        TvAccountTestApplication.graph = graph
    }

    private fun show() {
        val navigator = TvNavigator(mutableListOf<NavKey>(PleyKey.Settings, PleyKey.History), RailState()) {}
        compose.setContent {
            TvTheme(Tier.T2, reducedMotion = true) {
                CompositionLocalProvider(LocalTvNavigator provides navigator) { TvHistoryScreen() }
            }
        }
        compose.waitUntil(5_000) { compose.onAllNodesWithText("Film 1").fetchSemanticsNodes().isNotEmpty() }
    }

    @Test
    fun theCrossRemovesAPlayAndFocusGoesToTheNextOnesCross() {
        show()
        compose.focused().assert(hasText("Film 1"))
        compose.press(Key.DirectionRight)
        compose.focused().assert(hasContentDescription("Remove Film 1 from history"))
        compose.press(Key.DirectionCenter)
        compose.waitUntil(5_000) { graph.removed.isNotEmpty() }
        assertEquals(listOf(1L), graph.removed)
        compose.waitForIdle()
        compose.focused().assert(hasContentDescription("Remove Film 2 from history"))
    }

    @Test
    fun removingTheLastPlayShownGivesFocusToTheOneAbove() {
        graph.history.value = app.pley.android.data.Loadable.Ready(
            app.pley.android.model.HistoryPage(2, listOf(TvAccountTestGraph.play(1), TvAccountTestGraph.play(2))),
            fresh = true,
        )
        show()
        compose.press(Key.DirectionDown)
        compose.focused().assert(hasText("Film 2"))
        compose.press(Key.DirectionRight)
        compose.press(Key.DirectionCenter)
        compose.waitUntil(5_000) { graph.removed.isNotEmpty() }
        compose.waitForIdle()
        compose.focused().assert(hasContentDescription("Remove Film 1 from history"))
    }
}
