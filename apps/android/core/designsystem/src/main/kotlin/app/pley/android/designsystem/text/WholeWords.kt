package app.pley.android.designsystem.text

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.isSpecified
import androidx.compose.ui.unit.sp

/**
 * Text in a narrow slot that never breaks inside a word: at its own size
 * unless one of its words is wider than the slot, then as much smaller as
 * that word needs (down to [MinSize]). With a large system font, "Libraries"
 * was "Librarie" over "s" and "The Glasshouse" "The Gla" over "sshouse";
 * Android hyphenates neither. Measured and drawn in one layout, so a
 * navigation bar can ask for its intrinsic size.
 */
@Composable
fun WholeWordsText(
    text: String,
    style: TextStyle,
    modifier: Modifier = Modifier,
    maxLines: Int = 2,
) {
    val measurer = rememberTextMeasurer()
    val laid = remember { arrayOfNulls<TextLayoutResult>(1) }
    Layout(
        modifier = modifier
            .semantics { this.text = AnnotatedString(text) }
            .drawBehind { laid[0]?.let { drawText(it) } },
    ) { _, constraints ->
        val fitted = fitted(measurer, text, style, constraints.maxWidth)
        val result = measurer.measure(
            text,
            fitted,
            overflow = TextOverflow.Ellipsis,
            maxLines = maxLines,
            constraints = Constraints(maxWidth = constraints.maxWidth),
        )
        laid[0] = result
        layout(
            result.size.width.coerceIn(constraints.minWidth, constraints.maxWidth),
            result.size.height.coerceIn(constraints.minHeight, constraints.maxHeight),
        ) {}
    }
}

/** [style], smaller by half points until the widest words fit [width] each on a line. */
private fun fitted(measurer: TextMeasurer, text: String, style: TextStyle, width: Int): TextStyle {
    if (!style.fontSize.isSpecified || width <= 0 || width == Constraints.Infinity) return style
    val words = text.split(' ', '\n', '\t').filter { it.isNotEmpty() }.sortedByDescending { it.length }.take(3)
    val max = style.fontSize.value
    var size = max
    fun fits(s: Float) = words.all { measurer.measure(it, style.copy(fontSize = s.sp), maxLines = 1, softWrap = false).size.width <= width }
    while (size > MinSize && !fits(size)) size -= Step
    if (size == max) return style
    val ratio = size / max
    return style.copy(fontSize = size.sp, lineHeight = style.lineHeight.scaled(ratio))
}

private fun TextUnit.scaled(ratio: Float): TextUnit = if (isSpecified) (value * ratio).sp else this

private const val MinSize = 9f
private const val Step = 0.5f
