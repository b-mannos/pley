package app.pley.android.ui.tv.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.tv.material3.ColorScheme
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Shapes
import androidx.tv.material3.Typography
import androidx.tv.material3.darkColorScheme
import app.pley.android.common.Tier
import app.pley.android.designsystem.LocalReducedMotion
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyType

/**
 * The TV's look: tv-material's theme built from pley's tokens (design spec
 * §1.1, §1.2, §1.4), plus the device's [tier] and whether to move less.
 *
 * The TV never uses Compose Material 3: the two libraries each have a
 * MaterialTheme, and mixing them mixes colours, type and shapes
 * (https://developer.android.com/training/tv/playback/compose). tv-material
 * starts from its light scheme and the phone's type scale, so every value
 * is set here, none left to its defaults.
 *
 * [reducedMotion] is pley's own "Reduce motion" switch; `null` leaves it to
 * the system's "Remove animations".
 */
@Composable
fun TvTheme(
    tier: Tier = Tier.T2,
    reducedMotion: Boolean? = null,
    content: @Composable () -> Unit,
) {
    val reduced = reducedMotion ?: PleyMotion.systemReducedMotion(LocalContext.current)
    MaterialTheme(colorScheme = TvColors, shapes = TvShapes, typography = TvTypography) {
        CompositionLocalProvider(
            LocalTvTier provides tier,
            LocalReducedMotion provides reduced,
            content = content,
        )
    }
}

/**
 * The device's tier (architecture.md, design spec §6): T0 swaps backdrops
 * at once and asks for smaller pictures.
 */
val LocalTvTier = staticCompositionLocalOf { Tier.T2 }

/**
 * The scheme of design spec §1.1, the phone's roles with `border` as the
 * focus ring's colour and no tint on surfaces.
 */
internal val TvColors: ColorScheme = darkColorScheme(
    primary = PleyColors.Accent,
    onPrimary = PleyColors.OnAccent,
    primaryContainer = PleyColors.PrimaryContainer,
    onPrimaryContainer = PleyColors.AccentBright,
    inversePrimary = PleyColors.BrandDeep,
    secondary = PleyColors.Surface3,
    onSecondary = PleyColors.Fg,
    secondaryContainer = PleyColors.Surface3,
    onSecondaryContainer = PleyColors.Fg,
    tertiary = PleyColors.BrandLight,
    onTertiary = PleyColors.OnAccent,
    tertiaryContainer = PleyColors.Surface3,
    onTertiaryContainer = PleyColors.BrandLight,
    background = PleyColors.Bg,
    onBackground = PleyColors.Fg,
    surface = PleyColors.Bg,
    onSurface = PleyColors.Fg,
    surfaceVariant = PleyColors.Surface2,
    onSurfaceVariant = PleyColors.Muted,
    surfaceTint = Color.Transparent,
    inverseSurface = PleyColors.Fg,
    inverseOnSurface = PleyColors.Bg,
    error = PleyColors.Danger,
    onError = PleyColors.Bg,
    errorContainer = PleyColors.Surface3,
    onErrorContainer = PleyColors.DangerText,
    border = PleyColors.AccentBright,
    borderVariant = PleyColors.Control,
    scrim = PleyColors.Bg,
)

/** tv-material's roles set to pley's TV type (design spec §1.2). */
internal val TvTypography = Typography(
    displayLarge = PleyType.Tv.HeroTitle,
    displayMedium = PleyType.Tv.HeroTitle,
    displaySmall = PleyType.Tv.PageTitle,
    headlineLarge = PleyType.Tv.PageTitle,
    headlineMedium = PleyType.Tv.PageTitle,
    headlineSmall = PleyType.Tv.Section,
    titleLarge = PleyType.Tv.Section,
    titleMedium = PleyType.Tv.Title,
    titleSmall = PleyType.Tv.Label,
    bodyLarge = PleyType.Tv.BodyLarge,
    bodyMedium = PleyType.Tv.Body,
    bodySmall = PleyType.Tv.Caption,
    labelLarge = PleyType.Tv.Label,
    labelMedium = PleyType.Tv.Caption,
    labelSmall = PleyType.Tv.Badge,
)

/** Badges 4, fields and tiles 8, surfaces 12, dialogs and panels 16, sheets 20 (design spec §1.4). */
internal val TvShapes = Shapes(
    extraSmall = RoundedCornerShape(PleyShape.BadgeRadius),
    small = RoundedCornerShape(PleyShape.FieldRadius),
    medium = RoundedCornerShape(PleyShape.CardRadius),
    large = RoundedCornerShape(PleyShape.DialogRadius),
    extraLarge = RoundedCornerShape(PleyShape.SheetRadius),
)
