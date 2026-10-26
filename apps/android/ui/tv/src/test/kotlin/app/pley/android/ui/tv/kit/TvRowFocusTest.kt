package app.pley.android.ui.tv.kit

import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * D-pad rules 2 and 3 on a page of rows, with key events: up and down
 * between rows, Left from a row's first tile into the rail, Right stopping
 * at the end, and each row coming back to the tile it had.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvRowFocusTest {
    @get:Rule
    val compose = createComposeRule()

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

    private lateinit var list: LazyListState

    private fun page(withHero: Boolean = false) {
        compose.setContent {
            list = rememberLazyListState()
            TvTheme {
                CompositionLocalProvider(LocalTvRailFocus provides rail) {
                    Row(Modifier.fillMaxSize()) {
                        Box(
                            Modifier
                                .size(56.dp)
                                .focusRequester(rail.requester)
                                .onFocusChanged { rail.keepsFocus = it.hasFocus }
                                .semantics { contentDescription = "rail" }
                                .testTag("rail")
                                .focusable(),
                        )
                        TvRowColumn(
                            state = list,
                            contentPadding = PaddingValues(top = if (withHero) 0.dp else 172.dp),
                            heroHeight = if (withHero) 400.dp else 0.dp,
                            modifier = Modifier.fillMaxSize(),
                        ) {
                            if (withHero) {
                                item("hero") {
                                    Column(Modifier.height(400.dp)) {
                                        Spacer(Modifier.height(341.dp))
                                        Box(
                                            Modifier
                                                .padding(start = 40.dp)
                                                .size(148.dp, 48.dp)
                                                .testTag("resume")
                                                .initialFocus()
                                                .focusable(),
                                        )
                                    }
                                }
                            }
                            item("a") { TvRow("a", "Row A", tiles("A", 8), TileKind.Landscape, onOpen = {}, initialFocus = !withHero) }
                            item("b") { TvRow("b", "Row B", tiles("B", 8, from = 100), TileKind.Poster, onOpen = {}) }
                        }
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun theFirstTileTakesFocusFirst() {
        page()
        compose.focused().assertContentDescriptionEquals("A 1")
    }

    @Test
    fun downFromTheHeroEntersTheRowsAndUpScrollsBackToIt() {
        page(withHero = true)
        compose.onNodeWithTag("resume").assertIsFocused()
        compose.press(Key.DirectionDown)
        compose.focused().assertContentDescriptionEquals("A 1")
        // The row's header is at y = 172: the hero (400 dp) scrolled 400 + 28 - 172.
        compose.mainClock.advanceTimeBy(2_000)
        compose.waitForIdle()
        assertEquals(0, list.firstVisibleItemIndex)
        assertEquals(with(compose.density) { 256.dp.roundToPx() }.toFloat(), list.firstVisibleItemScrollOffset.toFloat(), 2f)
        compose.press(Key.DirectionUp)
        compose.onNodeWithTag("resume").assertIsFocused()
        compose.mainClock.advanceTimeBy(2_000)
        compose.waitForIdle()
        assertEquals(0, list.firstVisibleItemScrollOffset)
    }

    @Test
    fun rightMovesAlongAndStopsAtTheEnd() {
        page()
        compose.press(Key.DirectionRight, times = 3)
        compose.focused().assertContentDescriptionEquals("A 4")
        compose.press(Key.DirectionRight, times = 10)
        compose.focused().assertContentDescriptionEquals("A 8")
    }

    @Test
    fun leftFromTheFirstTileEntersTheRail() {
        page()
        compose.press(Key.DirectionRight)
        compose.press(Key.DirectionLeft)
        compose.focused().assertContentDescriptionEquals("A 1")
        compose.press(Key.DirectionLeft)
        assertEquals(1, rail.asked)
        compose.onNodeWithTag("rail").assertIsFocused()
    }

    @Test
    fun leftFromAScrolledRowsFirstTileStillGoesToTheRail() {
        page()
        // Row A scrolled along, then down to row B's first tile: row A's tiles are under the rail.
        compose.press(Key.DirectionRight, times = 4)
        compose.press(Key.DirectionDown)
        compose.focused().assertContentDescriptionEquals("B 100")
        compose.press(Key.DirectionLeft)
        compose.onNodeWithTag("rail").assertIsFocused()
    }

    @Test
    fun eachRowComesBackToItsTile() {
        page()
        compose.press(Key.DirectionRight, times = 2)
        compose.focused().assertContentDescriptionEquals("A 3")
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionRight, times = 3)
        compose.focused().assertContentDescriptionEquals("B 103")
        compose.press(Key.DirectionUp)
        compose.focused().assertContentDescriptionEquals("A 3")
        compose.press(Key.DirectionDown)
        compose.focused().assertContentDescriptionEquals("B 103")
    }
}

