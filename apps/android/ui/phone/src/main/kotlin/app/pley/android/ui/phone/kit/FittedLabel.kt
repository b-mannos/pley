package app.pley.android.ui.phone.kit

import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import app.pley.android.designsystem.text.WholeWordsText

/**
 * A short label in a narrow slot (the tab bar, the item page's actions)
 * that never breaks inside a word, whatever the system's font size: two
 * lines at most, and smaller when a word is wider than the slot
 * ([WholeWordsText]). At 150 % "Libraries" was "Librarie" over "s".
 */
@Composable
fun FittedLabel(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = LocalTextStyle.current,
    color: Color = LocalContentColor.current,
    textAlign: TextAlign = TextAlign.Center,
) {
    WholeWordsText(text, style.merge(color = color, textAlign = textAlign), modifier)
}
