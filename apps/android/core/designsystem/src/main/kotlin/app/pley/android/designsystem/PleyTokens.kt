package app.pley.android.designsystem

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * The web app's colours (apps/web/src/index.css), as plain values: the
 * phone builds its Material 3 theme from them and the TV its tv-material
 * one, so both look like pley without sharing a theme.
 */
object PleyColors {
    val Accent = Color(0xFF38BDF8)
    val AccentHover = Color(0xFF7DD3FC)
    val AccentFg = Color(0xFF03131F)

    val BrandLight = Color(0xFFBAE6FD)
    val BrandDeep = Color(0xFF0284C7)
    val BrandTile = Color(0xFF0A1622)

    val Bg = Color(0xFF0B0D10)
    val Surface = Color(0xFF12151A)
    val Surface2 = Color(0xFF191D24)
    val Surface3 = Color(0xFF222731)
    val Border = Color(0xFF2A303B)
    val BorderStrong = Color(0xFF3A4250)

    val Fg = Color(0xFFE9EBEF)
    val Muted = Color(0xFFA1A9B6)
    val Subtle = Color(0xFF6C7584)

    val Success = Color(0xFF3FB27F)
    val Danger = Color(0xFFE5484D)
    val Info = Color(0xFF4C9AFF)
}

/** Corner radii, as the web's rounded, rounded-lg and rounded-xl. */
object PleyShapes {
    val Small = 6.dp
    val Medium = 8.dp
    val Large = 12.dp
}

/**
 * What a TV keeps clear around the picture: some TVs crop the edges
 * (overscan). 48 dp at the sides and 27 dp above and below, 5% of the
 * 960 by 540 dp screen Fire TV and Google TV draw on.
 */
object TvSafeArea {
    val Horizontal = 48.dp
    val Vertical = 27.dp
}
