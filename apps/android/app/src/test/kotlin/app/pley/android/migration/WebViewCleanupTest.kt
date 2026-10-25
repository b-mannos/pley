package app.pley.android.migration

import java.io.File
import java.io.IOException
import java.nio.file.Files
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WebViewCleanupTest {
    private val root: File = Files.createTempDirectory("webview").toFile()
    private val files = WebViewCleanup.files(File(root, "data"), File(root, "cache"))
    private val webView = FakeWebView()
    private val mark = Mark()
    private val loggedOut = mutableListOf<Pair<String, String>>()
    private var servers = listOf("http://nas:7539/", "https://pley.example.com", "http://office:7539")

    private val cleanup = WebViewCleanup(
        files = files,
        servers = { servers },
        webView = webView,
        logout = { origin, token ->
            if (origin.contains("office")) throw IOException("out of reach")
            loggedOut += origin to token
        },
        mark = mark,
    )

    @After
    fun tearDown() {
        root.deleteRecursively()
    }

    private fun leftovers() {
        for (file in files) {
            val leftover = if (file.extension == "xml") file else File(file, "Default/Cookies")
            leftover.apply { parentFile!!.mkdirs() }.writeText("x")
        }
    }

    @Test
    fun aFreshInstallNeverStartsAWebView() = runBlocking {
        cleanup.run()
        assertEquals(0, webView.asked)
        assertFalse(webView.cleared)
        assertFalse(mark.done)
    }

    @Test
    fun theOldSessionsEndAndTheWebViewsDataGoes() = runBlocking {
        leftovers()
        webView.cookies = mapOf(
            "http://nas:7539" to "theme=dark; pley_token=nas-session",
            "https://pley.example.com" to "theme=dark",
            "http://office:7539" to "pley_token=office-session",
        )
        cleanup.run()
        // Each server that holds a session is told once; one out of reach doesn't stop the rest.
        assertEquals(listOf("http://nas:7539" to "nas-session"), loggedOut)
        assertTrue(webView.cleared)
        assertTrue(files.none { it.exists() })
        assertTrue(mark.done)
    }

    @Test
    fun itHappensOnce() = runBlocking {
        leftovers()
        webView.cookies = mapOf("http://nas:7539" to "pley_token=nas-session")
        cleanup.run()
        // A web view started in that process may write its folder again: it is only deleted.
        leftovers()
        cleanup.run()
        assertEquals(1, webView.asked)
        assertEquals(1, loggedOut.size)
        assertTrue(files.none { it.exists() })
    }

    @Test
    fun aDeviceWithoutAWebViewStillLosesTheFolders() = runBlocking {
        leftovers()
        webView.broken = true
        cleanup.run()
        assertTrue(loggedOut.isEmpty())
        assertTrue(files.none { it.exists() })
        assertTrue(mark.done)
    }

    @Test
    fun theSessionCookieIsRead() {
        assertEquals("abc123", WebViewCleanup.token("theme=dark; pley_token=abc123; other=1"))
        assertEquals("abc123", WebViewCleanup.token("pley_token=abc123"))
        assertNull(WebViewCleanup.token("theme=dark"))
        assertNull(WebViewCleanup.token("pley_token="))
        assertNull(WebViewCleanup.token("not_pley_token=abc"))
        assertNull(WebViewCleanup.token(null))
    }

    @Test
    fun theLogoutGoesToTheServerAsABearerToken() {
        val request = WebViewCleanup.logoutRequest("http://nas:7539", "abc123")
        assertEquals("POST", request.method)
        assertEquals("http://nas:7539/api/auth/logout", request.url.toString())
        assertEquals("Bearer abc123", request.header("Authorization"))
        assertNull(request.header("Cookie"))
    }

    private class FakeWebView : WebViewCleanup.WebViewData {
        var cookies = emptyMap<String, String>()
        var asked = 0
        var cleared = false
        var broken = false

        override suspend fun tokens(origins: List<String>): Map<String, String> {
            asked++
            if (broken) throw IllegalStateException("no web view")
            return origins.mapNotNull { o -> WebViewCleanup.token(cookies[o])?.let { o to it } }.toMap()
        }

        override suspend fun clear() {
            if (broken) throw IllegalStateException("no web view")
            cleared = true
        }
    }

    private class Mark : WebViewCleanup.DoneMark {
        var done = false

        override fun isDone() = done

        override fun setDone() {
            done = true
        }
    }
}
