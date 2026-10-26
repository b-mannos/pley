package app.pley.android.downloads

import app.pley.android.data.DownloadState
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.MediaFile
import app.pley.android.network.PleyApi
import app.pley.android.testing.network.FakePleyApi
import kotlin.random.Random
import kotlinx.coroutines.runBlocking
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import okio.Buffer
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** A download's file, whole or picked up where it stopped, its failures, and waiting for a network. */
class DownloadRunnerTest {
    @get:Rule
    val folder = TemporaryFolder()

    private val server = MockWebServer()
    private lateinit var api: FakePleyApi
    private lateinit var store: DownloadStore
    private val body = Random(7).nextBytes(300_000)

    @Before
    fun setUp() {
        server.start()
        api = FakePleyApi(server.url("/").toString().trimEnd('/'))
        api.answer("item", detail())
        store = DownloadStore(folder.newFolder("downloads"))
        store.load()
    }

    @After
    fun tearDown() {
        server.close()
    }

    @Test
    fun aFileComesWholeAndIsPlayable() = runBlocking {
        server.enqueue(MockResponse.Builder().code(200).body(Buffer().write(body)).build())
        val saved = add()

        assertFalse("nothing waits", runner().run())

        val done = store.get(saved.key)!!
        assertEquals(DownloadState.Done, done.state)
        assertTrue(done.extrasSaved)
        assertArrayEquals(body, store.mediaOf(done).readBytes())
        assertFalse("the partial file is gone", store.partOf(done).exists())
        assertEquals("/api/files/7/stream", server.takeRequest().url.encodedPath)
    }

    @Test
    fun aFileCutShortGoesOnFromWhereItStopped() = runBlocking {
        val saved = add()
        store.partOf(saved).apply { parentFile!!.mkdirs() }.writeBytes(body.copyOfRange(0, 100_000))
        server.enqueue(
            MockResponse.Builder()
                .code(206)
                .setHeader("Content-Range", "bytes 100000-299999/300000")
                .body(Buffer().write(body.copyOfRange(100_000, body.size)))
                .build(),
        )

        runner().run()

        assertEquals("bytes=100000-", server.takeRequest().headers["Range"])
        val done = store.get(saved.key)!!
        assertEquals(DownloadState.Done, done.state)
        assertArrayEquals(body, store.mediaOf(done).readBytes())
    }

    @Test
    fun aServerThatIgnoresTheRangeSendsItAllAgain() = runBlocking {
        val saved = add()
        store.partOf(saved).apply { parentFile!!.mkdirs() }.writeBytes(ByteArray(50_000))
        server.enqueue(MockResponse.Builder().code(200).body(Buffer().write(body)).build())

        runner().run()

        assertArrayEquals(body, store.mediaOf(store.get(saved.key)!!).readBytes())
    }

    @Test
    fun aFileTheServerNoLongerHasFails() = runBlocking {
        server.enqueue(MockResponse.Builder().code(404).build())
        val saved = add()

        runner().run()

        val failed = store.get(saved.key)!!
        assertEquals(DownloadState.Failed, failed.state)
        assertEquals("This file is no longer on the server", failed.error)
    }

    @Test
    fun withoutANetworkItMayUseDownloadsWait() = runBlocking {
        val saved = add()

        assertTrue("some wait", runner(open = false).run(waitMs = 10))

        assertEquals(DownloadState.Waiting, store.get(saved.key)!!.state)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun theIndexIsReadBackAndACutDownloadWaitsItsTurn() {
        val saved = add()
        store.update(saved.key) { it.copy(state = DownloadState.Downloading) }
        store.partOf(saved).apply { parentFile!!.mkdirs() }.writeBytes(ByteArray(1234))

        val again = DownloadStore(store.dirOf(saved).parentFile!!.parentFile!!.parentFile!!).apply { load() }

        val back = again.get(saved.key)!!
        assertEquals(DownloadState.Queued, back.state)
        assertEquals(1234L, back.downloadedBytes)
    }

    private fun add(): Saved {
        val saved = Saved(
            server = "http://nas:7539",
            userId = 2,
            itemId = 42,
            fileId = 7,
            kind = ItemKind.Movie,
            title = "Alien",
            container = "mkv",
            sizeBytes = body.size.toLong(),
            addedAt = 1,
        )
        store.add(listOf(saved))
        return saved
    }

    private fun runner(open: Boolean = true) = DownloadRunner(
        store = store,
        sources = object : DownloadSources {
            override fun api(server: String, userId: Long): PleyApi = api

            override fun media(server: String) = OkHttpClient()

            override fun subtitles(server: String) = OkHttpClient()

            override val images = OkHttpClient()
        },
        gate = object : NetworkGate {
            override fun open(mobile: Boolean) = open

            override suspend fun await(mobile: Boolean, timeoutMs: Long) = open
        },
        mobileAllowed = { false },
    )

    private fun detail() = ItemDetail(
        id = 42,
        libraryId = 1,
        kind = ItemKind.Movie,
        title = "Alien",
        files = listOf(MediaFile(id = 7, path = "Alien (1979).mkv", size = body.size.toLong(), container = "mkv")),
    )
}
