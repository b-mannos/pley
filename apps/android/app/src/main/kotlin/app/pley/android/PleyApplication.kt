package app.pley.android

import android.app.Application
import android.os.StrictMode
import androidx.tracing.trace
import app.pley.android.common.TraceNames
import app.pley.android.designsystem.PleyImageLoader
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.ScreensGraph
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import java.util.ServiceLoader
import java.util.concurrent.Callable
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.launch

/**
 * The app. Starting builds nothing: it starts the one disk read the first
 * screen needs and returns (15 ms at most). The rest is made on first use
 * (AppGraph).
 */
class PleyApplication : Application(), PleyGraphOwner, SingletonImageLoader.Factory {
    val graph: AppGraph by lazy { AppGraph(this) }

    /** What the first screen is chosen by (StartupRead). */
    lateinit var startup: Deferred<StartupState>
        private set

    override val screensGraph: ScreensGraph
        get() = graph

    override fun onCreate() {
        trace(TraceNames.APP_ON_CREATE) {
            super.onCreate()
            if (BuildConfig.DEBUG) watchForSlowCalls()
            startup = StartupRead.start(this)
            restartDemoServer()
        }
    }

    /**
     * Builds with the demo server (debug and benchmark builds): it lives in
     * the app's process, so when the device is on it, it starts again with
     * the app, off the main thread, before anything asks it for data.
     */
    private fun restartDemoServer() {
        val type = try {
            Class.forName("app.pley.android.fixtures.DemoServerFactory")
        } catch (_: ClassNotFoundException) {
            return
        }
        val factory = ServiceLoader.load(type, type.classLoader).firstOrNull() as? Callable<*> ?: return
        MainScope().launch(Dispatchers.IO) {
            if (startup.await().server == DEMO_SERVER) runCatching { factory.call() }
        }
    }

    private companion object {
        /** The demo server's address on its usual port (the fixtures' FixtureDemoServerFactory.PORT). */
        const val DEMO_SERVER = "http://127.0.0.1:7740"
    }

    /** Coil's one loader, on the images client; made with the first picture. */
    override fun newImageLoader(context: PlatformContext): ImageLoader =
        PleyImageLoader.create(context, { graph.network.images }, graph.device)

    /** Debug builds log disk and network work on the main thread, and leaks. */
    private fun watchForSlowCalls() {
        StrictMode.setThreadPolicy(StrictMode.ThreadPolicy.Builder().detectAll().penaltyLog().build())
        StrictMode.setVmPolicy(StrictMode.VmPolicy.Builder().detectAll().penaltyLog().build())
    }
}
