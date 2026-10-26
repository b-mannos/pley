package app.pley.android.player.engine

import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.compose.ContentFrame
import androidx.media3.ui.compose.SURFACE_TYPE_SURFACE_VIEW

/**
 * The video, on a SurfaceView: frames go straight to the display's
 * compositor (timed better, less power, HDR passed through, full resolution
 * on TVs whose interface is 1080p). Fitted to the screen, black until the
 * first frame. The player may be prepared before this exists. Never put it
 * in a lazy list or under a full-screen alpha animation. Subtitles go over
 * it with [ScreenCaptions][app.pley.android.player.subtitles.ScreenCaptions].
 */
@OptIn(markerClass = [UnstableApi::class])
@Composable
fun VideoSurface(player: Player?, modifier: Modifier = Modifier) {
    ContentFrame(
        player = player,
        modifier = modifier.background(androidx.compose.ui.graphics.Color.Black),
        surfaceType = SURFACE_TYPE_SURFACE_VIEW,
        contentScale = ContentScale.Fit,
    )
}
