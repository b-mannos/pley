package app.pley.android.ui.tv.kit

import android.view.KeyEvent as NativeKeyEvent
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performKeyPress
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tiles: Select opens, holding Select gives the actions (rule 6), and a
 * held direction key moves one tile per 100 ms, jumping without animation
 * (rule 5).
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvTileTest {
    @get:Rule
    val compose = createComposeRule()

    private var now = 50_000L
    private val gate = KeyRepeatGate { now }
    private val opened = mutableListOf<Long>()
    private val actions = mutableListOf<Long>()

    private fun row() {
        compose.setContent {
            TvTheme {
                CompositionLocalProvider(LocalKeyRepeatGate provides gate) {
                    Row(Modifier.fillMaxSize().keyRepeatGate(gate)) {
                        TvRow(
                            rowKey = "r",
                            title = "Row",
                            tiles = tiles("T", 20),
                            kind = TileKind.Poster,
                            onOpen = { opened += it.id },
                            onActions = { actions += it.id },
                            initialFocus = true,
                            state = rememberLazyListState(),
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun selectOpensTheTile() {
        row()
        compose.press(Key.DirectionCenter)
        assertEquals(listOf(1L), opened)
        assertTrue(actions.isEmpty())
    }

    @Test
    fun holdingSelectOpensTheActions() {
        row()
        compose.onRoot().performKeyInput {
            keyDown(Key.DirectionCenter)
            advanceEventTime(1_000)
            keyUp(Key.DirectionCenter)
        }
        compose.waitForIdle()
        assertEquals(listOf(1L), actions)
        assertTrue(opened.isEmpty())
    }

    private fun hold(code: Int, repeat: Int) {
        compose.onRoot().performKeyPress(KeyEvent(NativeKeyEvent(0, now, NativeKeyEvent.ACTION_DOWN, code, repeat)))
        compose.waitForIdle()
    }

    @Test
    fun aHeldKeyMovesOneTilePer100Ms() {
        row()
        val right = NativeKeyEvent.KEYCODE_DPAD_RIGHT
        hold(right, 0)
        compose.focused().assertContentDescriptionEquals("T 2")
        now += 400
        // Ten repeats 50 ms apart: five moves.
        for (i in 1..10) {
            hold(right, i)
            now += 50
        }
        compose.onRoot().performKeyPress(KeyEvent(NativeKeyEvent(0, now, NativeKeyEvent.ACTION_UP, right, 0)))
        compose.waitForIdle()
        compose.focused().assertContentDescriptionEquals("T 7")
    }
}
