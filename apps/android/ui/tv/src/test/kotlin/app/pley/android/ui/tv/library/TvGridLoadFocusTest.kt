package app.pley.android.ui.tv.library

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.designsystem.tile.rememberSkeleton
import app.pley.android.ui.tv.kit.rememberFocusMemory
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A TV grid's focus while its pages load: a cell focused as a skeleton
 * keeps focus when its title arrives, and the D-pad goes on from it.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvGridLoadFocusTest {
    @get:Rule
    val compose = createComposeRule()

    private val tiles = List(120) { ArtworkTileModel(id = it + 1L, title = "Title ${it + 1}", subtitle = "2024") }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun aFocusedSkeletonKeepsFocusWhenItsPageLoads() {
        var loaded by mutableStateOf(false)
        lateinit var focus: TvGridFocus
        compose.setContent {
            TvTheme(reducedMotion = true) {
                focus = remember { TvGridFocus() }
                Box(Modifier.fillMaxSize()) {
                    TvPosterGrid(
                        total = tiles.size,
                        tileAt = { if (loaded) tiles[it] else null },
                        keyAt = { it },
                        kind = TileKind.LibraryGrid,
                        onOpen = { _, _ -> },
                        memory = rememberFocusMemory(),
                        memoryPrefix = "grid",
                        skeleton = rememberSkeleton(animated = false),
                        focus = focus,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onRoot().performKeyInput { repeat(3) { pressKey(Key.DirectionDown) } }
        compose.waitForIdle()
        val before = focus.focused
        assertEquals(TvGridColumnsForTest * 3, before)

        loaded = true
        compose.waitForIdle()
        compose.onAllNodes(isFocused()).assertCountEquals(1)
        assertEquals(before, focus.focused)

        compose.onRoot().performKeyInput { pressKey(Key.DirectionRight) }
        compose.waitForIdle()
        assertEquals(before + 1, focus.focused)
    }

    private companion object {
        /** The library grid's columns on TV. */
        val TvGridColumnsForTest = tvGridColumns(TileKind.LibraryGrid)
    }
}
