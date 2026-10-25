package app.pley.android.ui.phone.home

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.platform.LocalDensity
import app.pley.android.common.perf.StartupSignals
import app.pley.android.designsystem.image.LocalArtworkImageLoader
import app.pley.android.ui.phone.kit.rowCardWidth
import app.pley.android.ui.phone.theme.LocalPhoneMetrics
import coil3.SingletonImageLoader
import coil3.compose.LocalPlatformContext
import coil3.request.ImageRequest
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withTimeoutOrNull

/**
 * Tells the shell Home is drawn (StartupSignals.homeDrawn, its
 * ReportDrawnWhen: the benchmarks' time to full display): the first frame
 * with content, once the first row's first pictures are in at the cards'
 * size (or after 2 s).
 */
@Composable
internal fun MarkHomeDrawn(model: PhoneHomeModel) {
    val context = LocalPlatformContext.current
    val loader = LocalArtworkImageLoader.current ?: SingletonImageLoader.get(context)
    val density = LocalDensity.current
    val metrics = LocalPhoneMetrics.current
    LaunchedEffect(Unit) {
        if (StartupSignals.homeDrawn.value) return@LaunchedEffect
        withFrameNanos { }
        val row = model.rows.firstOrNull()?.model
        val tiles = row?.items
        if (row != null && tiles != null) {
            val width = rowCardWidth(row.kind, metrics.contentWidth - metrics.margin * 2)
            val w = with(density) { width.roundToPx() }
            val h = with(density) { (width / row.kind.aspectRatio).roundToPx() }
            withTimeoutOrNull(FIRST_ROW_WAIT_MS) {
                coroutineScope {
                    tiles.take(FIRST_ROW_TILES).mapNotNull { it.image }
                        .map { image -> async { loader.execute(ImageRequest.Builder(context).data(image).size(w, h).build()) } }
                        .awaitAll()
                }
            }
        }
        StartupSignals.markHomeDrawn()
    }
}

private const val FIRST_ROW_WAIT_MS = 2_000L
private const val FIRST_ROW_TILES = 3
