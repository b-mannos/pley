package app.pley.android.benchmark

import android.content.Intent
import android.view.KeyEvent
import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Makes the app's Baseline and Startup Profiles from its journeys, on an
 * emulator or device with API 33 or later (docs/ANDROID.md): run on purpose,
 * the result committed: cold start, and the TV's journey. The phone's
 * journeys are still to come.
 */
@RunWith(AndroidJUnit4::class)
class BaselineProfileGenerator {
    @get:Rule
    val rule = BaselineProfileRule()

    @Test
    fun startup() = rule.collect(packageName = TARGET, includeInStartupProfile = true) {
        pressHome()
        startActivityAndWait()
    }

    /**
     * The TV, on a TV or a TV emulator, against the demo server of debug and
     * benchmark builds: start, Home's hero and rows, an item page, a
     * library's grid, and the player. Each pass starts from a cleared app,
     * so the demo server runs in the app's process while it is walked.
     */
    @Test
    fun tv() = rule.collect(packageName = TARGET, includeInStartupProfile = true) {
        device.executeShellCommand("pm clear $TARGET")
        pressHome()
        startActivityAndWait(Intent(Intent.ACTION_MAIN).setClassName(TARGET, "app.pley.android.TvActivity"))
        // Choose your server → Use demo server; the code is approved at once.
        pressAndRest(Key.RIGHT, Key.RIGHT, Key.CENTER)
        Thread.sleep(10_000)
        // Home: down into the rows, along them and back, row by row.
        pressAndRest(Key.DOWN)
        repeat(2) {
            repeat(8) { pressAndRest(Key.RIGHT, restMs = 120) }
            repeat(8) { pressAndRest(Key.LEFT, restMs = 120) }
            pressAndRest(Key.DOWN)
        }
        // An item page: open the focused title, go down its rows, come back.
        pressAndRest(Key.CENTER, restMs = 2_000)
        repeat(2) { pressAndRest(Key.DOWN) }
        repeat(4) { pressAndRest(Key.RIGHT, restMs = 150) }
        pressAndRest(Key.BACK, restMs = 1_500)
        // A library: the rail, Movies, the grid.
        pressAndRest(Key.UP, Key.UP, Key.UP)
        pressAndRest(Key.LEFT, Key.DOWN, Key.CENTER, restMs = 1_500)
        pressAndRest(Key.DOWN)
        repeat(5) { pressAndRest(Key.DOWN, restMs = 150) }
        repeat(5) { pressAndRest(Key.UP, restMs = 150) }
        pressAndRest(Key.BACK, restMs = 1_000)
        // The player: Home's Resume, then the controls, then out.
        pressAndRest(Key.LEFT, Key.UP, Key.CENTER, restMs = 2_000)
        pressAndRest(Key.CENTER, restMs = 5_000)
        pressAndRest(Key.DOWN, Key.RIGHT, Key.RIGHT, restMs = 800)
        pressAndRest(Key.BACK, Key.BACK, restMs = 1_500)
    }

    private enum class Key(val code: Int) {
        UP(KeyEvent.KEYCODE_DPAD_UP),
        DOWN(KeyEvent.KEYCODE_DPAD_DOWN),
        LEFT(KeyEvent.KEYCODE_DPAD_LEFT),
        RIGHT(KeyEvent.KEYCODE_DPAD_RIGHT),
        CENTER(KeyEvent.KEYCODE_DPAD_CENTER),
        BACK(KeyEvent.KEYCODE_BACK),
    }

    private fun androidx.benchmark.macro.MacrobenchmarkScope.pressAndRest(vararg keys: Key, restMs: Long = 600) {
        for (key in keys) {
            device.pressKeyCode(key.code)
            Thread.sleep(restMs)
        }
    }
}
