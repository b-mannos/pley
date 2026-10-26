package app.pley.android.ui.tv.home

import androidx.activity.ComponentActivity
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.NavKey
import app.pley.android.data.Loadable
import app.pley.android.model.Hubs
import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.network.OfflineException
import app.pley.android.screens.PleyKey
import app.pley.android.screens.home.HomeViewModel
import app.pley.android.screens.home.LibrariesViewModel
import app.pley.android.testing.Fixtures
import app.pley.android.ui.tv.kit.LocalTvRailFocus
import app.pley.android.ui.tv.kit.TvRailFocus
import app.pley.android.ui.tv.shell.LocalTvNavigator
import app.pley.android.ui.tv.shell.LocalTvOffline
import app.pley.android.ui.tv.shell.TvOfflineNotice
import app.pley.android.ui.tv.shell.TvOfflineState
import app.pley.android.ui.tv.shell.RailState
import app.pley.android.ui.tv.shell.TvNavigator
import app.pley.android.ui.tv.theme.TvTheme
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Home on TV with key events: initial focus on Resume, Down into the
 * rows, the focused tile coming back after the page was away, Back from
 * the rows going to the rail with the page back at its hero, and the
 * states drawn from what was saved without the network.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvHomeTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val sources = TestHomeSources().apply {
        home.value = Loadable.Ready(Fixtures.decode<Hubs>("hubs/home.json"), fresh = false)
    }
    private val backStack = mutableListOf<NavKey>(PleyKey.Home)

    private val rail = object : TvRailFocus {
        val requester = FocusRequester()
        var asked = 0
        override var keepsFocus = false

        override fun focusCurrent(): Boolean {
            asked++
            requester.requestFocus()
            return true
        }
    }

    @Composable
    private fun Page(vm: HomeViewModel) {
        TvTheme {
            val navigator = remember { TvNavigator(backStack, RailState(), exit = {}) }
            CompositionLocalProvider(LocalTvNavigator provides navigator, LocalTvRailFocus provides rail) {
                Row(Modifier.fillMaxSize()) {
                    Box(Modifier.size(56.dp).focusRequester(rail.requester).onFocusChanged { rail.keepsFocus = it.hasFocus }.testTag("rail").focusable())
                    TvHomeScreen(vm)
                }
            }
        }
    }

    private fun show() {
        val vm = HomeViewModel(sources, Dispatchers.Unconfined)
        compose.setContent { Page(vm) }
    }

    private fun press(key: Key, times: Int = 1) = repeat(times) {
        compose.onRoot().performKeyInput { pressKey(key) }
        compose.waitForIdle()
    }

    private fun focusedDescription(): String =
        compose.onNode(isFocused()).fetchSemanticsNode().config.getOrElse(SemanticsProperties.ContentDescription) { emptyList() }.joinToString()

    @Test
    fun resumeHasFocusFirstAndDownGoesIntoTheRows() {
        show()
        compose.waitForIdle()
        // Drawn from what was saved: the hero is Continue Watching's first, Severance's "Half Loop".
        compose.onNodeWithText("Severance").assertExists()
        compose.onNodeWithText("S1 · E2 · Half Loop").assertExists()
        compose.onNodeWithText("Resume").assertIsFocused()

        press(Key.DirectionDown)
        assertTrue(focusedDescription(), focusedDescription().startsWith("Half Loop"))
        press(Key.DirectionRight)
        assertTrue(focusedDescription(), focusedDescription().startsWith("Arrival"))
        // Down lands on the nearest tile of the next row: past its one movie, "See all".
        press(Key.DirectionDown)
        assertEquals("See all", focusedDescription())
        press(Key.DirectionLeft)
        assertTrue(focusedDescription(), focusedDescription().startsWith("Dune: Part Two"))
    }

    @Test
    fun savedRowsWithTheServerAwaySaySoUntilItAnswers() {
        val saved = Fixtures.decode<Hubs>("hubs/home.json")
        sources.home.value = Loadable.Failed(OfflineException(IOException("no answer in time")), saved)
        val offline = TvOfflineState()
        val vm = HomeViewModel(sources, Dispatchers.Unconfined)
        compose.setContent {
            CompositionLocalProvider(LocalTvOffline provides offline) {
                Box(Modifier.fillMaxSize()) {
                    Page(vm)
                    TvOfflineNotice(offline)
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Severance").assertExists()
        compose.onNodeWithText("Showing what pley had saved. Reconnecting…").assertIsDisplayed()
        compose.onNodeWithText("Resume").assertIsFocused()

        sources.home.value = Loadable.Ready(saved, fresh = true)
        compose.waitForIdle()
        compose.onNodeWithText("Showing what pley had saved. Reconnecting…").assertDoesNotExist()
    }

    @Test
    fun theFocusedTileComesBackAfterThePageWasAway() {
        val vm = HomeViewModel(sources, Dispatchers.Unconfined)
        val restoration = StateRestorationTester(compose)
        restoration.setContent { Page(vm) }
        compose.waitForIdle()
        press(Key.DirectionDown)
        press(Key.DirectionRight)
        assertTrue(focusedDescription().startsWith("Arrival"))

        restoration.emulateSavedInstanceStateRestore()
        compose.waitForIdle()
        compose.mainClock.advanceTimeBy(100)
        compose.waitForIdle()
        assertTrue(focusedDescription(), focusedDescription().startsWith("Arrival"))
    }

    @Test
    fun backFromTheRowsGoesToTheRailAndTheHeroWaits() {
        show()
        compose.waitForIdle()
        press(Key.DirectionDown, times = 2)
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        assertEquals(1, rail.asked)
        compose.onNodeWithTag("rail").assertIsFocused()
        // Coming back from the rail lands on Resume, not on the row left.
        press(Key.DirectionRight)
        compose.onNodeWithText("Resume").assertIsFocused()
    }

    @Test
    fun selectOpensTheItemAndLongPressItsActions() {
        show()
        compose.waitForIdle()
        press(Key.DirectionDown)
        press(Key.DirectionCenter)
        assertEquals(PleyKey.Item(204), backStack.last())
    }

    @Test
    fun noLibrariesSaysSoInTheWebsWords() {
        sources.home.value = Loadable.Loading
        sources.libraryList.value = Loadable.Ready(emptyList(), fresh = true)
        show()
        compose.waitForIdle()
        compose.onNodeWithText("No libraries yet").assertExists()
        compose.onNodeWithText("Ask the server owner to add a library.").assertExists()
    }

    @Test
    fun anErrorWithNothingSavedOffersRetryWithFocus() {
        sources.home.value = Loadable.Failed(java.io.IOException("nothing answers there"))
        show()
        compose.waitForIdle()
        compose.onNodeWithText("Couldn't load your home screen").assertExists()
        compose.onNodeWithText("Nothing answers there.").assertExists()
        compose.onNodeWithText("Retry").assertIsFocused()
        press(Key.DirectionCenter)
        assertEquals(1, sources.hubRefreshes)
    }

    @Test
    fun theLibrariesPageListsThemAllWithFocusOnTheFirst() {
        sources.libraryList.value = Loadable.Ready(
                listOf(
                    Library(1, "Movies", LibraryKind.Movies, itemCount = 412),
                    Library(2, "Shows", LibraryKind.Shows, itemCount = 38, scanning = true),
                    Library(3, "Music", LibraryKind.Music, itemCount = 120),
                    Library(4, "Concerts", LibraryKind.Movies, itemCount = 12),
                ),
            fresh = true,
        )
        val vm = LibrariesViewModel({ sources.libraries }, { sources.scans }, Dispatchers.Unconfined)
        compose.setContent {
            TvTheme {
                val navigator = remember { TvNavigator(backStack, RailState(), exit = {}) }
                CompositionLocalProvider(LocalTvNavigator provides navigator) { TvLibrariesScreen(vm) }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Libraries").assertExists()
        compose.onNodeWithText("Concerts").assertExists()
        compose.onNodeWithText("38 shows · Scanning…").assertExists()
        compose.onNodeWithText("Movies").assertIsFocused()
        press(Key.DirectionRight)
        press(Key.DirectionCenter)
        assertEquals(PleyKey.Library(2), backStack.last())
    }
}
