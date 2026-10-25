package app.pley.android.ui.tv.player

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.tv.material3.Text
import app.pley.android.ui.tv.R

/**
 * The TV player, in a window of its own: the display mode matched to the
 * video, HDR, and the screen kept on only while playing; everything is let
 * go in onStop. A placeholder for now.
 */
class TvPlayerActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            Box(Modifier.fillMaxSize().background(Color.Black), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.tv_player_placeholder), color = Color.White)
            }
        }
    }

    companion object {
        /** The key PlaybackServices.prepare gave, to find the playback by. */
        const val EXTRA_PLAYBACK = "playback"

        fun intent(context: Context, playback: String): Intent =
            Intent(context, TvPlayerActivity::class.java).putExtra(EXTRA_PLAYBACK, playback)
    }
}
