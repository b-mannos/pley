package app.pley.android.ui.tv.player

import android.os.SystemClock
import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.ComponentActivity
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performKeyPress
import androidx.compose.ui.test.pressKey
import app.pley.android.model.Marker
import app.pley.android.model.MarkerKind
import app.pley.android.model.MarkerSource
import app.pley.android.player.ChapterMark
import app.pley.android.player.PlayerPhase
import app.pley.android.player.PlayerUiState
import app.pley.android.player.TrackChoice
import app.pley.android.player.UpNextUi
import app.pley.android.testing.player.engine.FakePlayerEngine
import app.pley.android.testing.player.session.FakePlaybackController
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The TV player with the remote's keys: Centre, Left and Right with the
 * controls hidden, held keys scrubbing and seeking once, Up and Down
 * showing the controls, Back walking out, Skip and Up Next taking focus,
 * the media keys, the panels, the refresh-rate switch and errors.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
@OptIn(ExperimentalTestApi::class)
class TvPlayerScreenTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private var exits = 0

    /** An episode 10 minutes in, an hour long, with subtitles, two audio tracks and chapters. */
    private fun playing(change: (PlayerUiState) -> PlayerUiState = { it }) = FakePlaybackController(
        change(
            PlayerUiState(
                phase = PlayerPhase.Playing,
                itemId = 20_001,
                title = "Fixture Show",
                subtitle = "S1 E1 — Pilot",
                positionMs = 600_000,
                durationMs = 3_600_000,
                bufferedMs = 660_000,
                audio = listOf(TrackChoice(1, "English (AAC Stereo)", true), TrackChoice(2, "French (EAC3 5.1)", false)),
                subtitles = listOf(TrackChoice(null, "Off", true), TrackChoice(3, "English (SRT)", false)),
                chapters = listOf(
                    ChapterMark("Opening", 0, 300_000, null),
                    ChapterMark("The Records", 300_000, 1_800_000, null),
                    ChapterMark("Credits", 1_800_000, 3_600_000, null),
                ),
            ),
        ),
    )

    private fun show(controller: FakePlaybackController) {
        compose.setContent {
            TvTheme {
                TvPlayerScreen(controller, onExit = { exits++ }, now = { 0L })
            }
        }
        compose.waitForIdle()
    }

    private fun press(key: Key, times: Int = 1) {
        repeat(times) {
            compose.onRoot().performKeyInput { pressKey(key) }
            compose.waitForIdle()
        }
    }

    /** Sends one native key event, as a remote's held key does (repeats included). */
    private fun send(action: Int, code: Int, repeat: Int, at: Long) {
        compose.onRoot().performKeyPress(KeyEvent(AndroidKeyEvent(at, at, action, code, repeat)))
        compose.waitForIdle()
    }

    private fun back() {
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
    }

    private fun focusedOn(tag: String) = compose.onNodeWithTag(tag, useUnmergedTree = true).assertIsFocused()

    @Test
    fun `Centre pauses with the controls hidden and shows them on the scrubber, then plays`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionCenter)
        assertEquals(listOf("pause"), controller.calls.map { it.name })
        focusedOn(TAG_SCRUBBER)
        controller.update { it.copy(phase = PlayerPhase.Paused) }
        compose.waitForIdle()
        press(Key.DirectionCenter)
        assertEquals("play", controller.calls.last().name)
    }

    @Test
    fun `Left seeks back 10 s per press, with a badge, and seeks once`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionLeft, times = 3)
        compose.onNodeWithTag(TAG_BADGE).assertExists()
        compose.onNodeWithText("−30 s").assertExists()
        assertTrue("no seek while presses come", controller.calls("seekTo").isEmpty())
        compose.mainClock.advanceTimeBy(TvSeeker.SETTLE_MS + 100)
        compose.waitForIdle()
        assertEquals(listOf(570_000L), controller.calls("seekTo").map { it.args.single() })
        compose.mainClock.advanceTimeBy(TvSeeker.BADGE_MS)
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_BADGE).assertDoesNotExist()
    }

    @Test
    fun `holding Right scrubs through the previews and seeks once on release`() {
        val controller = playing()
        show(controller)
        val start = SystemClock.uptimeMillis()
        send(AndroidKeyEvent.ACTION_DOWN, AndroidKeyEvent.KEYCODE_DPAD_RIGHT, 0, start)
        for (i in 1..40) send(AndroidKeyEvent.ACTION_DOWN, AndroidKeyEvent.KEYCODE_DPAD_RIGHT, i, start + 400 + i * 50L)
        // The scrubber shows with the preview, and nothing has seeked yet, not even after the presses' wait.
        compose.onNodeWithTag(TAG_PREVIEW).assertExists()
        focusedOn(TAG_SCRUBBER)
        compose.mainClock.advanceTimeBy(TvSeeker.SETTLE_MS + 100)
        compose.waitForIdle()
        assertTrue(controller.calls("seekTo").isEmpty())
        send(AndroidKeyEvent.ACTION_UP, AndroidKeyEvent.KEYCODE_DPAD_RIGHT, 0, start + 2_500)
        val seeks = controller.calls("seekTo").map { it.args.single() as Long }
        assertEquals(1, seeks.size)
        assertTrue("held 2 s goes past one step: ${seeks.single()}", seeks.single() > 610_000L + 30_000L)
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        assertEquals(1, controller.calls("seekTo").size)
    }

    @Test
    fun `Up shows the controls on the scrubber, Down on the subtitles`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionUp)
        focusedOn(TAG_SCRUBBER)
        press(Key.DirectionDown)
        focusedOn(TAG_PLAY_PAUSE)
        back()
        press(Key.DirectionDown)
        focusedOn(TAG_SUBTITLES)
    }

    @Test
    fun `Back closes the panel, then hides the controls, then leaves`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionDown)
        press(Key.DirectionCenter)
        compose.onNodeWithTag(TAG_PANEL).assertExists()
        back()
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_PANEL).assertDoesNotExist()
        focusedOn(TAG_SUBTITLES)
        back()
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_SCRUBBER).assertDoesNotExist()
        assertEquals(0, exits)
        back()
        assertEquals(1, exits)
    }

    @Test
    fun `the subtitles panel ticks the current track and chooses another`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionDown)
        press(Key.DirectionCenter)
        compose.onNodeWithText("English (SRT)").assertExists()
        press(Key.DirectionDown)
        press(Key.DirectionCenter)
        assertEquals(listOf(3L), controller.calls("selectSubtitle").map { it.args.single() })
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_PANEL).assertDoesNotExist()
        focusedOn(TAG_SUBTITLES)
    }

    @Test
    fun `Info asks for the playback's details while it is open`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionDown)
        press(Key.DirectionRight, times = 2)
        focusedOn(TAG_INFO)
        press(Key.DirectionCenter)
        assertEquals(true, controller.calls("setDetailsOpen").last().args.single())
        back()
        assertEquals(false, controller.calls("setDetailsOpen").last().args.single())
    }

    @Test
    fun `Skip takes focus while its marker runs and Centre skips`() {
        val controller = playing {
            it.copy(
                positionMs = 3_000,
                skipLabel = "Skip Intro",
                markers = listOf(Marker(1, MarkerKind.Intro, 2_000, 60_000, MarkerSource.Fingerprint)),
            )
        }
        show(controller)
        focusedOn(TAG_SKIP)
        // Left and Right still seek; Centre is Skip's.
        press(Key.DirectionCenter)
        assertEquals(listOf("skip"), controller.calls.map { it.name })
        controller.update { it.copy(skipLabel = null, positionMs = 61_000) }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_SKIP).assertDoesNotExist()
        press(Key.DirectionCenter)
        assertEquals("pause", controller.calls.last().name)
    }

    @Test
    fun `Up Next focuses Play now, which plays next, and Back cancels it`() {
        val controller = playing()
        show(controller)
        press(Key.DirectionUp)
        controller.update { it.copy(upNextCard = UpNextUi(20_002, "Second", "Up next · S1 E2", null, 7)) }
        compose.waitForIdle()
        focusedOn(TAG_PLAY_NOW)
        compose.onNodeWithText("Up next · S1 · E2").assertExists()
        compose.onNodeWithTag(TAG_SCRUBBER).assertDoesNotExist()
        press(Key.DirectionCenter)
        assertEquals("playNext", controller.calls.last().name)
        back()
        assertEquals("cancelUpNext", controller.calls.last().name)
        assertEquals(0, exits)
    }

    @Test
    fun `media keys work whatever shows`() {
        val controller = playing()
        show(controller)
        press(Key.MediaFastForward)
        compose.onNodeWithText("+30 s").assertExists()
        compose.mainClock.advanceTimeBy(TvSeeker.SETTLE_MS + 100)
        compose.waitForIdle()
        assertEquals(listOf(630_000L), controller.calls("seekTo").map { it.args.single() })
        press(Key.MediaPlayPause)
        assertEquals("pause", controller.calls.last().name)
        // Next on an episode plays the next one; Previous goes to the chapter's start.
        press(Key.MediaNext)
        assertEquals("playNext", controller.calls.last().name)
        press(Key.MediaPrevious)
        assertEquals(listOf(630_000L, 300_000L), controller.calls("seekTo").map { it.args.single() })
    }

    @Test
    fun `CAPTIONS turns subtitles on and off, saying so`() {
        val controller = playing()
        show(controller)
        send(AndroidKeyEvent.ACTION_DOWN, AndroidKeyEvent.KEYCODE_CAPTIONS, 0, SystemClock.uptimeMillis())
        assertEquals(listOf(3L), controller.calls("selectSubtitle").map { it.args.single() })
        compose.onNodeWithText("Subtitles: English (SRT)").assertExists()
        controller.update { it.copy(subtitles = listOf(TrackChoice(null, "Off", false), TrackChoice(3, "English (SRT)", true))) }
        compose.waitForIdle()
        send(AndroidKeyEvent.ACTION_DOWN, AndroidKeyEvent.KEYCODE_CAPTIONS, 0, SystemClock.uptimeMillis())
        assertEquals(listOf(3L, null), controller.calls("selectSubtitle").map { it.args.single() })
        compose.onNodeWithText("Subtitles off").assertExists()
    }

    @Test
    fun `the screen says so while the TV matches its refresh rate`() {
        val controller = playing { it.copy(phase = PlayerPhase.Paused) }
        val engine = FakePlayerEngine()
        engine.update { it.copy(switchingDisplayMode = true, switchingToHz = 23.976f) }
        controller.engine = engine
        show(controller)
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        compose.onNodeWithText("Matching refresh rate…").assertExists()
        compose.onNodeWithText("23.976 Hz").assertExists()
        engine.update { it.copy(switchingDisplayMode = false, switchingToHz = null) }
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_SWITCHING).assertDoesNotExist()
    }

    @Test
    fun `an error offers Try again, focused, and Back`() {
        val controller = playing { it.copy(phase = PlayerPhase.Error("The server stopped answering.", canRetry = true)) }
        show(controller)
        compose.onNodeWithText("Can't play this item").assertExists()
        compose.onNodeWithText("The server stopped answering.").assertExists()
        press(Key.DirectionCenter)
        assertEquals("retry", controller.calls.last().name)
        back()
        assertEquals(1, exits)
    }

    @Test
    fun `the playback's notices show as a toast and are dismissed`() {
        val controller = playing { it.copy(notice = "This is the last episode") }
        show(controller)
        compose.onNodeWithText("This is the last episode").assertExists()
        assertTrue(controller.calls("dismissNotice").isNotEmpty())
        compose.mainClock.advanceTimeBy(TvPlayerUi.TOAST_MS + 100)
        compose.waitForIdle()
        compose.onNodeWithTag(TAG_TOAST).assertDoesNotExist()
    }
}
