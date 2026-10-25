package app.pley.android.player.engine

import android.hardware.display.DisplayManager
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.view.SurfaceView
import androidx.activity.ComponentActivity
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.pley.android.caps.AndroidDisplayInfo
import app.pley.android.common.DeviceProfile
import app.pley.android.model.DynamicRange
import app.pley.android.model.MediaStream
import app.pley.android.model.StreamKind
import app.pley.android.player.EngineSource
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import okhttp3.OkHttpClient
import okio.Buffer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.math.abs

/**
 * A TV changing its display mode for a 23.976 fps film (refresh-rate
 * matching set to Always, so the device's own setting doesn't stop it),
 * playing it on the new mode, and handing the mode back on close. TVs
 * only; emulators have one mode.
 */
@RunWith(AndroidJUnit4::class)
class FrameRateOnDeviceTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val profile = DeviceProfile.of(context)
    private lateinit var scenario: ActivityScenario<ComponentActivity>
    private lateinit var surface: SurfaceView
    private var engine: Media3Engine? = null
    private val server = MockWebServer()

    @Before
    fun setUp() {
        assumeTrue("a TV", profile.isTv)
        val display = AndroidDisplayInfo(context).read()
        assumeTrue("a TV with a 23.976 Hz mode", display != null && display.modes.any { abs(it.refreshRate - 23.976f) < 0.01f })
        scenario = ActivityScenario.launch(ComponentActivity::class.java)
        scenario.onActivity { activity ->
            // Shown over a lock screen, as a player is; the device is woken for the run.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
                activity.setShowWhenLocked(true)
                activity.setTurnScreenOn(true)
            }
            surface = SurfaceView(activity)
            activity.setContentView(surface)
        }
    }

    @After
    fun tearDown() {
        if (!::scenario.isInitialized) return
        instrumentation.runOnMainSync {
            engine?.detachWindow()
            engine?.release()
        }
        scenario.close()
        server.close()
    }

    private fun <T> onMain(block: () -> T): T {
        var out: T? = null
        instrumentation.runOnMainSync { out = block() }
        @Suppress("UNCHECKED_CAST")
        return out as T
    }

    private fun waitFor(timeoutMs: Long, what: String, condition: () -> Boolean) {
        val end = SystemClock.elapsedRealtime() + timeoutMs
        while (!onMain(condition)) {
            check(SystemClock.elapsedRealtime() < end) { "Timed out waiting for $what" }
            SystemClock.sleep(50)
        }
    }

    private fun refreshRate(): Float {
        val displays = context.getSystemService(DisplayManager::class.java)
        return displays.getDisplay(android.view.Display.DEFAULT_DISPLAY).mode.refreshRate
    }

    @Test
    fun aFilmSwitchesTheTvTo23976HzPlaysAndHandsTheModeBack() {
        val bytes = javaClass.classLoader!!.getResourceAsStream("app/pley/android/fixtures/media/sample-2160p-hdr10.mkv")!!.use { it.readBytes() }
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                val range = request.headers["Range"]?.removePrefix("bytes=")?.split('-')
                val start = range?.getOrNull(0)?.toLongOrNull() ?: 0L
                val end = range?.getOrNull(1)?.toLongOrNull() ?: (bytes.size - 1L)
                return MockResponse.Builder()
                    .code(if (range != null) 206 else 200)
                    .addHeader("Accept-Ranges", "bytes")
                    .apply { if (range != null) addHeader("Content-Range", "bytes $start-$end/${bytes.size}") }
                    .body(Buffer().write(bytes, start.toInt(), (end - start + 1).toInt()))
                    .build()
            }
        }
        server.start()
        val caps = realProbe(context, profile)
        val before = refreshRate()
        Log.i(TAG, "before: $before Hz; modes ${caps.details.input?.display?.modes?.map { "${it.id}:${it.width}x${it.height}@${it.refreshRate}" }}")

        val film = MediaStream(index = 0, kind = StreamKind.Video, codec = "hevc", width = 3840, height = 2160, frameRate = 24000.0 / 1001, dynamicRange = DynamicRange.Hdr10)
        instrumentation.runOnMainSync {
            engine = Media3Engine(
                context = context,
                profile = profile,
                music = false,
                caps = { caps },
                mediaClient = { OkHttpClient() },
                settings = { EngineSettings(refreshRate = RefreshRateMatching.Always) },
                frameRateMemory = PreferencesFrameRateMemory(context),
                focusPort = AndroidFocusPort(context),
            ).also { it.player.setVideoSurfaceView(surface) }
        }
        val engine = engine!!
        val started = SystemClock.elapsedRealtime()
        instrumentation.runOnMainSync {
            // At tap time: before the window exists, as the session does.
            engine.prepare(
                EngineSource(server.url("/api/files/2/stream").toString(), hls = false, mimeType = "video/x-matroska", video = film, durationMs = 2 * 60 * 60 * 1000L),
                0,
                playWhenReady = true,
            )
        }
        assertEquals(false, onMain { engine.player.playWhenReady })
        scenario.onActivity { engine.attachWindow(it.window) }
        waitFor(FrameRateController.SWITCH_TIMEOUT_MS + FrameRateController.SETTLE_MS + 3_000, "the mode switch") {
            !engine.state.value.switchingDisplayMode && engine.player.playWhenReady
        }
        val switchedIn = SystemClock.elapsedRealtime() - started
        val during = refreshRate()
        waitFor(20_000, "6 s of playback") { engine.player.currentPosition >= 6_000 || engine.player.playerError != null }
        val stats = onMain { engine.state.value.stats }
        Log.i(TAG, "during: $during Hz after $switchedIn ms; mode ${stats.displayModeId}; $stats; error ${onMain { engine.player.playerError }}")
        assertTrue("23.976 Hz while playing, was $during", abs(during - 23.976f) < 0.01f)
        assertEquals(null, onMain { engine.player.playerError })

        instrumentation.runOnMainSync { engine.detachWindow() }
        SystemClock.sleep(6_000)
        val after = refreshRate()
        Log.i(TAG, "after close: $after Hz")
        assertEquals("the system's mode is back", before, after, 0.01f)
    }

    private companion object {
        const val TAG = "FrameRateOnDeviceTest"
    }
}
