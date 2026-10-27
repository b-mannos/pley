package app.pley.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider
import app.pley.android.common.Tier
import app.pley.android.designsystem.image.LocalArtworkImageLoader
import app.pley.android.screens.onboarding.Household
import app.pley.android.ui.tv.shell.TvApp
import app.pley.android.ui.tv.shell.TvShellDemo
import app.pley.android.ui.tv.shell.TvShellSource

/**
 * The native app on TVs (Android TV, Google TV, Fire TV). Back goes
 * through the activity's back dispatcher only (Compose's BackHandler; the
 * manifest turns on the system's back callbacks), which Fire OS 16 needs:
 * it can skip onBackPressed. The last Back finishes the activity, not the
 * process: the next one reads again what its first screen is chosen by
 * (PleyApplication.startupFor).
 *
 * Debug builds take two extras, for trying the frame on an emulator:
 * `--es tier T0|T1|T2` overrides the device's tier, and `--ez demo true`
 * shows made-up libraries and rows instead of the app's data.
 *
 * ```
 * adb shell am start -n app.pley.android/.TvActivity --ez demo true --es tier T2
 * ```
 */
class TvActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val app = application as PleyApplication
        val device = app.graph.device
        val tier = debugTier() ?: device.tier
        val demo = BuildConfig.DEBUG && intent.getBooleanExtra(EXTRA_DEMO, false)
        // Made before the content, so what the first screen is chosen by is read
        // (in the background) while the window comes up: the first frame shows it.
        val source = if (demo) {
            null
        } else {
            val read = app.startupFor(savedInstanceState)
            TvShellSource.of(
                app.graph,
                startup = { read.await().let { it.server to it.signedIn } },
                // "Who's watching?" at a cold start needs the household's size, saved on this device.
                householdSize = {
                    val startup = read.await()
                    Household.sizeAtStart(app, { app.graph }, startup.server, startup.signedIn)
                },
            )
        }
        setContent {
            if (source == null) {
                CompositionLocalProvider(LocalArtworkImageLoader provides TvShellDemo.imageLoader(this, device.copy(tier = tier))) {
                    TvApp(TvShellDemo.source, onExit = ::finish, tier = tier, placeholder = TvShellDemo.placeholder)
                }
            } else {
                TvApp(source, onExit = ::finish, tier = tier)
            }
        }
    }

    /** Debug builds: the tier asked for with `--es tier`, or `null`. */
    private fun debugTier(): Tier? {
        if (!BuildConfig.DEBUG) return null
        return intent.getStringExtra(EXTRA_TIER)?.let { name -> Tier.entries.firstOrNull { it.name == name } }
    }

    private companion object {
        const val EXTRA_TIER = "tier"
        const val EXTRA_DEMO = "demo"
    }
}
