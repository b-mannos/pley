package app.pley.android.ui.tv.kit

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.BiasAlignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Surface
import androidx.tv.material3.SurfaceDefaults
import app.pley.android.common.Tier
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyElevation
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.image.Artwork
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.designsystem.image.GradientBitmaps
import app.pley.android.designsystem.image.drawStretched
import app.pley.android.designsystem.reducedMotion
import app.pley.android.ui.tv.theme.LocalTvTier

/**
 * Where the bottom gradient over a backdrop runs (design spec §2.2, §4.1,
 * §4.3): [bg][PleyColors.Bg] at 0 % at [from] to 100 % at [to], from the
 * top of the screen.
 */
@Immutable
data class BackdropScrim(val from: Dp, val to: Dp) {
    companion object {
        /** Home at rest, under the hero. */
        val Hero = BackdropScrim(220.dp, 410.dp)

        /** Home in the rows, under the info block. */
        val Rows = BackdropScrim(120.dp, 300.dp)

        /** Item pages. */
        val Item = BackdropScrim(260.dp, 500.dp)
    }
}

/**
 * The page's backdrop (design spec §2.2, §3.6 of the TV addendum): one
 * picture layer under everything, cropped towards its subject (right of
 * centre, above the middle), with the two gradients that keep the text
 * column on at least 85 % [bg][PleyColors.Bg], drawn together.
 *
 * It follows [image] only once focus has rested ([rememberRested]): a
 * 300 ms crossfade on T1 and T2, on the new picture's layer alone
 * (ModulateAlpha, no offscreen buffer); an instant swap on T0 and with
 * reduced motion. Pictures are asked for at most 1280 wide, 960 on T0.
 * Never blurred.
 */
@Composable
fun TvBackdrop(
    image: ArtworkImage?,
    title: String,
    modifier: Modifier = Modifier,
    scrim: BackdropScrim = BackdropScrim.Hero,
) {
    val tier = LocalTvTier.current
    val cap = ArtworkImage.backdropCap(tier)
    val wanted = remember(image, title, cap) { Backdrop(image?.let { if (it.maxWidth in 1..cap) it else it.copy(maxWidth = cap) }, title) }
    val shown = rememberRested(wanted)
    val instant = tier == Tier.T0 || reducedMotion()
    var under by remember { mutableStateOf<Backdrop?>(null) }
    var top by remember { mutableStateOf(shown) }
    val fade = remember { Animatable(1f) }
    LaunchedEffect(shown) {
        if (shown == top) return@LaunchedEffect
        if (instant) {
            top = shown
            under = null
            return@LaunchedEffect
        }
        under = top
        top = shown
        fade.snapTo(0f)
        fade.animateTo(1f, tween(PleyMotion.BACKDROP_MS, easing = LinearEasing))
        under = null
    }
    Box(
        modifier
            .fillMaxSize()
            // The whole stack (picture, placeholder, scrims) is drawn once into a
            // layer that is kept: while focus moves over the rows nothing in it
            // changes, so each frame costs one copy of it instead of four or five
            // full-screen passes (43 ms of GPU a frame down to 13 on a Google TV
            // Streamer). It is drawn again only while the backdrop changes.
            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
            .backdropScrims(scrim),
    ) {
        under?.let { BackdropPicture(it, Modifier.fillMaxSize()) }
        BackdropPicture(
            top,
            Modifier
                .fillMaxSize()
                .graphicsLayer {
                    alpha = fade.value
                    compositingStrategy = CompositingStrategy.ModulateAlpha
                },
        )
    }
}

@Immutable
private data class Backdrop(val image: ArtworkImage?, val title: String)

@Composable
private fun BackdropPicture(backdrop: Backdrop, modifier: Modifier) {
    Artwork(backdrop.image, backdrop.title, modifier, alignment = SubjectAlignment)
}

/** Subjects sit right of centre and above the middle. */
private val SubjectAlignment = BiasAlignment(0.4f, -0.6f)

/**
 * The backdrop's two gradients in one pass over it: from the left, bg at
 * 100 % (x = 0), 92 % (300), 85 % (560), 0 % (800); from the top, as
 * [scrim] says.
 */
private fun Modifier.backdropScrims(scrim: BackdropScrim): Modifier = drawWithCache {
    // Stretched pictures, not gradient shaders (GradientBitmaps): over the whole
    // screen a shader cost about 13 ms a frame each on a TV's GPU.
    val leftEnd = minOf(800.dp.toPx(), size.width)
    val from = scrim.from.toPx()
    val to = scrim.to.toPx()
    onDrawWithContent {
        drawContent()
        drawStretched(LeftScrim, Offset.Zero, Size(leftEnd, size.height))
        drawStretched(BottomScrim, Offset(0f, from), Size(size.width, to - from))
        if (to < size.height) drawRect(PleyColors.Bg, topLeft = Offset(0f, to), size = Size(size.width, size.height - to))
    }
}

/** From the left, over 800 dp: bg at 100 % (x = 0), 92 % (300), 85 % (560), 0 % (800). */
private val LeftScrim by lazy {
    GradientBitmaps.horizontal(
        0f to PleyColors.Bg,
        300f / 800f to PleyColors.Bg.copy(alpha = 0.92f),
        560f / 800f to PleyColors.Bg.copy(alpha = 0.85f),
        1f to Color.Transparent,
    )
}

/** From the top of its band: bg at 0 % to 100 %. */
private val BottomScrim by lazy { GradientBitmaps.vertical(0f to Color.Transparent, 1f to PleyColors.Bg) }

/**
 * A side panel (design spec §2.7; Subtitles, Audio, Quality, Info): 360 dp
 * from the right edge, full height, [surface2][PleyColors.Surface2] with
 * 16 dp left corners, over a [scrim][PleyColors.Scrim]. It slides in over
 * 200 ms (in place with reduced motion), takes focus ([initialFocus] on
 * the row to start from), and Back closes it ([onDismiss]).
 */
@Composable
fun TvSidePanel(
    visible: Boolean,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    val reduced = reducedMotion()
    BackHandler(enabled = visible, onBack = onDismiss)
    Box(Modifier.fillMaxSize()) {
        AnimatedVisibility(visible, enter = fadeIn(tween(if (reduced) 0 else PleyMotion.OVERLAY_MS)), exit = fadeOut(tween(if (reduced) 0 else PleyMotion.OVERLAY_MS))) {
            Box(Modifier.fillMaxSize().background(PleyColors.Scrim))
        }
        AnimatedVisibility(
            visible,
            modifier = Modifier.align(Alignment.CenterEnd),
            enter = slideInHorizontally(PleyMotion.overlay(reduced)) { it },
            exit = slideOutHorizontally(PleyMotion.overlay(reduced)) { it },
        ) {
            Surface(
                modifier = modifier.width(SidePanelWidth).fillMaxHeight().focusGroup(),
                shape = RoundedCornerShape(topStart = PleyShape.DialogRadius, bottomStart = PleyShape.DialogRadius),
                colors = SurfaceDefaults.colors(containerColor = PleyColors.Surface2, contentColor = PleyColors.Fg),
            ) {
                Column(
                    Modifier
                        .fillMaxSize()
                        .topHairline()
                        .padding(start = 24.dp, end = TvSafeArea.Horizontal, top = TvSafeArea.Vertical, bottom = TvSafeArea.Vertical),
                    content = content,
                )
            }
        }
    }
}

/** Level 2's 1 dp [surface4][PleyColors.Surface4] line along the top. */
private fun Modifier.topHairline(): Modifier = drawWithCache {
    val h = PleyElevation.Level2Hairline.toPx()
    onDrawWithContent {
        drawContent()
        drawRect(PleyColors.Surface4, size = androidx.compose.ui.geometry.Size(size.width, h))
    }
}

private val SidePanelWidth = 360.dp
