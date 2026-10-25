package app.pley.android.benchmark

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Makes the app's Baseline and Startup Profiles from its journeys, on an
 * emulator or device with API 33 or later (docs/ANDROID.md): run on purpose,
 * the result committed. A skeleton: cold start only, until the journeys exist.
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
}
