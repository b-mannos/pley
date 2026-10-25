package app.pley.android.ui.phone.kit

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.snapping.SnapPosition
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.layout.LazyLayoutCacheWindow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.common.TestTags
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyPhone
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.image.rememberImagePrefetcher
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.SkeletonState
import app.pley.android.designsystem.tile.SkeletonTile
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.designsystem.tile.TileStyle
import app.pley.android.ui.phone.theme.LocalPhoneMetrics
import androidx.compose.ui.platform.testTag
import kotlin.math.floor
import kotlin.math.max

/**
 * How wide a row's cards are (design spec §3.2, the web's fitCards): cards
 * of [kind]'s minimum width (posters 118, squares 132, landscape 236),
 * grown up to 1.35x on wide pages, fitted so the page's [available] width
 * holds whole cards and the next one peeks into the margin. To fit one
 * more whole card a card may give up to [FIT_SLACK] of its width: a 393 dp
 * phone shows three posters of 112 rather than two of 174. When not even
 * two fit (landscape on a phone), the card keeps its own width so the next
 * one still shows that the row scrolls.
 */
fun rowCardWidth(kind: TileKind, available: Dp, gap: Dp = PleyPhone.TileGap): Dp {
    val growth = (available / WIDE_PAGE).coerceIn(1f, PleyPhone.MAX_CARD_GROWTH)
    val wanted = kind.phoneMinWidth * growth
    val perPage = max(1, floor((available + gap) / (wanted * (1f - FIT_SLACK) + gap)).toInt())
    if (perPage < 2) return wanted
    return minOf(wanted * PleyPhone.MAX_CARD_GROWTH, floor(((available - gap * (perPage - 1)) / perPage).value).dp)
}

/** How much of its minimum width a card may give up so a row or grid fits one more whole card. */
const val FIT_SLACK = 0.15f

/** Pages this wide and wider grow cards past their minimum, up to [PleyPhone.MAX_CARD_GROWTH] at 1.35x it. */
private val WIDE_PAGE = 840.dp

/** One row of a hub: what it holds, or null while it loads. */
@Immutable
data class HubRowModel(
    /** The hub's key, stable across refreshes: the row's key in the page. */
    val key: String,
    val title: String,
    val kind: TileKind,
    /** null: still loading (skeleton cards). */
    val items: List<ArtworkTileModel>?,
)

/**
 * A row of a hub (design spec §3.2, §4.1): its title in titleLarge, then
 * whole cards that scroll sideways and snap to a card's start. Cards are
 * keyed by item and typed by [HubRowModel.kind]; about one screen of cards
 * beyond the visible ones is composed ahead, and their pictures 1.5
 * screens ahead ([rememberImagePrefetcher]). A loading row shows skeleton
 * cards in the same shapes.
 *
 * [onTitleClick] makes the title a link ("Recently Added in Movies" opens
 * the library sorted that way), with a chevron.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun HubRow(
    row: HubRowModel,
    onOpen: (ArtworkTileModel) -> Unit,
    skeleton: SkeletonState,
    modifier: Modifier = Modifier,
    onLongPress: ((ArtworkTileModel) -> Unit)? = null,
    onTitleClick: (() -> Unit)? = null,
    state: LazyListState = rememberHubRowState(),
) {
    KitProbe.rowComposed?.let { probe -> SideEffect { probe(row.key) } }
    val metrics = LocalPhoneMetrics.current
    val width = rowCardWidth(row.kind, metrics.contentWidth - metrics.margin * 2)
    Column(modifier.fillMaxWidth()) {
        RowHeader(row.title, metrics.margin, onTitleClick)
        Spacer(Modifier.height(PleyPhone.HeaderToContent - PRESS_ROOM))
        val items = row.items
        val density = LocalDensity.current
        val images = remember(items) { items?.map { it.image }.orEmpty() }
        rememberImagePrefetcher(
            state = state,
            images = images,
            widthPx = with(density) { width.roundToPx() },
            heightPx = with(density) { (width / row.kind.aspectRatio).roundToPx() },
            tier = LocalPhoneDeviceTier.current,
        )
        LazyRow(
            state = state,
            modifier = Modifier.fillMaxWidth().testTag(TestTags.HOME_ROWS + "." + row.key),
            contentPadding = PaddingValues(horizontal = metrics.margin, vertical = PRESS_ROOM),
            horizontalArrangement = Arrangement.spacedBy(PleyPhone.TileGap),
            flingBehavior = rememberSnapFlingBehavior(state, SnapPosition.Start),
            userScrollEnabled = items != null,
        ) {
            if (items == null) {
                items(SKELETON_CARDS, contentType = { SKELETON }) {
                    SkeletonTile(skeleton, row.kind, TileStyle.Phone, width = width)
                }
            } else {
                items(items, key = { it.id }, contentType = { row.kind }) { model ->
                    MediaCard(
                        model = model,
                        kind = row.kind,
                        onClick = { onOpen(model) },
                        onLongClick = onLongPress?.let { { it(model) } },
                        modifier = Modifier.width(width),
                    )
                }
            }
        }
    }
}

/**
 * A row's scroll state, composing about one screen of cards ahead of the
 * visible ones and a quarter behind (LazyLayoutCacheWindow), so a fling
 * finds them ready.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun rememberHubRowState(): LazyListState =
    rememberLazyListState(cacheWindow = remember { LazyLayoutCacheWindow(aheadFraction = 1f, behindFraction = 0.25f) })

@Composable
private fun RowHeader(title: String, margin: Dp, onClick: (() -> Unit)?) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 32.dp)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = margin)
            .semantics { heading() },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleLarge,
            color = PleyColors.Fg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        if (onClick != null) {
            Spacer(Modifier.width(4.dp))
            Icon(PleyIcons.ChevronRight, contentDescription = null, size = 20.dp, tint = PleyColors.Muted)
        }
    }
}

/** Room above and below the cards for a pressed card's growth and ring, taken from the gaps around the row. */
internal val PRESS_ROOM = 6.dp

private const val SKELETON_CARDS = 6
private const val SKELETON = "skeleton"
