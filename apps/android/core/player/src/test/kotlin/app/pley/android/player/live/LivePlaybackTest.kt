package app.pley.android.player.live

import android.os.Looper
import app.pley.android.network.ApiException
import app.pley.android.network.OfflineException
import app.pley.android.testing.data.FakeLiveTv
import app.pley.android.testing.player.engine.FakePlayerEngine
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

/** Watching a channel: tuning with this device's id, zapping, the server's refusals, no pausing, the background. */
@RunWith(RobolectricTestRunner::class)
class LivePlaybackTest {
    private val scheduler = TestCoroutineScheduler()
    private val scope = CoroutineScope(StandardTestDispatcher(scheduler) + SupervisorJob())
    private val live = FakeLiveTv()
    private val engines = mutableListOf<FakePlayerEngine>()
    private val playback = LivePlayback(live, { FakePlayerEngine().also { engines += it } }, { "android-1" }, scope)

    @After
    fun close() {
        playback.close()
        scope.cancel()
    }

    /** Runs what is due, the player's callbacks included. */
    private fun settle() {
        scheduler.runCurrent()
        shadowOf(Looper.getMainLooper()).idle()
        scheduler.runCurrent()
    }

    private fun engine() = engines.single()

    @Test
    fun `a channel is watched with this device's id and plays its HLS from the live edge`() {
        playback.tune(1)
        assertEquals(LivePhase.Tuning, playback.state.value.phase)
        settle()
        assertEquals(listOf("watch 1 android-1"), live.sent)
        val prepared = engine().prepared.single()
        assertEquals("http://127.0.0.1:7539/api/live/sessions/live-1/index.m3u8", prepared.source.url)
        assertTrue(prepared.source.hls && prepared.source.live && prepared.playWhenReady)

        engine().player.ready()
        settle()
        assertEquals(LivePhase.Playing, playback.state.value.phase)
        assertEquals(1L, playback.state.value.channelId)
    }

    @Test
    fun `zapping lets the last channel go and keeps the one engine`() {
        playback.tune(1)
        settle()
        engine().player.ready()
        settle()
        playback.tune(2)
        settle()
        assertEquals(listOf("watch 1 android-1", "leave live-1", "watch 2 android-1"), live.sent)
        assertEquals(1, engines.size)
        assertEquals(2, engine().prepared.size)
        assertEquals(LivePhase.Tuning, playback.state.value.phase)
    }

    @Test
    fun `every connection in use says which channels hold them, and trying again tunes again`() {
        val message = "The one connection of Test Provider is in use, watching Film Four."
        live.watchFails = ApiException(409, "conflict", message)
        playback.tune(1)
        settle()
        val failed = playback.state.value.phase as LivePhase.Failed
        assertEquals(LiveTrouble.Kind.Busy, failed.trouble.kind)
        assertEquals(message, failed.trouble.message)
        assertTrue(failed.trouble.canRetry)
        assertTrue("nothing was prepared", engines.isEmpty())

        live.watchFails = null
        playback.retry()
        settle()
        assertEquals(listOf("watch 1 android-1", "watch 1 android-1"), live.sent)
        assertEquals(1, engine().prepared.size)
    }

    @Test
    fun `refusals and failures are told apart`() {
        assertEquals(LiveTrouble.Kind.Missing, LiveTrouble.of(ApiException(404, "not_found", "gone")).kind)
        assertFalse(LiveTrouble.of(ApiException(404, "not_found", "gone")).canRetry)
        assertEquals(LiveTrouble.Kind.Stopped, LiveTrouble.of(ApiException(410, "gone", "stopped")).kind)
        assertEquals(LiveTrouble.Kind.NotAllowed, LiveTrouble.of(ApiException(403, "forbidden", "no")).kind)
        assertEquals(LiveTrouble.Kind.Unavailable, LiveTrouble.of(ApiException(503, "unavailable", "provider")).kind)
        assertEquals(LiveTrouble.Kind.Offline, LiveTrouble.of(OfflineException(IOException("down"))).kind)
    }

    @Test
    fun `live has no pause, so a channel paused by something else plays on`() {
        playback.tune(1)
        settle()
        engine().player.ready()
        settle()
        // Headphones pulled out, a stray media key.
        engine().player.pause()
        settle()
        assertTrue(engine().player.playWhenReady)
    }

    @Test
    fun `the background lets the connection and the decoders go, and coming back tunes again`() {
        playback.tune(3)
        settle()
        engine().player.ready()
        settle()
        playback.stop()
        settle()
        assertTrue(engine().released)
        assertNull(playback.engine.value)
        assertEquals(LivePhase.Idle, playback.state.value.phase)
        assertEquals(3L, playback.state.value.channelId)
        assertEquals("leave live-3", live.sent.last())

        playback.resume()
        settle()
        assertEquals("watch 3 android-1", live.sent.last())
        assertEquals(2, engines.size)
    }

    @Test
    fun `a closed playback tunes nothing more`() {
        playback.tune(1)
        settle()
        playback.close()
        playback.tune(2)
        settle()
        assertEquals(listOf("watch 1 android-1", "leave live-1"), live.sent)
    }
}
