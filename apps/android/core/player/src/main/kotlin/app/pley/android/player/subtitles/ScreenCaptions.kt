package app.pley.android.player.subtitles

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.view.accessibility.CaptioningManager
import androidx.annotation.OptIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.Player
import androidx.media3.common.text.Cue
import androidx.media3.common.text.CueGroup
import androidx.media3.common.util.UnstableApi
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.SubtitleView
import app.pley.android.data.Settings
import app.pley.android.data.SubtitleBackground
import app.pley.android.data.SubtitleFont
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * How pley draws text subtitles (apps/web/src/player/subtitles.ts): white,
 * bold, a dark outline, no box, [textFraction] of the height they are drawn
 * in, in [font], [raise] of the height above the bottom. When the system's
 * captions are on (Settings → Accessibility → Captions), their style and
 * size win, unless [custom]: the person chose pley's own size, background
 * or font (design spec §7). The position applies either way.
 */
@Immutable
data class CaptionLook(
    /** The text's height as a share of the area's height: 0.038, 0.048, 0.06 or 0.076. */
    val textFraction: Float = DEFAULT_FRACTION,
    /** A box of 66 % black behind the text. */
    val box: Boolean = false,
    /** pley's own size, background or font were chosen: they apply even with the system's captions on. */
    val custom: Boolean = false,
    val font: SubtitleFont = SubtitleFont.Sans,
    /** How far above the bottom, as a share of the height: one of [Settings.SUBTITLE_RAISES]. */
    val raise: Float = 0f,
) {
    @OptIn(UnstableApi::class)
    internal fun captionStyle(): CaptionStyleCompat = CaptionStyleCompat(
        Color.WHITE,
        if (box) BOX_COLOR else Color.TRANSPARENT,
        Color.TRANSPARENT,
        CaptionStyleCompat.EDGE_TYPE_OUTLINE,
        Color.BLACK,
        typeface(font),
    )

    companion object {
        /** "Normal": 4.8 % of the height, as on the web. */
        const val DEFAULT_FRACTION = 0.048f

        /** 66 % black. */
        const val BOX_COLOR = 0xA8000000.toInt()

        /** How subtitles look with [settings] (Settings → Subtitles): the size nearest theirs, and pley's own style once any was chosen. */
        fun of(settings: Settings): CaptionLook {
            val defaults = Settings()
            return CaptionLook(
                textFraction = Settings.SUBTITLE_SIZES.minBy { abs(it - settings.subtitleSize) },
                box = settings.subtitleBackground == SubtitleBackground.DarkBox,
                custom = settings.subtitleSize != defaults.subtitleSize ||
                    settings.subtitleBackground != defaults.subtitleBackground ||
                    settings.subtitleFont != defaults.subtitleFont,
                font = settings.subtitleFont,
                raise = Settings.SUBTITLE_RAISES.minBy { abs(it - settings.subtitleRaise) },
            )
        }

        /** The fonts this device has: the casual hand only where the system has one (Android's "casual"; not every Fire OS). */
        fun fonts(): List<SubtitleFont> = SubtitleFont.entries.filter { it != SubtitleFont.Casual || casualFont != null }

        /** [font] in bold, as subtitles are drawn; the default's where the device lacks it. */
        fun typeface(font: SubtitleFont): Typeface = when (font) {
            SubtitleFont.Sans -> Typeface.DEFAULT_BOLD
            SubtitleFont.Serif -> Typeface.create(Typeface.SERIF, Typeface.BOLD)
            SubtitleFont.Monospace -> Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
            SubtitleFont.Casual -> casualFont ?: Typeface.DEFAULT_BOLD
        }

        /** The system's casual family in bold; `null` when asking for it gives the default back. */
        private val casualFont: Typeface? by lazy {
            Typeface.create("casual", Typeface.BOLD).takeIf { it != Typeface.DEFAULT_BOLD && it != Typeface.DEFAULT }
        }
    }
}

/**
 * Subtitles on a screen of their own, over the video rather than inside it:
 * they stay in view however far the picture is zoomed. Cues come from the
 * engine's own tracks (subtitles inside a file played as it is) and from
 * [serverCues] (the server's WebVTT, merged as it arrives); one draws only
 * text cues, another only [pictures] (PGS, VobSub), so a screen can put
 * pictures inside the visible part of the video and text over all of it.
 *
 * [bottomPadding] keeps them clear of the bottom edge, and lifts them above
 * the player's controls while those show; text is raised further by the
 * look's [CaptionLook.raise]; [sidePadding] and [topPadding]
 * keep long lines and cues placed at the top inside a TV's safe area. The
 * text keeps its size: a share of the whole height, padding or not. Cue
 * changes redraw the view without recomposing anything.
 */
@OptIn(UnstableApi::class)
@Composable
fun ScreenCaptions(
    player: Player?,
    serverCues: State<List<Cue>>,
    look: CaptionLook,
    pictures: Boolean,
    bottomPadding: Dp,
    modifier: Modifier = Modifier,
    sidePadding: Dp = 0.dp,
    topPadding: Dp = 0.dp,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val padding = with(density) { bottomPadding.roundToPx() }
    val side = with(density) { sidePadding.roundToPx() }
    val top = with(density) { topPadding.roundToPx() }
    val system = remember(context) { mutableStateOf(systemCaptionsOn(context)) }
    // The system's style or size changed: the view reads them again.
    val changes = remember { mutableIntStateOf(0) }
    DisposableEffect(context) {
        val manager = context.getSystemService(CaptioningManager::class.java)
        val listener = object : CaptioningManager.CaptioningChangeListener() {
            override fun onEnabledChanged(enabled: Boolean) {
                system.value = enabled
            }

            override fun onUserStyleChanged(userStyle: CaptioningManager.CaptionStyle) {
                changes.intValue++
            }

            override fun onFontScaleChanged(fontScale: Float) {
                changes.intValue++
            }
        }
        manager?.addCaptioningChangeListener(listener)
        onDispose { manager?.removeCaptioningChangeListener(listener) }
    }
    val engineCues = remember { mutableStateOf<List<Cue>>(emptyList()) }
    DisposableEffect(player) {
        val listener = object : Player.Listener {
            override fun onCues(cueGroup: CueGroup) {
                engineCues.value = cueGroup.cues
            }
        }
        if (player != null) {
            engineCues.value = player.currentCues.cues
            player.addListener(listener)
        }
        onDispose {
            player?.removeListener(listener)
            engineCues.value = emptyList()
        }
    }
    val height = remember { mutableIntStateOf(0) }
    AndroidView(
        factory = { SubtitleView(it) },
        modifier = modifier.onSizeChanged { height.intValue = it.height },
        update = { view ->
            changes.intValue
            // The view lays its cues out inside its padding: sizes are scaled back up to the whole height's.
            val h = height.intValue
            // Text only: pictures keep the place their cues give them.
            val bottom = padding + if (pictures) 0 else (look.raise * h).roundToInt()
            val inside = h - bottom - top
            val scale = if (inside in 1 until h) h.toFloat() / inside else 1f
            val manager = view.context.getSystemService(CaptioningManager::class.java)
            if (system.value && !look.custom && manager != null) {
                view.setUserDefaultStyle()
                view.setFractionalTextSize(SubtitleView.DEFAULT_TEXT_SIZE_FRACTION * manager.fontScale * scale)
            } else {
                view.setStyle(look.captionStyle())
                view.setFractionalTextSize(look.textFraction * scale)
            }
            view.setApplyEmbeddedFontSizes(false)
            // Media3 leaves cues on the last line at the very bottom: the view's own padding lifts all of them.
            if (view.paddingBottom != bottom || view.paddingTop != top || view.paddingLeft != side) view.setPadding(side, top, side, bottom)
            val showing = serverCues.value + engineCues.value
            view.setCues(showing.filter { (it.bitmap != null) == pictures })
        },
    )
}

/** Whether the system's captions are on. */
private fun systemCaptionsOn(context: Context): Boolean =
    context.getSystemService(CaptioningManager::class.java)?.isEnabled == true
