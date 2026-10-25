package app.pley.android.player.music

import android.app.PendingIntent
import android.content.Intent
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Music in the background (Android 17 plays background audio only from a
 * foreground service): a Media3 media session service, foreground with
 * type mediaPlayback while music plays, which shows the notification and
 * the lock screen's controls. It plays the app's one [MusicPlayback]
 * ([MusicHub]); with nothing to play, it takes no connections and stops.
 *
 * Only the app itself, the system (its media controls, the notification,
 * Bluetooth and the lock screen) and Media3's notification controller may
 * connect: no other app drives pley's music.
 */
@OptIn(markerClass = [UnstableApi::class])
class MusicService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        sessionOrMake()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = sessionOrMake()

    /**
     * The session over the process's music, made again when music plays
     * after a stop (the service may still be bound then); `null` with
     * nothing to play.
     */
    private fun sessionOrMake(): MediaSession? {
        session?.let { return it }
        val playback = MusicHub.playback ?: return null
        if (!playback.active) return null
        val output = playback.output() as? EngineMusicOutput ?: return null
        val made = MediaSession.Builder(this, QueuePlayer(output.player, playback))
            .setId(SESSION_ID)
            .setCallback(OnlyOurs(packageName))
            .apply { openApp()?.let(::setSessionActivity) }
            .build()
        session = made
        playback.onStopped = {
            playback.onStopped = null
            if (session === made) session = null
            made.release()
            stopSelf()
        }
        return made
    }

    override fun onDestroy() {
        MusicHub.playback?.onStopped = null
        session?.release()
        session = null
        super.onDestroy()
    }

    /** Tapping the notification opens the app (MainActivity, which hands over to the TV's on a TV). */
    private fun openApp(): PendingIntent? {
        val intent = Intent(Intent.ACTION_MAIN)
            .setClassName(this, MAIN_ACTIVITY)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        return PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }

    /** Accepts the app, the system's controllers and Media3's notification; refuses everyone else. */
    private class OnlyOurs(private val own: String) : MediaSession.Callback {
        override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult =
            if (allowed(session, controller)) {
                super.onConnect(session, controller)
            } else {
                MediaSession.ConnectionResult.reject()
            }

        private fun allowed(session: MediaSession, controller: MediaSession.ControllerInfo): Boolean =
            controller.packageName == own ||
                session.isMediaNotificationController(controller) ||
                controller.isTrusted
    }

    private companion object {
        const val SESSION_ID = "pley.music"
        const val MAIN_ACTIVITY = "app.pley.android.MainActivity"
    }
}

/**
 * The process's music, for the service: the player graph puts its
 * [MusicPlayback] here when it makes it. A service the system starts in a
 * process without one (after process death) has nothing to play.
 */
object MusicHub {
    @Volatile
    var playback: MusicPlayback? = null
}
