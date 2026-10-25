package app.pley.android.ui.phone.kit

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.layout.LazyLayoutCacheWindow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.common.TestTags
import app.pley.android.designsystem.PleyPhone
import app.pley.android.designsystem.image.rememberImagePrefetcher
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.SkeletonState
import app.pley.android.designsystem.tile.SkeletonTile
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.designsystem.tile.TileStyle
import app.pley.android.ui.phone.theme.LocalPhoneMetrics
import kotlin.math.floor
import kotlin.math.max

/**
 * How many columns a grid of [kind] has in [available] width (design spec
 * §3.2): as many cells of [kind]'s minimum width as fit, each giving up
 * at most [FIT_SLACK] of it, and never fewer than 2. A 412 dp phone gets 3
 * columns of about 119 dp, a 393 dp one 3 of 112.
 */
fun gridColumns(kind: TileKind, available: Dp, gap: Dp = PleyPhone.TileGap): Int =
    max(2, floor((available + gap) / (kind.phoneMinWidth * (1f - FIT_SLACK) + gap)).toInt())

/** [gridColumns] as the grid's cells, sharing out the width evenly. */
@Stable
class FittedCells(private val kind: TileKind, private val gap: Dp) : GridCells {
    override fun Density.calculateCrossAxisCellSizes(availableSize: Int, spacing: Int): List<Int> {
        val count = gridColumns(kind, availableSize.toDp(), gap)
        val cells = availableSize - spacing * (count - 1)
        val each = cells / count
        val rest = cells % count
        return List(count) { if (it < rest) each + 1 else each }
    }

    override fun equals(other: Any?): Boolean = other is FittedCells && other.kind == kind && other.gap == gap

    override fun hashCode(): Int = 31 * kind.hashCode() + gap.hashCode()
}

/**
 * A grid of cards (design spec §3.2, §4.2): max(2, width / cell) columns,
 * 12 dp apart, each card keyed by its item. While [items] is null it
 * shows [skeletonCount] skeleton cards in the same shape. [header] goes
 * above the cards, across the whole width (chips, a count). About one
 * screen ahead is composed before it scrolls in, and the pictures of the
 * next 1.5 screens are loaded.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ItemGrid(
    items: List<ArtworkTileModel>?,
    kind: TileKind,
    onOpen: (ArtworkTileModel) -> Unit,
    skeleton: SkeletonState,
    modifier: Modifier = Modifier,
    onLongPress: ((ArtworkTileModel) -> Unit)? = null,
    state: LazyGridState = rememberItemGridState(),
    contentPadding: PaddingValues? = null,
    skeletonCount: Int = 12,
    header: (LazyGridScope.() -> Unit)? = null,
) {
    val metrics = LocalPhoneMetrics.current
    val gap = PleyPhone.TileGap
    val available = metrics.contentWidth - metrics.margin * 2
    val columns = gridColumns(kind, available, gap)
    val cellWidth = (available - gap * (columns - 1)) / columns
    val density = LocalDensity.current
    val images = remember(items) { items?.map { it.image }.orEmpty() }
    rememberImagePrefetcher(
        state = state,
        images = images,
        widthPx = with(density) { cellWidth.roundToPx() },
        heightPx = with(density) { (cellWidth / kind.aspectRatio).roundToPx() },
        tier = LocalPhoneDeviceTier.current,
    )
    LazyVerticalGrid(
        columns = remember(kind, gap) { FittedCells(kind, gap) },
        state = state,
        modifier = modifier.testTag(TestTags.LIBRARY_GRID),
        contentPadding = contentPadding ?: PaddingValues(horizontal = metrics.margin, vertical = PRESS_ROOM + 2.dp),
        horizontalArrangement = Arrangement.spacedBy(gap),
        verticalArrangement = Arrangement.spacedBy(ROW_GAP),
        userScrollEnabled = items != null || header != null,
    ) {
        header?.invoke(this)
        if (items == null) {
            items(skeletonCount, contentType = { SKELETON }) {
                SkeletonTile(skeleton, kind, TileStyle.Phone, width = cellWidth)
            }
        } else {
            items(items, key = { it.id }, contentType = { kind }) { model ->
                MediaCard(
                    model = model,
                    kind = kind,
                    onClick = { onOpen(model) },
                    onLongClick = onLongPress?.let { { it(model) } },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}

/** A header item for [ItemGrid]: across every column. */
fun LazyGridScope.fullWidth(key: Any, content: @Composable () -> Unit) {
    item(key = key, span = { GridItemSpan(maxLineSpan) }, contentType = key) { content() }
}

/** A grid's scroll state, composing about one screen ahead (LazyLayoutCacheWindow). */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun rememberItemGridState(): LazyGridState =
    rememberLazyGridState(cacheWindow = remember { LazyLayoutCacheWindow(aheadFraction = 1f, behindFraction = 0.25f) })

/** Between a card's caption and the next row's art. */
private val ROW_GAP = 16.dp

private const val SKELETON = "skeleton"
