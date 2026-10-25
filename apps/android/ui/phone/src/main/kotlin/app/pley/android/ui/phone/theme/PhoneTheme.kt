package app.pley.android.ui.phone.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import app.pley.android.designsystem.PleyColors

/** Material 3 in pley's colours (dark only, as the web). Phones and tablets only. */
@Composable
fun PhoneTheme(content: @Composable () -> Unit) {
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
