package app.pley.android.player.engine

import android.content.Context
import android.view.KeyEvent
import androidx.annotation.OptIn
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import java.util.concurrent.atomic.AtomicInteger

/**
 * The media session of a video player screen: remotes' media keys, Alexa
 * and Google Assistant reach the player through it. Active only while the
 * screen is started ([start] in onStart, [stop] in onStop), so a video
 * never plays on in the background.
 */
@OptIn(markerClass = [UnstableApi::class])
class VideoMediaSession(context: Context, private val player: Player) {
    private val context = context.applicationContext
    private var session: MediaSession? = null

    val isActive: Boolean get() = session != null

    fun start() {
        if (session != null) return
        session = MediaSession.Builder(context, player)
            .setId("pley.video.${ids.incrementAndGet()}")
            .setCallback(Callback)
            .build()
    }

    fun stop() {
        session?.release()
        session = null
    }

    private object Callback : MediaSession.Callback

    private companion object {
        val ids = AtomicInteger()
    }
}

/**
 * Media keys the player screens handle themselves: play/pause, play and
 * pause. They return `true`, as TV remotes expect of a player (Amazon and
 * Google's TV checks): an unhandled play/pause would start another app's
 * media.
 */
object MediaKeys {
    fun handle(player: Player, event: KeyEvent): Boolean {
        val code = event.keyCode
        if (code !in KEYS) return false
        if (event.action != KeyEvent.ACTION_DOWN || event.repeatCount > 0) return true
        when (code) {
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_HEADSETHOOK -> if (player.isPlaying || player.playWhenReady) player.pause() else player.play()
            KeyEvent.KEYCODE_MEDIA_PLAY -> player.play()
            KeyEvent.KEYCODE_MEDIA_PAUSE -> player.pause()
        }
        return true
    }

    private val KEYS = setOf(
        KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
        KeyEvent.KEYCODE_HEADSETHOOK,
        KeyEvent.KEYCODE_MEDIA_PLAY,
        KeyEvent.KEYCODE_MEDIA_PAUSE,
    )
}
