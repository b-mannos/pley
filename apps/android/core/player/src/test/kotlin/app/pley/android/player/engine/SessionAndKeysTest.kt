package app.pley.android.player.engine

import android.content.Context
import android.os.Looper
import android.view.KeyEvent
import androidx.media3.common.Player
import androidx.test.core.app.ApplicationProvider
import app.pley.android.player.EngineSource
import app.pley.android.testing.player.engine.FakePlayerEngine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

@RunWith(RobolectricTestRunner::class)
class SessionAndKeysTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    private fun preparedEngine() = FakePlayerEngine().apply {
        prepare(EngineSource("http://nas:7539/api/files/1/stream", hls = false), 0, playWhenReady = false)
        player.ready()
        shadowOf(Looper.getMainLooper()).idle()
    }

    private fun key(code: Int, action: Int = KeyEvent.ACTION_DOWN, repeat: Int = 0) = KeyEvent(0, 0, action, code, repeat)

    @Test
    fun playPauseTogglesAndIsAlwaysHandled() {
        val engine = preparedEngine()
        assertTrue(MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE)))
        assertTrue(engine.player.playWhenReady)
        // The key's release and its repeats are handled too, and change nothing.
        assertTrue(MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.ACTION_UP)))
        assertTrue(MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, repeat = 1)))
        assertTrue(engine.player.playWhenReady)
        assertTrue(MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE)))
        assertFalse(engine.player.playWhenReady)
    }

    @Test
    fun playAndPauseKeys() {
        val engine = preparedEngine()
        assertTrue(MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_MEDIA_PLAY)))
        assertTrue(engine.player.playWhenReady)
        assertTrue(MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_MEDIA_PAUSE)))
        assertFalse(engine.player.playWhenReady)
        assertFalse("other keys are the screen's", MediaKeys.handle(engine.player, key(KeyEvent.KEYCODE_DPAD_CENTER)))
    }

    @Test
    fun theSessionLivesWhileTheScreenIsStarted() {
        val engine = preparedEngine()
        val session = VideoMediaSession(context, engine.player)
        session.start()
        session.start()
        assertTrue(session.isActive)
        session.stop()
        assertFalse(session.isActive)
        // A second screen later gets a session of its own.
        val again = VideoMediaSession(context, engine.player)
        again.start()
        assertTrue(again.isActive)
        again.stop()
    }

    @Test
    fun theFakeEngineRecordsAndReports() {
        val engine = preparedEngine()
        assertEquals(1, engine.prepared.size)
        assertEquals(Player.STATE_READY, engine.player.playbackState)
        engine.update { it.copy(switchingDisplayMode = true) }
        assertTrue(engine.state.value.switchingDisplayMode)
        engine.player.advanceTo(42_000)
        assertEquals(42_000L, engine.player.currentPosition)
        engine.release()
        assertTrue(engine.released)
    }
}
