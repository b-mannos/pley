package app.pley.android.ui.tv.kit

import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.SemanticsNodeInteractionsProvider
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import app.pley.android.designsystem.tile.ArtworkTileModel

/** Presses [key] on the remote, as a D-pad does: down and up. */
@OptIn(ExperimentalTestApi::class)
internal fun ComposeContentTestRule.press(key: Key, times: Int = 1) {
    repeat(times) {
        onRoot().performKeyInput { pressKey(key) }
        waitForIdle()
    }
}

/** The node that has focus now. */
internal fun SemanticsNodeInteractionsProvider.focused(): SemanticsNodeInteraction = onNode(isFocused())

/** Made-up tiles [from] to [from] + [count] - 1, titled "<prefix> <id>". */
internal fun tiles(prefix: String, count: Int, from: Long = 1): List<ArtworkTileModel> =
    (from until from + count).map { ArtworkTileModel(id = it, title = "$prefix $it", subtitle = "2024") }
