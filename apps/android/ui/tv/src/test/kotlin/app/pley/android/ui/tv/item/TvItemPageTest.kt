package app.pley.android.ui.tv.item

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.navigation3.runtime.NavKey
import app.pley.android.common.TestTags
import app.pley.android.common.Tier
import app.pley.android.screens.PleyKey
import app.pley.android.screens.item.ItemViewModel
import app.pley.android.ui.tv.shell.LocalTvNavigator
import app.pley.android.ui.tv.shell.RailState
import app.pley.android.ui.tv.shell.TvNavigator
import app.pley.android.ui.tv.theme.TvTheme
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The TV item page with the remote (D-pad rules 1 and 6, design spec §2.7):
 * focus starts on Play or Resume; the Audio and subtitles panel takes focus,
 * keeps it, saves a choice, and Back closes it with focus back on its button.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
@OptIn(ExperimentalTestApi::class)
class TvItemPageTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun show(data: ItemData) {
        val model = ItemViewModel(data, data.launcher, 1, compute = Dispatchers.Main.immediate)
        val stack = mutableListOf<NavKey>(PleyKey.Home, PleyKey.Item(1))
        val navigator = TvNavigator(stack, RailState()) {}
        compose.setContent {
            TvTheme(Tier.T2, reducedMotion = true) {
                CompositionLocalProvider(LocalTvNavigator provides navigator) { TvItemScreen(model) }
            }
        }
        compose.waitForIdle()
    }

    private fun press(key: Key, times: Int = 1) = repeat(times) {
        compose.onRoot().performKeyInput { pressKey(key) }
        compose.waitForIdle()
    }

    @Test
    fun focusStartsOnPlay() {
        show(ItemData(movie()))
        compose.onNodeWithTag(TestTags.ITEM_PLAY).assertIsFocused()
        compose.onNodeWithText("Play").assertExists()
    }

    @Test
    fun aMovieInProgressStartsOnResume() {
        show(ItemData(movie(positionMs = 60_000)))
        compose.onNodeWithTag(TestTags.ITEM_PLAY).assertIsFocused()
        compose.onNodeWithText("Resume").assertExists()
    }

    @Test
    fun theTracksPanelTakesFocusSavesAChoiceAndBackClosesIt() {
        val data = ItemData(movie())
        show(data)
        // Play → Watched → Audio and subtitles.
        press(Key.DirectionRight, 2)
        compose.onNode(isFocused()).assert(hasContentDescription("Audio and subtitles"))
        press(Key.Enter)
        compose.onNodeWithTag(TV_PANEL).assertExists()
        // Focus starts on the current audio track, and stays in the panel.
        compose.onNode(isFocused()).assertExists()
        press(Key.DirectionLeft, 3)
        compose.onNode(isFocused()).assert(hasText("English (TrueHD 7.1)"))
        press(Key.DirectionDown)
        press(Key.Enter)
        assertEquals(listOf(listOf<Long?>(1, 11, 2, null)), data.streams)
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
        compose.onNode(isFocused()).assert(hasContentDescription("Audio and subtitles"))
    }
}
