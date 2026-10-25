package app.pley.android.ui.phone.theme

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyPhone

/** Material's window width classes, by the window's width. */
enum class WidthClass {
    /** Under 600 dp: phones held upright. The navigation bar. */
    Compact,

    /** 600 to 840 dp: small tablets, phones on their side, unfolded foldables. The rail. */
    Medium,

    /** 840 dp and up: tablets. The rail. */
    Expanded,
    ;

    companion object {
        fun of(windowWidth: Dp): WidthClass = when {
            windowWidth < 600.dp -> Compact
            windowWidth < 840.dp -> Medium
            else -> Expanded
        }
    }
}

/**
 * The page's measures for this window (design spec §1.3): the side margin
 * by width class, and how wide the page's content is, beside the rail
 * when there is one. Rows and grids size their cards from [contentWidth]
 * without measuring themselves, so a screen of rows lays out in one pass.
 */
@Immutable
data class PhoneMetrics(
    val widthClass: WidthClass,
    /** The page's width: the window's, less the rail's. */
    val contentWidth: Dp,
) {
    /** The side margin: 16 compact, 24 medium, 32 expanded. */
    val margin: Dp
        get() = when (widthClass) {
            WidthClass.Compact -> PleyPhone.MarginCompact
            WidthClass.Medium -> PleyPhone.MarginMedium
            WidthClass.Expanded -> PleyPhone.MarginExpanded
        }

    /** Whether navigation is a rail beside the page rather than a bar under it. */
    val rail: Boolean get() = widthClass != WidthClass.Compact

    companion object {
        /** The collapsed navigation rail (WideNavigationRailCollapsed). */
        val RailWidth = 96.dp

        /** A window [windowWidth] wide. */
        fun of(windowWidth: Dp): PhoneMetrics {
            val widthClass = WidthClass.of(windowWidth)
            val content = if (widthClass == WidthClass.Compact) windowWidth else windowWidth - RailWidth
            return PhoneMetrics(widthClass, content.coerceAtLeast(0.dp))
        }
    }
}

/** The page's [PhoneMetrics]; [PhoneTheme] provides them for the window. */
val LocalPhoneMetrics = staticCompositionLocalOf { PhoneMetrics.of(412.dp) }

@Composable
internal fun rememberPhoneMetrics(): PhoneMetrics {
    val width = LocalWindowInfo.current.containerSize.width
    val density = LocalDensity.current
    return remember(width, density) { PhoneMetrics.of(with(density) { width.toDp() }) }
}
