package app.pley.android.designsystem.icon

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import app.pley.android.designsystem.PleyColors

/**
 * A skip of [seconds], as players draw it: Lucide's turning arrow, back
 * (counter-clockwise) or [forward], with the number inside. [size] square
 * in [tint]; decoration, so the button it is in says what it does. The
 * number keeps its share of the icon whatever the font scale.
 */
@Composable
fun SkipIcon(
    forward: Boolean,
    seconds: Int,
    modifier: Modifier = Modifier,
    size: Dp = 24.dp,
    tint: Color = PleyColors.Fg,
) {
    val density = LocalDensity.current
    val style = remember(size, tint, density) {
        TextStyle(
            color = tint,
            fontSize = with(density) { (size * NUMBER_SHARE).toSp() },
            fontWeight = FontWeight.Bold,
            letterSpacing = (-0.02).em,
            textAlign = TextAlign.Center,
            lineHeight = 1.em,
            lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.Both),
        )
    }
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        Icon(if (forward) PleyIcons.RotateCw else PleyIcons.RotateCcw, contentDescription = null, size = size, tint = tint)
        BasicText(seconds.toString(), style = style, maxLines = 1, softWrap = false)
    }
}

/** The number's size against the icon's: it fits inside the arrow's circle (16 of 24). */
private const val NUMBER_SHARE = 0.36f
