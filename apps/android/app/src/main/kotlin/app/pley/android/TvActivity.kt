package app.pley.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import app.pley.android.common.perf.PerfMonitor
import app.pley.android.ui.tv.shell.TvApp

/**
 * The native app on TVs (Android TV, Google TV, Fire TV). Back goes
 * through the activity's back dispatcher only (Compose's BackHandler),
 * which Fire OS 16 needs: it can skip onBackPressed.
 */
class TvActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        PerfMonitor.attach(this)
        setContent { TvApp() }
    }
}
