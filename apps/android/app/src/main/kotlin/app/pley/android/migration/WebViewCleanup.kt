package app.pley.android.migration

import app.pley.android.common.Addresses
import java.io.File
import kotlin.coroutines.cancellation.CancellationException
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/**
 * What the web view app (pley for Android 0.1) left on a device that was
 * updated to the native app, cleared once:
 *
 * 1. Its session on each saved server ends: the `pley_token` cookie it
 *    signed in with, a full-rights session of 90 days, goes once to POST
 *    /api/auth/logout on that server, as a Bearer token. Best effort: a
 *    server out of reach keeps the session until it expires.
 * 2. Its cookies and web storage are deleted, then its files.
 *
 * Only when its data folder ([files]' first) exists, so a fresh install never
 * starts a web view. Its saved servers and install id were already
 * brought over (core/data, WebViewMigration). Nothing outside this package
 * reads cookies.
 */
class WebViewCleanup(
    /** The web view's data folder (`app_webview`) first, then its other folders and files. */
    private val files: List<File>,
    /** The web view app's saved servers (their addresses). */
    private val servers: () -> List<String>,
    private val webView: WebViewData,
    /** Ends the session [token] holds on [origin]. */
    private val logout: suspend (origin: String, token: String) -> Unit,
    private val mark: DoneMark,
) {
    /** The web view's cookies and storage. */
    interface WebViewData {
        /** The `pley_token` cookie of each of [origins] that has one. */
        suspend fun tokens(origins: List<String>): Map<String, String>

        /** Deletes every cookie and everything web pages stored. */
        suspend fun clear()
    }

    /** Whether the sessions were ended and the web view's data deleted. */
    interface DoneMark {
        fun isDone(): Boolean

        fun setDone()
    }

    /** Runs off the main thread; does nothing on a device the web view app never ran on. */
    suspend fun run() {
        if (files.none { it.exists() }) return
        // A web view that ran in this process can write its files again: then they're only deleted.
        if (!mark.isDone() && files.first().exists()) {
            val origins = servers().mapNotNull(Addresses::origin).distinct()
            val tokens = bestEffort { webView.tokens(origins) }.orEmpty()
            for ((origin, token) in tokens) bestEffort { logout(origin, token) }
            bestEffort { webView.clear() }
        }
        for (file in files) file.deleteRecursively()
        mark.setDone()
    }

    private inline fun <T> bestEffort(block: () -> T): T? = try {
        block()
    } catch (e: CancellationException) {
        throw e
    } catch (_: Exception) {
        // No web view on the device, or the server out of reach.
        null
    }

    companion object {
        /** The cookie the web app's session was kept in. */
        const val SESSION_COOKIE = "pley_token"

        /** The web view's folders and preferences, under the app's data and cache folders. */
        fun files(dataDir: File, cacheDir: File): List<File> = listOf(
            File(dataDir, "app_webview"),
            File(dataDir, "app_textures"),
            File(dataDir, "shared_prefs/WebViewChromiumPrefs.xml"),
            File(dataDir, "shared_prefs/AwOriginVisitLoggerPrefs.xml"),
            File(cacheDir, "WebView"),
            File(cacheDir, "org.chromium.android_webview"),
        )

        /** The session token in a `Cookie` header's worth of cookies, if there is one. */
        fun token(cookies: String?): String? = cookies
            ?.split(';')
            ?.map { it.trim() }
            ?.firstOrNull { it.startsWith("$SESSION_COOKIE=") }
            ?.removePrefix("$SESSION_COOKIE=")
            ?.takeIf { it.isNotEmpty() }

        /** POST /api/auth/logout on [origin] with [token]: the server ends that session. */
        fun logoutRequest(origin: String, token: String): Request = Request.Builder()
            .url("$origin/api/auth/logout")
            .header("Authorization", "Bearer $token")
            .post(ByteArray(0).toRequestBody())
            .build()
    }
}
