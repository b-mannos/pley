package app.pley.android.player.engine

import android.app.Activity
import android.content.Context
import android.graphics.SurfaceTexture
import android.os.Looper
import android.view.Surface
import androidx.media3.common.Player
import androidx.media3.test.utils.FakeClock
import androidx.media3.test.utils.robolectric.ShadowMediaCodecConfig
import androidx.media3.test.utils.robolectric.TestPlayerRunHelper
import androidx.test.core.app.ApplicationProvider
import app.pley.android.caps.CapsDetails
import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.DecoderPolicy
import app.pley.android.caps.DisplayState
import app.pley.android.caps.MatchContent
import app.pley.android.common.DeviceProfile
import app.pley.android.common.FormFactor
import app.pley.android.common.Tier
import app.pley.android.model.DynamicRange
import app.pley.android.model.MediaStream
import app.pley.android.model.StreamKind
import app.pley.android.model.caps.ClientCaps
import app.pley.android.player.EngineSource
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import okhttp3.Headers
import okhttp3.OkHttpClient
import okio.Buffer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import java.io.File
import java.time.Duration

@RunWith(RobolectricTestRunner::class)
class Media3EngineTest {
    @get:Rule
    val codecs: ShadowMediaCodecConfig = ShadowMediaCodecConfig.withNoDefaultSupportedCodecs()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val engines = mutableListOf<Media3Engine>()

    private val phone = DeviceProfile(FormFactor.Phone, fireTv = false, tier = Tier.T2, model = "Pixel 7a", totalRamBytes = 8L shl 30, lowRam = false)
    private val tv = DeviceProfile(FormFactor.Tv, fireTv = false, tier = Tier.T2, model = "Google TV Streamer", totalRamBytes = 4L shl 30, lowRam = false)

    /** The fixture server's 20 s clip: H.264 High 1080p24 8-bit SDR, AAC stereo. */
    private val clip: ByteArray by lazy {
        javaClass.classLoader!!.getResourceAsStream("app/pley/android/fixtures/media/sample-1080p.mp4")!!.use { it.readBytes() }
    }

    private class MemoryFrameRate : FrameRateMemory {
        override suspend fun timeouts(displayKey: String) = 0

        override suspend fun setTimeouts(displayKey: String, count: Int) = Unit

        override suspend fun takeHint() = false
    }

    private class FakeHost(var display: DisplayState) : DisplayModeHost {
        val requested = mutableListOf<Int>()

        override val displayKey = "0:HDMI"

        override fun read() = display

        override fun request(modeId: Int) {
            requested += modeId
        }

        override suspend fun awaitMode(modeId: Int, timeoutMs: Long): Boolean {
            display = display.copy(activeModeId = modeId)
            return true
        }
    }

    private fun snapshot(policy: DecoderPolicy) = CapsSnapshot(ClientCaps(), emptyList(), CapsDetails(policy = policy))

    private fun engine(
        profile: DeviceProfile = phone,
        caps: CapsSnapshot? = null,
        client: OkHttpClient = OkHttpClient(),
        host: DisplayModeHost? = null,
        settings: EngineSettings = EngineSettings(),
        music: Boolean = false,
    ): Media3Engine = Media3Engine(
        context = context,
        profile = profile,
        music = music,
        caps = { caps },
        mediaClient = { client },
        settings = { settings },
        frameRateMemory = MemoryFrameRate(),
        focusPort = if (music) null else AndroidFocusPort(context),
        sdk = 34,
        hostFor = { host ?: WindowModeHost(it) },
        playerClock = FakeClock(true),
    ).also { engines += it }

    @After
    fun releaseAll() = engines.forEach { it.release() }

    private fun file(): String {
        val f = File.createTempFile("clip", ".mp4").apply { deleteOnExit() }
        f.writeBytes(clip)
        return f.toURI().toString()
    }

    @Test
    fun aBundledMp4ReachesReadyAndPlaysOnAPhone() {
        codecs.addDecoders(ShadowMediaCodecConfig.CODEC_INFO_AVC, ShadowMediaCodecConfig.CODEC_INFO_AAC)
        val engine = engine()
        engine.player.setVideoSurface(Surface(SurfaceTexture(1)))
        engine.prepare(EngineSource(file(), hls = false, mimeType = "video/mp4"), startMs = 0, playWhenReady = true)

        TestPlayerRunHelper.run(engine.player).untilState(Player.STATE_READY)
        assertTrue("audio focus granted", engine.player.playWhenReady)
        TestPlayerRunHelper.run(engine.player).untilPositionAtLeast(2_000)

        // The first frame itself is checked on a device (EngineOnDeviceTest).
        val state = engine.state.value
        assertNull(state.needsServerHelp)
        assertEquals(1920, state.stats.width)
        assertEquals("SDR", state.stats.dynamicRange)
        assertNotNull(state.stats.videoDecoder)
    }

    @Test
    fun musicPlaysTheAudioAloneAndMissesNothing() {
        codecs.addDecoders(ShadowMediaCodecConfig.CODEC_INFO_AVC, ShadowMediaCodecConfig.CODEC_INFO_AAC)
        val engine = engine(music = true)
        engine.prepare(EngineSource(file(), hls = false), startMs = 0, playWhenReady = true)
        TestPlayerRunHelper.run(engine.player).untilState(Player.STATE_READY)
        TestPlayerRunHelper.run(engine.player).untilPositionAtLeast(1_000)
        assertNull(engine.state.value.needsServerHelp)
        assertNull("no video decoder for music", engine.state.value.stats.videoDecoder)
        assertNotNull(engine.state.value.stats.audioDecoder)
        assertTrue(engine.player.trackSelectionParameters.audioOffloadPreferences.audioOffloadMode != 0)
    }

    @Test
    fun videoWithoutAnAllowedDecoderNeedsTheServer() {
        codecs.addDecoders(ShadowMediaCodecConfig.CODEC_INFO_AAC)
        val engine = engine()
        engine.prepare(EngineSource(file(), hls = false), startMs = 0, playWhenReady = false)
        TestPlayerRunHelper.run(engine.player).untilState(Player.STATE_READY)
        assertEquals(NeedsServerHelp(video = true, audio = false), engine.state.value.needsServerHelp)
    }

    @Test
    fun theMediaClientCarriesTheTokenAndTheFileArrivesInRanges() {
        codecs.addDecoders(ShadowMediaCodecConfig.CODEC_INFO_AVC, ShadowMediaCodecConfig.CODEC_INFO_AAC)
        val server = MockWebServer()
        server.dispatcher = RangeDispatcher(clip, token = "Bearer secret")
        server.start()
        try {
            val client = OkHttpClient.Builder()
                .addInterceptor { chain -> chain.proceed(chain.request().newBuilder().header("Authorization", "Bearer secret").build()) }
                .build()
            val engine = engine(client = client)
            engine.prepare(EngineSource(server.url("/api/files/1/stream").toString(), hls = false), startMs = 5_000, playWhenReady = false)
            TestPlayerRunHelper.run(engine.player).untilState(Player.STATE_READY)
            assertEquals(5_000L, engine.player.currentPosition)
            // The header, then from the 5 s point on.
            assertEquals(null, server.takeRequest().headers["Range"])
            assertTrue(server.takeRequest().headers["Range"]!!.startsWith("bytes="))
        } finally {
            server.close()
        }
    }

    @Test
    fun aMissingSessionFailsAtOnceRatherThanRetrying() {
        val server = MockWebServer()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest) = MockResponse.Builder().code(404).build()
        }
        server.start()
        try {
            val engine = engine()
            // Not playing: the player's clock moves by itself while the request is really out, and
            // playing, Media3's stuck-buffering check would reach its 10 minutes before a slow 404 came.
            engine.prepare(EngineSource(server.url("/api/playback/s1/index.m3u8").toString(), hls = true), startMs = 0, playWhenReady = false)
            val error = TestPlayerRunHelper.run(engine.player).untilPlayerError()
            assertTrue(MediaRetries.isFinal(error))
            assertEquals(1, server.requestCount)
        } finally {
            server.close()
        }
    }

    @Test
    fun aTvWaitsForItsDisplayModeBeforePlaying() {
        codecs.addDecoders(ShadowMediaCodecConfig.CODEC_INFO_AVC, ShadowMediaCodecConfig.CODEC_INFO_AAC)
        val host = FakeHost(ModeChoiceTest.STREAMER.copy(matchContent = MatchContent.Always))
        val engine = engine(profile = tv, caps = snapshot(DecoderPolicy(tv = true, allowSoftware = true)), host = host)
        val film = MediaStream(index = 0, kind = StreamKind.Video, codec = "h264", frameRate = 23.976, dynamicRange = DynamicRange.Sdr)
        engine.prepare(EngineSource(file(), hls = false, video = film, durationMs = 2 * 60 * 60 * 1000L), startMs = 0, playWhenReady = true)
        // Pressed play at tap time: it buffers, but waits for the window to choose a mode.
        assertFalse(engine.player.playWhenReady)

        val window = Robolectric.buildActivity(Activity::class.java).setup().get().window
        engine.attachWindow(window)
        assertEquals(listOf(46), host.requested)
        assertTrue(engine.state.value.switchingDisplayMode)
        assertFalse(engine.player.playWhenReady)

        // Play pressed again while switching: still waits.
        engine.player.play()
        assertFalse(engine.player.playWhenReady)

        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(FrameRateController.SETTLE_MS))
        assertFalse(engine.state.value.switchingDisplayMode)
        assertTrue(engine.player.playWhenReady)
        assertEquals(46, engine.state.value.stats.displayModeId)

        engine.detachWindow()
        assertEquals("the mode goes back to the system's choice", listOf(46, 0), host.requested)
    }

    @Test
    fun aTvLeavesTheModeForShortItemsAndPhonesNeverWait() {
        codecs.addDecoders(ShadowMediaCodecConfig.CODEC_INFO_AVC, ShadowMediaCodecConfig.CODEC_INFO_AAC)
        val host = FakeHost(ModeChoiceTest.STREAMER.copy(matchContent = MatchContent.Always))
        val film = MediaStream(index = 0, kind = StreamKind.Video, codec = "h264", frameRate = 23.976)
        val trailer = engine(profile = tv, caps = snapshot(DecoderPolicy(tv = true, allowSoftware = true)), host = host)
        trailer.prepare(EngineSource(file(), hls = false, video = film, durationMs = 150_000), startMs = 0, playWhenReady = true)
        assertTrue(trailer.player.playWhenReady)

        val off = engine(profile = tv, caps = snapshot(DecoderPolicy(tv = true, allowSoftware = true)), host = host, settings = EngineSettings(refreshRate = RefreshRateMatching.Off))
        off.prepare(EngineSource(file(), hls = false, video = film, durationMs = 7_200_000), startMs = 0, playWhenReady = true)
        assertTrue(off.player.playWhenReady)

        val onPhone = engine()
        onPhone.prepare(EngineSource(file(), hls = false, video = film, durationMs = 7_200_000), startMs = 0, playWhenReady = true)
        assertTrue(onPhone.player.playWhenReady)
        assertEquals(emptyList<Int>(), host.requested)
    }

    /** Serves [body] with ranges, as the server's /stream does, to requests carrying [token] only. */
    private class RangeDispatcher(private val body: ByteArray, private val token: String) : Dispatcher() {
        override fun dispatch(request: RecordedRequest): MockResponse {
            System.err.println("DEBUG request ${request.target} range=${request.headers["Range"]} auth=${request.headers["Authorization"]}")
            if (request.headers["Authorization"] != token) return MockResponse.Builder().code(401).build()
            val range = request.headers["Range"]?.removePrefix("bytes=")?.split('-')
            val start = range?.getOrNull(0)?.toLongOrNull() ?: 0L
            val end = range?.getOrNull(1)?.toLongOrNull() ?: (body.size - 1L)
            val slice = body.copyOfRange(start.toInt(), end.toInt() + 1)
            val headers = Headers.Builder()
                .add("Content-Type", "video/mp4")
                .add("Accept-Ranges", "bytes")
                .apply { if (range != null) add("Content-Range", "bytes $start-$end/${body.size}") }
                .build()
            return MockResponse.Builder()
                .code(if (range != null) 206 else 200)
                .headers(headers)
                .body(Buffer().write(slice))
                .build()
        }
    }
}
