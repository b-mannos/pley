package app.pley.android

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** The web app's colours (apps/web/src/index.css). */
object PleyColors {
    val Bg = Color(0xFF0B0D10)
    val Surface = Color(0xFF12151A)
    val Surface2 = Color(0xFF191D24)
    val Border = Color(0xFF2A303B)
    val Fg = Color(0xFFE9EBEF)
    val Muted = Color(0xFFA1A9B6)
    val Subtle = Color(0xFF6C7584)
    val Accent = Color(0xFF38BDF8)
    val AccentFg = Color(0xFF03131F)
    val Success = Color(0xFF3FB27F)
    val Danger = Color(0xFFE5484D)
}

@Composable
fun PleyTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = PleyColors.Accent,
            onPrimary = PleyColors.AccentFg,
            background = PleyColors.Bg,
            onBackground = PleyColors.Fg,
            surface = PleyColors.Surface,
            onSurface = PleyColors.Fg,
            surfaceVariant = PleyColors.Surface2,
            onSurfaceVariant = PleyColors.Muted,
            outline = PleyColors.Border,
            error = PleyColors.Danger,
        ),
        content = content,
    )
}
