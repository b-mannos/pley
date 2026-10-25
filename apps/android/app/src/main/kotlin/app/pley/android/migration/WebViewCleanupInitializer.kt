package app.pley.android.migration

import android.app.Activity
import android.app.Application
import android.content.Context
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.WebStorage
import androidx.core.content.edit
import androidx.startup.Initializer
import app.pley.android.PleyApplication
import app.pley.android.data.LegacyPreferences
import app.pley.android.data.SavedServersJson
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Runs [WebViewCleanup] once the first activity has drawn its first frame,
 * off the main thread. Starting the app only registers a callback: the
 * disk is looked at after the first frame. AndroidX Startup's provider
 * runs it (the app's manifest).
 */
class WebViewCleanupInitializer : Initializer<Unit> {
    override fun create(context: Context) {
        val app = context.applicationContext as Application
        app.registerActivityLifecycleCallbacks(
            object : ActivityResumed() {
                override fun onActivityResumed(activity: Activity) {
                    app.unregisterActivityLifecycleCallbacks(this)
                    // Posted while the first frame is pending: runs once it is drawn.
                    activity.window.decorView.post {
                        CoroutineScope(SupervisorJob() + Dispatchers.IO).launch { cleanup(app).run() }
                    }
                }
            },
        )
    }

    override fun dependencies(): List<Class<out Initializer<*>>> = emptyList()

    private fun cleanup(app: Application) = WebViewCleanup(
        files = WebViewCleanup.files(app.dataDir, app.cacheDir),
        servers = {
            val legacy = LegacyPreferences.of(app).get(LegacyPreferences.SERVERS_FILE, LegacyPreferences.SERVERS_KEY)
            SavedServersJson.decode(legacy).map { it.url }
        },
        webView = SystemWebView,
        logout = { origin, token ->
            // Only to the server whose cookie it is; a redirect is not followed.
            val client = (app as PleyApplication).graph.root.newBuilder()
                .followRedirects(false)
                .callTimeout(LOGOUT_TIMEOUT_S, TimeUnit.SECONDS)
                .build()
            client.newCall(WebViewCleanup.logoutRequest(origin, token)).execute().close()
        },
        mark = object : WebViewCleanup.DoneMark {
            private val prefs = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

            override fun isDone(): Boolean = prefs.getBoolean(DONE, false)

            override fun setDone() = prefs.edit { putBoolean(DONE, true) }
        },
    )

    /** The system's web view: started, and asked, on the main thread. */
    private object SystemWebView : WebViewCleanup.WebViewData {
        override suspend fun tokens(origins: List<String>): Map<String, String> = withContext(Dispatchers.Main) {
            val cookies = CookieManager.getInstance()
            origins.mapNotNull { origin -> WebViewCleanup.token(cookies.getCookie(origin))?.let { origin to it } }.toMap()
        }

        override suspend fun clear() = withContext(Dispatchers.Main) {
            val cookies = CookieManager.getInstance()
            withTimeoutOrNull(CLEAR_TIMEOUT_MS) {
                suspendCancellableCoroutine { done -> cookies.removeAllCookies { done.resume(Unit) } }
            }
            WebStorage.getInstance().deleteAllData()
            cookies.flush()
        }
    }

    /** Only [onActivityResumed] matters here. */
    private abstract class ActivityResumed : Application.ActivityLifecycleCallbacks {
        override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit

        override fun onActivityStarted(activity: Activity) = Unit

        override fun onActivityPaused(activity: Activity) = Unit

        override fun onActivityStopped(activity: Activity) = Unit

        override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit

        override fun onActivityDestroyed(activity: Activity) = Unit
    }

    private companion object {
        const val PREFS = "migration"
        const val DONE = "web_view_cleared"
        const val LOGOUT_TIMEOUT_S = 10L
        const val CLEAR_TIMEOUT_MS = 5_000L
    }
}
