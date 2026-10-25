package app.pley.android.ui.phone.onboarding

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.provider.Settings
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import app.pley.android.screens.onboarding.OnboardingSource
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.net.toUri
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyMark

/**
 * What the screens of getting in read and change: `null` (the app's, from
 * its graph) unless a test or a preview provides its own.
 */
val LocalOnboardingSource = staticCompositionLocalOf<OnboardingSource?> { null }

/** The pley mark and wordmark, as the sign-in screens open (mockup phone-signin). */
@Composable
internal fun PleyWordmark(modifier: Modifier = Modifier) {
    Row(modifier.semantics { heading() }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        Image(rememberVectorPainter(PleyMark.Glyph), contentDescription = null, modifier = Modifier.size(44.dp))
        Text("pley", style = PleyType.Phone.HeadlineMedium.copy(fontSize = 30.sp, lineHeight = 36.sp), color = PleyColors.Fg)
    }
}

/**
 * The sign-in screens' background: [brandDeep][PleyColors.BrandDeep]
 * glowing from the top-right corner over [bg][PleyColors.Bg] (design spec
 * §4.7). One gradient, drawn once.
 */
internal fun Modifier.brandCorner(): Modifier = drawWithCache {
    val brush = Brush.radialGradient(
        0f to PleyColors.BrandDeep.copy(alpha = 0.22f),
        1f to Color.Transparent,
        center = Offset(size.width, 0f),
        radius = maxOf(size.width, 1f) * 1.1f,
    )
    onDrawBehind { drawRect(brush) }
}

/** Lucide `globe`, for "Continue in browser" (not among the design system's icons). */
internal val GlobeIcon = PleyIcon(
    "globe",
    false,
    "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0z",
    "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20",
    "M2 12h20",
)

/** Opens this app's page in the system's settings. */
internal fun openAppSettings(context: Context) {
    try {
        context.startActivity(
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, "package:${context.packageName}".toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    } catch (_: ActivityNotFoundException) {
        // No settings screen for apps: nothing to open.
    }
}

/** Opens [url] in the browser (a server's own page, for "Open in browser"). */
internal fun openInBrowser(context: Context, url: String) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()).addCategory(Intent.CATEGORY_BROWSABLE).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        // No browser: the sign-in screen offers a code instead.
    }
}
