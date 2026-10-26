package app.pley.android.downloads

import app.pley.android.data.DownloadState
import app.pley.android.model.ConversionInfo
import app.pley.android.model.ConversionState
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.MediaFile
import app.pley.android.network.ApiException
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
    fun aFileTheServerMakesForThisPhoneIsWaitedForThenDownloaded() = runBlocking {
        api.answer("convert", ConversionInfo(id = CONVERSION, state = ConversionState.Running, progress = 0.4, url = FILE))
        api.answer("conversion", ConversionInfo(id = CONVERSION, state = ConversionState.Done, progress = 1.0, size = body.size.toLong(), url = FILE))
        server.enqueue(MockResponse.Builder().code(200).body(Buffer().write(body)).build())
        val saved = add(convert = true)

        assertFalse("nothing waits", runner().run())

        val done = store.get(saved.key)!!
        assertEquals(DownloadState.Done, done.state)
        assertArrayEquals(body, store.mediaOf(done).readBytes())
        assertEquals("the server's file", FILE, server.takeRequest().url.encodedPath)
        assertEquals((done.conversion to done.converting), CONVERSION to null)
        // Asked for once, then looked at until whole.
        assertEquals(1, api.calls("convert").size)
        assertEquals(listOf(CONVERSION), api.calls("conversion").map { it.args.single() })
    }

    @Test
    fun theOriginalComesWhenItPlaysHereAfterAll() = runBlocking {
        server.enqueue(MockResponse.Builder().code(200).body(Buffer().write(body)).build())
        val saved = add(convert = true)

        runner().run()

        assertEquals(DownloadState.Done, store.get(saved.key)!!.state)
        assertEquals("/api/files/7/stream", server.takeRequest().url.encodedPath)
    }

    @Test
    fun aServerThatCantConvertGivesTheOriginalUnlessASmallerFileWasAsked() = runBlocking {
        api.fail("convert", ApiException(503, "unavailable", "the transcoder can't convert files"))
        server.enqueue(MockResponse.Builder().code(200).body(Buffer().write(body)).build())
        val playable = add(convert = true)
        runner().run()
        assertEquals(DownloadState.Done, store.get(playable.key)!!.state)

        val smaller = add(convert = true, itemId = 43, maxHeight = 720)
        runner().run()
        val failed = store.get(smaller.key)!!
        assertEquals(DownloadState.Failed, failed.state)
        assertEquals(DownloadRunner.NO_CONVERSIONS, failed.error)
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

    private fun add(convert: Boolean = false, itemId: Long = 42, maxHeight: Int? = null): Saved {
        val saved = Saved(
            server = "http://nas:7539",
            userId = 2,
            itemId = itemId,
            fileId = 7,
            kind = ItemKind.Movie,
            title = "Alien",
            container = "mkv",
            sizeBytes = if (convert) 0 else body.size.toLong(),
            addedAt = 1,
            convert = convert,
            maxHeight = maxHeight,
            maxBitrateKbps = maxHeight?.let { 4_000 },
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
        conversionPollMs = 10,
    )

    private companion object {
        const val CONVERSION = "0123456789abcdef0123456789abcdef"
        const val FILE = "/api/conversions/$CONVERSION/file"
    }

    private fun detail() = ItemDetail(
        id = 42,
        libraryId = 1,
        kind = ItemKind.Movie,
        title = "Alien",
        files = listOf(MediaFile(id = 7, path = "Alien (1979).mkv", size = body.size.toLong(), container = "mkv")),
    )
}
