package app.pley.android.ui.tv.settings

import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.unit.dp
import app.pley.android.common.Tier
import app.pley.android.ui.tv.kit.focused
import app.pley.android.ui.tv.kit.press
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A choice panel opens on its current choice even when that choice is
 * further down than the panel shows (a profile's Spanish among some
 * thirty languages): it scrolls to it, and focus is there, not left on
 * the row behind the panel.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvChoicePanelTest {
    @get:Rule
    val compose = createComposeRule()

    private val choices = (0 until 35).map { TvChoice(it, "Language $it") }

    private fun panel(selected: Int, picked: (Int) -> Unit = {}) {
        val behind = FocusRequester()
        var open by mutableStateOf(false)
        compose.setContent {
            TvTheme(Tier.T2, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    Box(Modifier.size(40.dp).focusRequester(behind).focusable())
                    TvChoicePanel(open, "Subtitle language", choices, selected, onPick = picked, onDismiss = { open = false }, returnTo = behind)
                }
            }
        }
        // Focus is behind the panel, on the row that opens it, as in Settings.
        compose.runOnIdle { behind.requestFocus() }
        compose.runOnIdle { open = true }
        compose.waitForIdle()
    }

    @Test
    fun aChoiceFarDownTheListTakesFocus() {
        panel(selected = 28)
        compose.focused().assert(hasText("Language 28"))
        compose.press(Key.DirectionDown)
        compose.focused().assert(hasText("Language 29"))
    }

    @Test
    fun aChoiceInViewStillTakesFocus() {
        panel(selected = 2)
        compose.focused().assert(hasText("Language 2"))
    }

    @Test
    fun selectPicksTheFocusedChoice() {
        var picked: Int? = null
        panel(selected = 30) { picked = it }
        compose.press(Key.DirectionUp)
        compose.press(Key.DirectionCenter)
        assertEquals(29, picked)
    }
}
