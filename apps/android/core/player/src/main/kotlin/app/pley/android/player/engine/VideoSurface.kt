package app.pley.android.player.engine

import android.graphics.Color
import android.graphics.Typeface
import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.Player
import androidx.media3.common.text.Cue
import androidx.media3.common.text.CueGroup
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.SubtitleView
import androidx.media3.ui.compose.ContentFrame
import androidx.media3.ui.compose.SURFACE_TYPE_SURFACE_VIEW

/**
 * The video, on a SurfaceView: frames go straight to the display's
 * compositor (timed better, less power, HDR passed through, full resolution
 * on TVs whose interface is 1080p). Fitted to the screen, black until the
 * first frame. The player may be prepared before this exists. Never put it
 * in a lazy list or under a full-screen alpha animation.
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

/** How subtitles look (Settings → Subtitles). */
@OptIn(markerClass = [UnstableApi::class])
data class SubtitleStyle(
    val size: Size = Size.Normal,
    /** A dark outline around the letters; a soft shadow otherwise. */
    val outline: Boolean = true,
    /** A box of 66 % black behind the text. */
    val box: Boolean = false,
) {
    enum class Size(val scale: Float) {
        Small(0.8f),
        Normal(1f),
        Large(1.25f),
        Huge(1.5f),
    }

    /** Media3's caption style for this. */
    fun captionStyle(): CaptionStyleCompat = CaptionStyleCompat(
        Color.WHITE,
        if (box) BOX_COLOR else Color.TRANSPARENT,
        Color.TRANSPARENT,
        if (outline) CaptionStyleCompat.EDGE_TYPE_OUTLINE else CaptionStyleCompat.EDGE_TYPE_DROP_SHADOW,
        Color.BLACK,
        Typeface.DEFAULT_BOLD,
    )

    /** The text's height as a fraction of the video's. */
    val textFraction: Float get() = SubtitleView.DEFAULT_TEXT_SIZE_FRACTION * size.scale

    companion object {
        /** 66 % black. */
        const val BOX_COLOR = 0xA8000000.toInt()
    }
}

/**
 * Subtitles over the video, text and pictures alike (PGS, VobSub), drawn by
 * Media3's SubtitleView in [style]. They come from [player]'s own tracks,
 * or from [cues] when given (the session's subtitles from the server).
 */
@OptIn(markerClass = [UnstableApi::class])
@Composable
fun Subtitles(player: Player?, style: SubtitleStyle, modifier: Modifier = Modifier, cues: List<Cue>? = null) {
    val listener = remember { SubtitleListener() }
    Box(modifier.fillMaxSize()) {
        AndroidView(
            factory = { context -> SubtitleView(context).also { listener.view = it } },
            modifier = Modifier.fillMaxSize(),
            update = { view ->
                view.setStyle(style.captionStyle())
                view.setFractionalTextSize(style.textFraction)
                view.setApplyEmbeddedFontSizes(false)
                listener.override = cues
                view.setCues(cues ?: player?.currentCues?.cues.orEmpty())
            },
            onRelease = { listener.view = null },
        )
    }
    DisposableEffect(player) {
        player?.addListener(listener)
        onDispose { player?.removeListener(listener) }
    }
}

@OptIn(markerClass = [UnstableApi::class])
private class SubtitleListener : Player.Listener {
    var view: SubtitleView? = null
    var override: List<Cue>? = null

    override fun onCues(cueGroup: CueGroup) {
        if (override == null) view?.setCues(cueGroup.cues)
    }
}
