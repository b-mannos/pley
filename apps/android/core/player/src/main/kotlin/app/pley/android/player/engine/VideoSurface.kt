package app.pley.android.player.engine

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.compose.ContentFrame
import androidx.media3.ui.compose.SURFACE_TYPE_SURFACE_VIEW

/**
 * The video, on a SurfaceView: frames go straight to the display's
 * compositor (timed better, less power, HDR passed through, full resolution
 * on TVs whose interface is 1080p). Fitted to the screen, black until the
 * first frame, except while [holding]: a stream made ready beside the one
 * that played takes over (PlayerUiState.handoff), and the last picture
 * stays until the new one's first. The player may be prepared before this
 * exists. Never put it in a lazy list or under a full-screen alpha
 * animation. Subtitles go over it with
 * [ScreenCaptions][app.pley.android.player.subtitles.ScreenCaptions].
 */
@OptIn(markerClass = [UnstableApi::class])
@Composable
fun VideoSurface(player: Player?, modifier: Modifier = Modifier, holding: () -> Boolean = { false }) {
    ContentFrame(
        player = player,
        modifier = modifier.background(Color.Black),
        surfaceType = SURFACE_TYPE_SURFACE_VIEW,
        contentScale = ContentScale.Fit,
        shutter = { if (!holding()) Box(Modifier.fillMaxSize().background(Color.Black)) },
    )
}
