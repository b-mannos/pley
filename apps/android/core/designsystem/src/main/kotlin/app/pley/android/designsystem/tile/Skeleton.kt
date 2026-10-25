package app.pley.android.designsystem.tile

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.platform.InspectorInfo
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.reducedMotion

/**
 * The loading look of a screen with nothing saved yet (design spec §8):
 * wells and caption bars in the exact shapes of what is coming. On TV they
 * are still; on phones one shimmer runs per screen, shared by every
 * skeleton on it, and stops with reduced motion. Its position is read only
 * while drawing, so the shimmer never recomposes anything.
 */
@Stable
class SkeletonState internal constructor(internal val shimmer: State<Float>?)

/** One skeleton look per screen. [animated] is false on TV; reduced motion stops it anyway. */
@Composable
fun rememberSkeleton(animated: Boolean): SkeletonState {
    if (!animated || reducedMotion()) return remember { SkeletonState(null) }
    val transition = rememberInfiniteTransition(label = "skeleton")
    val shimmer = transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(SHIMMER_MS, easing = LinearEasing), RepeatMode.Restart),
        label = "shimmer",
    )
    return remember(shimmer) { SkeletonState(shimmer) }
}

private const val SHIMMER_MS = 1_200

/** Paints [shape] in [PleyColors.Surface2], with [state]'s shimmer passing over it. */
fun Modifier.skeleton(state: SkeletonState, shape: Shape): Modifier = this then SkeletonElement(state, shape)

/** A tile still loading: its well, and two caption bars of 60 % and 35 % of its width. */
@Composable
fun SkeletonTile(
    state: SkeletonState,
    kind: TileKind,
    style: TileStyle,
    modifier: Modifier = Modifier,
    width: Dp = kind.tvWidth,
) {
    Column(modifier.width(width).clearAndSetSemantics {}) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(width / kind.aspectRatio)
                .skeleton(state, style.shape(kind)),
        )
        Spacer(Modifier.height(style.captionGap))
        val line = style.captionTitle.lineHeight.value.dp
        Bar(state, 0.6f, line)
        Bar(state, 0.35f, line)
    }
}

@Composable
private fun Bar(state: SkeletonState, fraction: Float, line: Dp) {
    Box(Modifier.height(line), propagateMinConstraints = false) {
        Box(
            Modifier
                .fillMaxWidth(fraction)
                .height(line * 0.6f)
                .skeleton(state, PleyShape.Badge),
        )
    }
}

private data class SkeletonElement(val state: SkeletonState, val shape: Shape) : ModifierNodeElement<SkeletonNode>() {
    override fun create() = SkeletonNode(state, shape)

    override fun update(node: SkeletonNode) {
        node.state = state
        node.shape = shape
        node.invalidateDraw()
    }

    override fun InspectorInfo.inspectableProperties() {
        name = "skeleton"
    }
}

private class SkeletonNode(var state: SkeletonState, var shape: Shape) : Modifier.Node(), DrawModifierNode {
    override fun ContentDrawScope.draw() {
        val outline = shape.createOutline(size, layoutDirection, this)
        drawOutline(outline, PleyColors.Surface2)
        val t = state.shimmer?.value
        if (t != null) {
            val band = size.width.coerceAtLeast(1f)
            val x = -band + t * (size.width + 2 * band)
            drawOutline(
                outline,
                Brush.linearGradient(
                    colors = listOf(PleyColors.Surface2, PleyColors.Surface3, PleyColors.Surface2),
                    start = Offset(x - band / 2f, 0f),
                    end = Offset(x + band / 2f, 0f),
                ),
            )
        }
        drawContent()
    }
}
