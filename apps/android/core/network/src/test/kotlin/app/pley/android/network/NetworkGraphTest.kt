package app.pley.android.network

import app.pley.android.auth.Tokens
import app.pley.android.common.Tier
import app.pley.android.testing.network.MockPley
import kotlinx.coroutines.test.runTest
import mockwebserver3.MockResponse
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Request
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** The root client and the clients derived from it. */
class NetworkGraphTest {
    @get:Rule val folder = TemporaryFolder()

    private val noTokens: (String) -> Tokens = { testTokens(it, access = null) }

    @Test
    fun theRootKeepsConnectionsUnderTheServersIdleLimit() {
        val root = HttpRoot.create(TEST_AGENT, testProfile())
        assertEquals(64, root.dispatcher.maxRequests)
        assertEquals(8, root.dispatcher.maxRequestsPerHost)
        assertFalse(root.followSslRedirects)
        assertTrue(root.retryOnConnectionFailure)
        assertTrue(HttpRoot.KEEP_ALIVE_SECONDS < 30)
    }

    @Test
    fun theRootIntroducesItself() {
        MockPley.server().use { server ->
            server.enqueue(MockResponse.Builder().body("ok").build())
            val root = HttpRoot.create(TEST_AGENT, testProfile())
            root.newCall(Request.Builder().url(MockPley.url(server, "/api/health")).header("User-Agent", "okhttp/5").build()).execute().close()
            assertEquals(TEST_AGENT, server.takeRequest().headers["User-Agent"])
        }
    }

    @Test
    fun everyClientSharesTheRootsConnectionsAndThreads() {
        val graph = testGraph(folder.root, noTokens)
        val issuer = "http://192.168.1.5:7539"
        val clients = listOf(
            graph.apiClient(issuer, 2),
            graph.media(issuer),
            graph.subtitles(issuer),
            graph.eventsClient(issuer),
            graph.images,
            graph.probeClient,
        )
        for (client in clients) assertSame(graph.root.connectionPool, client.connectionPool)
        // Pictures have their own limits; everything else counts against the root's.
        for (client in clients - graph.images) assertSame(graph.root.dispatcher, client.dispatcher)
    }

    @Test
    fun timeoutsFollowWhatEachWaitsFor() {
        val graph = testGraph(folder.root, noTokens)
        val issuer = "http://192.168.1.5:7539"
        assertEquals(60_000, graph.media(issuer).readTimeoutMillis)
        assertEquals(60_000, graph.subtitles(issuer).readTimeoutMillis)
        assertEquals(45_000, graph.eventsClient(issuer).readTimeoutMillis)
        assertSame(graph.media(issuer), graph.media("$issuer/"))
        for (client in listOf(graph.media(issuer), graph.subtitles(issuer), graph.apiClient(issuer, 2))) {
            assertTrue(client.followRedirects)
            assertFalse(client.followSslRedirects)
        }
    }

    @Test
    fun picturesHaveTheirOwnLimitsOverHttp11() {
        val phone = testGraph(folder.root, noTokens)
        assertEquals(10, phone.images.dispatcher.maxRequestsPerHost)
        assertEquals(listOf(Protocol.HTTP_1_1), phone.images.protocols)

        val stick = testGraph(folder.root, noTokens, profile = testProfile(Tier.T0))
        assertEquals(6, stick.images.dispatcher.maxRequestsPerHost)
    }

    @Test
    fun picturesCarryTheTokenOfTheirOwnServerOnly() = runTest {
        MockPley.server().use { signedIn ->
            MockPley.server().use { other ->
                val issuer = MockPley.issuerOf(signedIn)
                val tokens = testTokens(issuer, access = "pley_at_pictures")
                val graph = testGraph(folder.root, { if (it == issuer) tokens else testTokens(it, access = null) })
                signedIn.enqueue(MockResponse.Builder().body("jpeg").build())
                other.enqueue(MockResponse.Builder().body("jpeg").build())

                fetch(graph.images, MockPley.url(signedIn, "/api/items/101/images/poster?w=320"))
                fetch(graph.images, MockPley.url(other, "/api/items/101/images/poster?w=320"))

                assertEquals("Bearer pley_at_pictures", signedIn.takeRequest().headers["Authorization"])
                assertNull(other.takeRequest().headers["Authorization"])
            }
        }
    }

    @Test
    fun mediaCarriesTheToken() = runTest {
        MockPley.server().use { server ->
            val issuer = MockPley.issuerOf(server)
            val tokens = testTokens(issuer, access = "pley_at_media")
            val graph = testGraph(folder.root, { tokens })
            server.enqueue(MockResponse.Builder().body("#EXTM3U").build())
            fetch(graph.media(issuer), "$issuer/api/playback/abc/index.m3u8")
            assertEquals("Bearer pley_at_media", server.takeRequest().headers["Authorization"])
        }
    }

    @Test
    fun theTokensAreReadWhenARequestNeedsThem() {
        var read = 0
        val graph = testGraph(folder.root, { read++; testTokens(it) })
        graph.media("http://192.168.1.5:7539")
        graph.api("http://192.168.1.5:7539", 2)
        graph.events("http://192.168.1.5:7539")
        assertEquals(0, read)
    }

    private fun fetch(client: OkHttpClient, url: String) {
        client.newCall(Request.Builder().url(url).build()).execute().close()
    }
}
