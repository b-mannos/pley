package app.pley.android.ui.phone.player

import android.app.PendingIntent
import android.app.RemoteAction
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import app.pley.android.ui.phone.R

/** The buttons picture in picture shows over the video. */
internal object PipActions {
    /** Pause while [playing], Play otherwise; the player screen's receiver does it. */
    fun playPause(context: Context, playing: Boolean): RemoteAction {
        val title = context.getString(if (playing) R.string.player_pause else R.string.player_play)
        val icon = Icon.createWithResource(context, if (playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play)
        val intent = Intent(PhonePlayerActivity.ACTION_PLAY_PAUSE).setPackage(context.packageName)
        val pending = PendingIntent.getBroadcast(context, REQUEST_PLAY_PAUSE, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        return RemoteAction(icon, title, title, pending)
    }

    private const val REQUEST_PLAY_PAUSE = 1
}
