package app.pley.android.ui.tv.theme

import androidx.compose.runtime.Composable
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme
import app.pley.android.designsystem.PleyColors

/**
 * tv-material in pley's colours. The TV never uses Compose Material 3: the
 * two libraries each have a MaterialTheme, and mixing them mixes colours,
 * type and shapes.
 */
@Composable
fun TvTheme(content: @Composable () -> Unit) {
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
            border = PleyColors.BorderStrong,
            error = PleyColors.Danger,
        ),
        content = content,
    )
}
