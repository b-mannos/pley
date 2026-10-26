package app.pley.android.ui.tv.kit

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.constrainWidth
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Border
import androidx.tv.material3.Button
import androidx.tv.material3.ButtonDefaults
import androidx.tv.material3.Glow
import androidx.tv.material3.IconButton
import androidx.tv.material3.IconButtonDefaults
import androidx.tv.material3.Text
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyFocus
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon

/**
 * The TV's buttons (design spec §2.5): tv-material's, in pley's look.
 * Focus is always the accent fill with a small scale; disabled buttons are
 * at 40 % and can't take focus, so focus skips them and is never trapped.
 * Buttons in a row sit [PleyTv.ButtonGap] apart.
 */

/**
 * A pill: Play, Resume, Get a new code, Play now. 40 tall, at least 128
 * wide. At rest a faint fill with a [control][PleyColors.Control] edge.
 * [progress] (0 to 1, Resume) draws the played line along its bottom.
 */
@Composable
fun TvPillButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: PleyIcon? = null,
    enabled: Boolean = true,
    progress: Float? = null,
    onLongClick: (() -> Unit)? = null,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    HandOffFocus(enabled, focused)
    Button(
        onClick = onClick,
        onLongClick = onLongClick,
        enabled = enabled,
        interactionSource = interactions,
        modifier = modifier
            .heightIn(min = PleyTv.ButtonHeight)
            .widthIn(min = PillMinWidth)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .focusProperties { canFocus = enabled || focused }
            .then(if (progress != null) Modifier.progressLine(progress) { focused } else Modifier),
        shape = ButtonDefaults.shape(PleyShape.Control),
        colors = ButtonDefaults.colors(
            containerColor = PleyColors.Fg.copy(alpha = 0.12f),
            contentColor = PleyColors.Fg,
            focusedContainerColor = PleyColors.Accent,
            focusedContentColor = PleyColors.OnAccent,
            pressedContainerColor = PleyColors.AccentBright,
            pressedContentColor = PleyColors.OnAccent,
            disabledContainerColor = PleyColors.Fg.copy(alpha = 0.12f),
            disabledContentColor = PleyColors.Fg,
        ),
        scale = ButtonDefaults.scale(focusedScale = PleyFocus.PILL_SCALE, pressedScale = 1f),
        glow = ButtonDefaults.glow(Glow.None, Glow.None, Glow.None),
        border = ButtonDefaults.border(
            border = Border(BorderStroke(1.dp, PleyColors.Control), shape = PleyShape.Control),
            focusedBorder = Border.None,
            pressedBorder = Border.None,
            disabledBorder = Border(BorderStroke(1.dp, PleyColors.Control), shape = PleyShape.Control),
            focusedDisabledBorder = Border.None,
        ),
        // tv-material's Row is at least 40 tall, at the top: as tall as the pill, it centres the label.
        contentPadding = PaddingValues(horizontal = PillPadding, vertical = 0.dp),
    ) {
        // That Row starts its content at the left; a short label sits centred in the 128.
        Row(
            Modifier.widthIn(min = PillMinWidth - PillPadding * 2),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (icon != null) {
                Icon(icon, contentDescription = null, size = PleyTv.IconSize, tint = androidx.tv.material3.LocalContentColor.current)
                Spacer(Modifier.width(8.dp))
            }
            Text(label, style = PleyType.Tv.Label)
        }
    }
}

/**
 * An icon button: From start, Watched, Audio and subtitles, More. A 40 dp
 * square, its corners a [control][PleyShape.Control]'s, with a 20 dp icon
 * and the pill's [control][PleyColors.Control] edge, so it reads on dark art
 * too; [onArt] puts it on [bg][PleyColors.Bg] at 80 % over a backdrop, else
 * on [surface3][PleyColors.Surface3]. Its [label] is
 * always what screen readers hear; on screen it fades in 6 dp below while
 * the button has focus. The button keeps that room ([PleyTv.ButtonLabelRoom])
 * under itself, so a row of buttons never jumps.
 */
@Composable
fun TvRoundButton(
    label: String,
    icon: PleyIcon,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onArt: Boolean = true,
    enabled: Boolean = true,
    onLongClick: (() -> Unit)? = null,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    HandOffFocus(enabled, focused)
    val labelAlpha by animateFloatAsState(if (focused) 1f else 0f, PleyMotion.reveal(), label = "label")
    Box(modifier.width(PleyTv.ButtonHeight).alpha(if (enabled) 1f else DISABLED_ALPHA), contentAlignment = Alignment.TopCenter) {
        IconButton(
            onClick = onClick,
            onLongClick = onLongClick,
            enabled = enabled,
            interactionSource = interactions,
            modifier = Modifier
                .size(PleyTv.ButtonHeight)
                .focusProperties { canFocus = enabled || focused }
                .semantics { contentDescription = label },
            shape = IconButtonDefaults.shape(PleyShape.Control),
            colors = IconButtonDefaults.colors(
                containerColor = if (onArt) PleyColors.OnArt else PleyColors.Surface3,
                contentColor = PleyColors.Fg,
                focusedContainerColor = PleyColors.Accent,
                focusedContentColor = PleyColors.OnAccent,
                pressedContainerColor = PleyColors.AccentBright,
                pressedContentColor = PleyColors.OnAccent,
                disabledContainerColor = if (onArt) PleyColors.OnArt else PleyColors.Surface3,
                disabledContentColor = PleyColors.Fg,
            ),
            scale = IconButtonDefaults.scale(focusedScale = PleyFocus.ROUND_SCALE, pressedScale = 1f),
            glow = IconButtonDefaults.glow(Glow.None, Glow.None, Glow.None),
            border = IconButtonDefaults.border(
                border = Border(BorderStroke(1.dp, PleyColors.Control), shape = PleyShape.Control),
                focusedBorder = Border.None,
                pressedBorder = Border.None,
                disabledBorder = Border(BorderStroke(1.dp, PleyColors.Control), shape = PleyShape.Control),
                focusedDisabledBorder = Border.None,
            ),
        ) {
            Icon(icon, contentDescription = null, size = PleyTv.IconSize, tint = androidx.tv.material3.LocalContentColor.current)
        }
        // The visible label, under the button; the button above already says it to screen readers.
        BasicText(
            label,
            style = PleyType.Tv.Caption.copy(color = PleyColors.Fg, textAlign = TextAlign.Center),
            maxLines = 1,
            softWrap = false,
            modifier = Modifier
                .padding(top = PleyTv.ButtonHeight + 6.dp)
                .graphicsLayer { alpha = labelAlpha }
                .wrapContentWidthUnbounded(),
        )
    }
}

/** A text button: Change server, Cancel, Clear filters. 40 tall, no fill until focused. */
@Composable
fun TvTextButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    color: Color = PleyColors.Fg,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    HandOffFocus(enabled, focused)
    Button(
        onClick = onClick,
        enabled = enabled,
        interactionSource = interactions,
        modifier = modifier
            .heightIn(min = 40.dp)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .focusProperties { canFocus = enabled || focused },
        shape = ButtonDefaults.shape(PleyShape.Control),
        colors = ButtonDefaults.colors(
            containerColor = Color.Transparent,
            contentColor = color,
            focusedContainerColor = PleyColors.Accent,
            focusedContentColor = PleyColors.OnAccent,
            pressedContainerColor = PleyColors.AccentBright,
            pressedContentColor = PleyColors.OnAccent,
            disabledContainerColor = Color.Transparent,
            disabledContentColor = color,
        ),
        scale = ButtonDefaults.scale(focusedScale = PleyFocus.PILL_SCALE, pressedScale = 1f),
        glow = ButtonDefaults.glow(Glow.None, Glow.None, Glow.None),
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) {
        Text(label, style = PleyType.Tv.Label)
    }
}

/** Disabled buttons are drawn at 40 %. */
private const val DISABLED_ALPHA = 0.4f

/** A pill's least width, and the room either side of its label. */
private val PillMinWidth = 128.dp
private val PillPadding = 20.dp

/**
 * Disabled buttons can't take focus (§2.5), and focus always shows (§5.2
 * rule 1): a button disabled while it has focus keeps it only until it has
 * handed it to a neighbour, across first, then down or up.
 */
@Composable
private fun HandOffFocus(enabled: Boolean, focused: Boolean) {
    val focus = LocalFocusManager.current
    LaunchedEffect(enabled, focused) {
        if (!enabled && focused) {
            listOf(FocusDirection.Right, FocusDirection.Left, FocusDirection.Down, FocusDirection.Up).firstOrNull { focus.moveFocus(it) }
        }
    }
}

/**
 * An in-progress pill's line (design spec §2.5): 3 dp, 5 dp above the
 * bottom, inset by the side padding. At rest accent over fg 20 %; focused
 * onAccent over onAccent 25 %.
 */
private fun Modifier.progressLine(progress: Float, focused: () -> Boolean): Modifier = drawWithContent {
    drawContent()
    val h = 3.dp.toPx()
    val inset = PillPadding.toPx()
    val y = size.height - 5.dp.toPx() - h
    val w = (size.width - 2 * inset).coerceAtLeast(0f)
    val on = focused()
    val track = if (on) PleyColors.OnAccent.copy(alpha = 0.25f) else PleyColors.Fg.copy(alpha = 0.2f)
    val fill = if (on) PleyColors.OnAccent else PleyColors.Accent
    val r = CornerRadius(h / 2f, h / 2f)
    drawRoundRect(track, Offset(inset, y), Size(w, h), r)
    drawRoundRect(fill, Offset(inset, y), Size(w * progress.coerceIn(0f, 1f), h), r)
}

/** Lets a label be wider than the button it sits under, centred on it. */
private fun Modifier.wrapContentWidthUnbounded(): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints.copy(minWidth = 0, maxWidth = androidx.compose.ui.unit.Constraints.Infinity))
    val width = constraints.constrainWidth(placeable.width)
    layout(width, placeable.height) { placeable.place((width - placeable.width) / 2, 0) }
}
