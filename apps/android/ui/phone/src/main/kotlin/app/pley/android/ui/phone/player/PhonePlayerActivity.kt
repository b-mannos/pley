package app.pley.android.ui.phone.player

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.pley.android.ui.phone.R

/**
 * The phone and tablet player, in a window of its own (orientation, picture
 * in picture, the screen kept on while playing). A placeholder for now.
 */
class PhonePlayerActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            Box(Modifier.fillMaxSize().background(Color.Black).padding(24.dp), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.player_placeholder), color = Color.White)
            }
        }
    }

    companion object {
        /** The key PlaybackServices.prepare gave, to find the playback by. */
        const val EXTRA_PLAYBACK = "playback"

        fun intent(context: Context, playback: String): Intent =
            Intent(context, PhonePlayerActivity::class.java).putExtra(EXTRA_PLAYBACK, playback)
    }
}
