package app.pley.android.fixtures

import okhttp3.OkHttpClient
import okhttp3.Request
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FixtureServerTest {
    @Test
    fun itRunsOnThisMachineOnly() {
        FixtureServer().use { server ->
            server.start()
            assertTrue(server.url, server.url.startsWith("http://127.0.0.1:"))
            OkHttpClient().newCall(Request.Builder().url("${server.url}/api/server").build()).execute().use {
                assertEquals(501, it.code)
            }
        }
    }
}
