@file:OptIn(ExperimentalCoroutinesApi::class)

package app.pley.android.player.session

import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.DefaultPlaybackCapsPolicy
import app.pley.android.data.LocalMedia
import app.pley.android.fixtures.FixtureIds
import app.pley.android.fixtures.FixtureServer
import app.pley.android.fixtures.Journal
import app.pley.android.model.ItemDetail
import app.pley.android.model.MaybeItem
import app.pley.android.model.PlaybackInfo
import app.pley.android.model.PlaybackRequest
import app.pley.android.model.ServerEvent
import app.pley.android.model.PleyJson
import app.pley.android.model.ProgressReport
import app.pley.android.model.SessionStatus
import app.pley.android.model.StreamChoice
import app.pley.android.model.TrickplayInfo
import app.pley.android.network.HttpPleyApi
import app.pley.android.network.OfflineException
import app.pley.android.network.PleyApi
import app.pley.android.network.SubtitleText
import app.pley.android.player.PlayRequest
import app.pley.android.player.PlayerUiState
import app.pley.android.player.subtitles.ApiSubtitleFetcher
import app.pley.android.player.subtitles.PictureSource
import app.pley.android.testing.caps.FakeDeviceCapabilities
import app.pley.android.testing.caps.TestDevices
import app.pley.android.testing.caps.capsOf
import app.pley.android.testing.player.session.FakeEngineLink
import java.net.ConnectException
import java.util.Collections
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import okhttp3.OkHttpClient
import org.junit.Assert.fail
import org.robolectric.RuntimeEnvironment

/**
 * [PleyApi] counting the calls on their way: a call counts until the
 * coroutine that made it goes on, so [SessionHarness.settle] knows when the
 * FixtureServer has answered everything.
 */
class CountingApi(private val delegate: PleyApi) : PleyApi by delegate {
    val inFlight = AtomicInteger()

    /** The calls made, by name, in order. */
    val names: MutableList<String> = Collections.synchronizedList(mutableListOf())

    /** How many calls of [name] were made. */
    fun count(name: String): Int = synchronized(names) { names.count { it == name } }

    /** Calls that failed, with their route. */
    val failures = mutableListOf<Pair<String, Throwable>>()

    /** Calls (by name) that fail as if the server were away. */
    val unreachable: MutableSet<String> = Collections.synchronizedSet(mutableSetOf())

    private suspend fun <T> counted(name: String, block: suspend () -> T): T {
        inFlight.incrementAndGet()
        names += name
        try {
            if (name in unreachable) throw OfflineException(ConnectException("$name: the server is away"))
            return block()
        } catch (e: Exception) {
            synchronized(failures) { failures += name to e }
            throw e
        } finally {
            inFlight.decrementAndGet()
        }
    }

    override suspend fun item(id: Long): ItemDetail = counted("item") { delegate.item(id) }

    override suspend fun next(id: Long): MaybeItem = counted("next") { delegate.next(id) }

    override suspend fun playlist(id: Long) = counted("playlist") { delegate.playlist(id) }

    override suspend fun collection(id: Long) = counted("collection") { delegate.collection(id) }

    override suspend fun startPlayback(request: PlaybackRequest): PlaybackInfo = counted("startPlayback") { delegate.startPlayback(request) }

    override suspend fun playbackStatus(sessionId: String): SessionStatus = counted("playbackStatus") { delegate.playbackStatus(sessionId) }

    override suspend fun stopPlayback(sessionId: String) = counted("stopPlayback") { delegate.stopPlayback(sessionId) }

    override suspend fun subtitles(fileId: Long, index: Int): SubtitleText = counted("subtitles") { delegate.subtitles(fileId, index) }

    override suspend fun trickplay(fileId: Long): TrickplayInfo? = counted("trickplay") { delegate.trickplay(fileId) }

    override suspend fun reportProgress(itemId: Long, report: ProgressReport) = counted("reportProgress") { delegate.reportProgress(itemId, report) }

    override suspend fun setStreams(itemId: Long, choice: StreamChoice) = counted("setStreams") { delegate.setStreams(itemId, choice) }
}

/**
 * A playback against a FixtureServer, with a fake engine and virtual time:
 * the server's clock is the test scheduler's, so the journal says when (in
 * virtual ms) each write came. Robolectric tests only (the texts are the
 * app's resources).
 */
class SessionHarness(
    val caps: CapsSnapshot = capsOf(TestDevices.sdrPhone),
    val autoplay: Boolean = true,
    val quality: String? = null,
    /** Picture subtitles beside a stream; `null` burns them in. */
    val pictures: PictureSource? = null,
    /** The server's events, as the app's stream would bring them. */
    val events: Flow<ServerEvent> = emptyFlow(),
) : AutoCloseable {
    val scheduler = TestCoroutineScheduler()
    private val dispatcher = StandardTestDispatcher(scheduler)
    val server = FixtureServer(FixtureServer.Options(scanEveryMs = 0, clock = { scheduler.currentTime }))
    val engine = FakeEngineLink { scheduler.currentTime }
    private val reportScope = CoroutineScope(dispatcher + SupervisorJob())
    val api: CountingApi
    val remembered = mutableListOf<String>()
    private var controllers = mutableListOf<SessionPlaybackController>()

    /** Downloads, for playbacks started from now on. */
    var local: LocalMedia = LocalMedia.None

    init {
        server.start()
        val token = server.accessToken()
        val client = OkHttpClient.Builder()
            .readTimeout(10, TimeUnit.SECONDS)
            .addInterceptor { chain -> chain.proceed(chain.request().newBuilder().header("Authorization", "Bearer $token").build()) }
            .build()
        api = CountingApi(HttpPleyApi(server.url, { client }))
    }

    val reporter = SessionReporter(reportScope)

    fun environment(): PlaybackEnvironment = PlaybackEnvironment(
        api = api,
        capabilities = FakeDeviceCapabilities(probe = caps),
        policy = DefaultPlaybackCapsPolicy,
        details = ItemDetails.of(api),
        engines = { engine },
        texts = ResourcePlayerTexts(RuntimeEnvironment.getApplication()),
        clientId = { CLIENT_ID },
        reporter = reporter,
        subtitles = ApiSubtitleFetcher(api),
        quality = object : QualityMemory {
            override suspend fun remembered(): String? = remembered.lastOrNull() ?: quality

            override suspend fun remember(id: String) {
                remembered += id
            }
        },
        autoplayNext = { autoplay },
        now = { scheduler.currentTime },
        work = dispatcher,
        pictures = pictures,
        events = events,
        local = local,
        tracksChosen = { itemId, choice -> synchronized(tracksChosen) { tracksChosen += itemId to choice } },
    )

    /** What the app was told of tracks the player saved (PlaybackEnvironment.tracksChosen). */
    val tracksChosen = mutableListOf<Pair<Long, StreamChoice>>()

    /** Starts playing [request] and lets the server answer. */
    fun play(request: PlayRequest): SessionPlaybackController {
        val controller = SessionPlaybackController(request, environment(), CoroutineScope(dispatcher + SupervisorJob()))
        controllers += controller
        controller.start()
        settle()
        return controller
    }

    /** Runs what is due now, waiting for the server's answers, until nothing is left to run. */
    fun settle() {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (true) {
            scheduler.runCurrent()
            if (api.inFlight.get() == 0) {
                scheduler.runCurrent()
                if (api.inFlight.get() == 0) return
            }
            if (System.nanoTime() > deadline) fail("the FixtureServer didn't answer in 10 s")
            Thread.sleep(2)
        }
    }

    /** Lets [ms] of virtual time pass, the server answering along the way. */
    fun advance(ms: Long) {
        var left = ms
        while (left > 0) {
            val step = minOf(STEP_MS, left)
            scheduler.advanceTimeBy(step)
            scheduler.runCurrent()
            settle()
            left -= step
        }
    }

    // ---------------------------------------------------------------- what the server got

    val journal: Journal get() = server.journal

    /** The playback requests, in order. */
    fun playbackRequests(): List<PlaybackRequest> =
        journal.entries().filter { it.method == "POST" && it.path == "/api/playback" }.map { PleyJson.decodeFromString(PlaybackRequest.serializer(), it.body) }

    /** The progress reports of [itemId], in order. */
    fun progress(itemId: Long): List<Pair<Long, ProgressReport>> =
        journal.writes("POST", "/api/items/$itemId/progress").map { it.atMs to PleyJson.decodeFromString(ProgressReport.serializer(), it.body) }

    /** The DELETEs of sessions, in order: their ids. */
    fun deletes(): List<String> = journal.entries().filter { it.method == "DELETE" && it.path.startsWith("/api/playback/") }.map { it.path.removePrefix("/api/playback/") }

    /** What happened, as "POST /api/playback", "POST /api/items/1/progress stopped", "DELETE"…, for order checks. */
    fun timeline(): List<String> = journal.entries().map { e ->
        when {
            e.path.endsWith("/progress") -> "progress ${PleyJson.decodeFromString(ProgressReport.serializer(), e.body).state.wire}"
            e.method == "DELETE" && e.path.startsWith("/api/playback/") -> "delete"
            e.path == "/api/playback" -> "start"
            e.path.endsWith("/streams") -> "streams"
            else -> "${e.method} ${e.path}"
        }
    }

    fun state(controller: SessionPlaybackController): PlayerUiState = controller.state.value

    override fun close() {
        controllers.forEach { it.close() }
        settle()
        server.close()
    }

    companion object {
        const val CLIENT_ID = "android-test"
        private const val STEP_MS = 250L

        val S1E1 = FixtureIds.SHOW_S1E1
    }
}
