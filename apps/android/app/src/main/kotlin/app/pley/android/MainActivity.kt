package app.pley.android

import android.app.UiModeManager
import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import android.os.SystemClock
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import app.pley.android.common.perf.PerfMonitor
import app.pley.android.common.perf.StartupSignals
import app.pley.android.ui.phone.shell.PhoneApp

/**
 * The native app on phones and tablets. A TV that opens it is sent to
 * TvActivity. The splash screen stays only while the startup read runs,
 * 300 ms at most: nothing here waits for the network.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        val splash = installSplashScreen()
        super.onCreate(savedInstanceState)
        if (isTelevision()) {
            startActivity(Intent(this, TvActivity::class.java))
            finish()
            return
        }
        val shownAt = SystemClock.uptimeMillis()
        splash.setKeepOnScreenCondition {
            !StartupSignals.startupRead.value && SystemClock.uptimeMillis() - shownAt < SPLASH_LIMIT_MS
        }
        enableEdgeToEdge()
        PerfMonitor.attach(this)
        setContent { PhoneApp() }
    }

    private fun isTelevision(): Boolean =
        getSystemService(UiModeManager::class.java)?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION

    private companion object {
        const val SPLASH_LIMIT_MS = 300L
    }
}
