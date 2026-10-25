package app.pley.android.ui.phone.kit

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.input.key.Key
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.designsystem.tile.rememberSkeleton
import app.pley.android.ui.phone.theme.PhoneTheme
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** How the kit's parts behave: presses, long presses, toasts, the rating, the delayed spinner. */
@RunWith(RobolectricTestRunner::class)
class KitBehaviourTest {
    @get:Rule
    val compose = createComposeRule()

    private val models = (1L..8L).map { ArtworkTileModel(it, "Film $it", "202$it", description = "Film $it, 202$it") }

    @After
    fun forget() {
        KitProbe.cardComposed = null
        KitProbe.rowComposed = null
    }

    @Test
    fun pressingACardRecomposesNeitherItNorItsRow() {
        val cards = mutableListOf<Long>()
        val rows = mutableListOf<String>()
        KitProbe.cardComposed = { synchronized(cards) { cards += it } }
        KitProbe.rowComposed = { synchronized(rows) { rows += it } }
        compose.setContent {
            PhoneTheme {
                HubRow(HubRowModel("films", "Films", TileKind.Poster, models), onOpen = {}, skeleton = rememberSkeleton(animated = false))
            }
        }
        compose.waitForIdle()
        val cardsBefore = synchronized(cards) { cards.size }
        val rowsBefore = synchronized(rows) { rows.size }
        assertTrue(cardsBefore > 0)

        compose.mainClock.autoAdvance = false
        compose.onNodeWithContentDescription("Film 1, 2021").performTouchInput { down(center) }
        compose.mainClock.advanceTimeBy(400)
        compose.onNodeWithContentDescription("Film 1, 2021").performTouchInput { up() }
        compose.mainClock.advanceTimeBy(400)
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()

        assertEquals(cardsBefore, synchronized(cards) { cards.size })
        assertEquals(rowsBefore, synchronized(rows) { rows.size })
    }

    @Test
    fun cardsOpenOnTapAndOfferTheirActionsOnLongPress() {
        val opened = mutableListOf<Long>()
        val actions = mutableListOf<Long>()
        compose.setContent {
            PhoneTheme {
                HubRow(
                    HubRowModel("films", "Films", TileKind.Poster, models),
                    onOpen = { opened += it.id },
                    onLongPress = { actions += it.id },
                    skeleton = rememberSkeleton(animated = false),
                )
            }
        }
        compose.onNodeWithContentDescription("Film 2, 2022").performClick()
        compose.onNodeWithContentDescription("Film 2, 2022").performTouchInput { longClick() }
        compose.runOnIdle {
            assertEquals(listOf(2L), opened)
            assertEquals(listOf(2L), actions)
        }
    }

    @Test
    fun atMostFourToastsShowAndEachGoesInTime() {
        val toasts = ToastHostState()
        compose.mainClock.autoAdvance = false
        compose.setContent { PhoneTheme { ToastHost(toasts) } }
        compose.runOnIdle {
            toasts.show("one")
            toasts.show("two")
            toasts.show("three")
            toasts.show("four", error = true)
            toasts.show("five")
        }
        compose.mainClock.advanceTimeByFrame()
        compose.runOnIdle { assertEquals(listOf("two", "three", "four", "five"), toasts.current.map { it.message }) }
        compose.mainClock.advanceTimeBy(ToastHostState.DURATION_MS + 100)
        compose.runOnIdle { assertEquals(listOf("four"), toasts.current.map { it.message }) }
        compose.mainClock.advanceTimeBy(ToastHostState.ERROR_DURATION_MS - ToastHostState.DURATION_MS)
        compose.runOnIdle { assertEquals(emptyList<String>(), toasts.current.map { it.message }) }
        compose.mainClock.autoAdvance = true
    }

    @Test
    fun aToastsActionRunsAndClosesIt() {
        val toasts = ToastHostState()
        var undone = 0
        compose.setContent { PhoneTheme { ToastHost(toasts) } }
        compose.runOnIdle { toasts.show("Marked as watched", actionLabel = "Undo", onAction = { undone++ }) }
        compose.onNodeWithText("Undo").performClick()
        compose.runOnIdle {
            assertEquals(1, undone)
            assertTrue(toasts.current.isEmpty())
        }
    }

    @Test
    fun theRatingTakesHalfStarsAndKeys() {
        var rating by mutableStateOf<Int?>(null)
        compose.setContent { PhoneTheme { StarRating(rating, { rating = it }) } }
        val stars = compose.onNodeWithContentDescription("Your rating")
        stars.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Not rated"))
        stars.requestFocus()
        stars.performKeyInput { pressKey(Key.DirectionRight) }
        compose.runOnIdle { assertEquals(1, rating) }
        stars.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "0.5 stars"))
        stars.performKeyInput { pressKey(Key.MoveEnd) }
        compose.runOnIdle { assertEquals(10, rating) }
        stars.performKeyInput { pressKey(Key.Delete) }
        compose.runOnIdle { assertEquals(null, rating) }
    }

    @Test
    fun theSpinnerWaitsBeforeShowing() {
        compose.mainClock.autoAdvance = false
        compose.setContent { PhoneTheme { DelayedSpinner(label = "Signing in…") } }
        compose.mainClock.advanceTimeBy(SPINNER_DELAY_MS - 100)
        compose.onNodeWithContentDescription("Signing in…").assertDoesNotExist()
        compose.mainClock.advanceTimeBy(200)
        compose.onNodeWithContentDescription("Signing in…").assertExists()
        compose.mainClock.autoAdvance = true
    }
}
