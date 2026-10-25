@file:OptIn(ExperimentalFoundationApi::class)

package app.pley.android.ui.tv.kit

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileKind
import kotlinx.coroutines.launch

/**
 * A row of tiles (design spec §2.4, D-pad rules 2 to 5): a header, then a
 * lazy row whose focused tile stays at the gutter (x = 96) while the row
 * moves under it.
 *
 * - The row is one focus group that comes back to the tile focused last
 *   (focusRestorer); [memory] keeps that tile across leaving the page, so
 *   Back lands on it.
 * - Left from the first tile enters the rail; Right stops at the end.
 * - Keys are the tiles' ids, and the content type their kind.
 * - While a key repeats, the row jumps a tile at a time without animating.
 *
 * [rowKey] names the row on its page; in a [TvRowColumn] it is also the
 * row's item key. [initialFocus] makes the first tile the page's initial
 * focus. [seeAll] adds the "See all" tile at the end. [onFocus] hears
 * each tile that takes focus (an info block follows it once it rests,
 * [rememberRested]); [onActions] is the long press of Select.
 */
@Composable
fun TvRow(
    rowKey: String,
    title: String?,
    tiles: List<ArtworkTileModel>,
    kind: TileKind,
    onOpen: (ArtworkTileModel) -> Unit,
    modifier: Modifier = Modifier,
    onActions: ((ArtworkTileModel) -> Unit)? = null,
    onFocus: ((ArtworkTileModel) -> Unit)? = null,
    memory: FocusMemory? = null,
    initialFocus: Boolean = false,
    seeAll: SeeAll? = null,
    state: LazyListState = rememberLazyListState(),
) {
    val density = LocalDensity.current
    val gate = LocalKeyRepeatGate.current
    val rail = LocalTvRailFocus.current
    val column = LocalTvRowColumn.current
    val focusManager = LocalFocusManager.current
    val scope = rememberCoroutineScope()
    val row = remember { RowFocus() }
    val spec = remember(density) { TvScrolling.row(density) }
    val group = remember { FocusRequester() }
    if (column != null) {
        DisposableEffect(column, rowKey) {
            column.rows[rowKey] = group
            onDispose { if (column.rows[rowKey] === group) column.rows.remove(rowKey) }
        }
    }
    Column(
        modifier
            .fillMaxWidth()
            .focusRequester(group)
            .onFocusChanged {
                row.hasFocus = it.hasFocus
                if (it.hasFocus) column?.focusedRow = rowKey
            }
            .focusedRowOnTop(row)
            .focusGroup(),
    ) {
        if (title != null) {
            BasicText(
                title,
                style = PleyType.Tv.Section.copy(color = PleyColors.Fg),
                maxLines = 1,
                modifier = Modifier.padding(start = PleyTv.Gutter, end = TvSafeArea.Horizontal),
            )
            Spacer(Modifier.height(PleyTv.HeaderToTiles))
        }
        CompositionLocalProvider(LocalBringIntoViewSpec provides spec) {
            LazyRow(
                state = state,
                modifier = Modifier
                    .fillMaxWidth()
                    .bleed(TileBleed)
                    .testTag("$ROW_TAG$rowKey")
                    .railOnLeftStopOnRight(rail)
                    .focusRestorer()
                    .onPreviewKeyEvent { event ->
                        // Held keys: jump straight to the next tile, then move focus there
                        // (it is already at the gutter, so nothing animates).
                        if (!gate.holding || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                        val step = when (event.key) {
                            Key.DirectionLeft -> -1
                            Key.DirectionRight -> 1
                            else -> return@onPreviewKeyEvent false
                        }
                        val count = tiles.size + if (seeAll != null) 1 else 0
                        val target = row.focusedIndex + step
                        if (row.focusedIndex < 0 || target !in 0 until count) return@onPreviewKeyEvent false
                        scope.launch {
                            state.scrollToItem(target)
                            withFrameNanos { }
                            focusManager.moveFocus(if (step < 0) FocusDirection.Left else FocusDirection.Right)
                        }
                        true
                    },
                contentPadding = PaddingValues(
                    start = PleyTv.Gutter,
                    end = TvSafeArea.Horizontal + kind.tvWidth,
                    top = TileBleed,
                    bottom = TileBleed,
                ),
                horizontalArrangement = Arrangement.spacedBy(kind.tvGap),
            ) {
                itemsIndexed(tiles, key = { _, tile -> tile.id }, contentType = { _, _ -> kind }) { index, tile ->
                    TvTile(
                        model = tile,
                        kind = kind,
                        onClick = { onOpen(tile) },
                        onLongClick = onActions?.let { { it(tile) } },
                        modifier = Modifier.onFocusChanged {
                            if (it.hasFocus) {
                                row.focusedIndex = index
                                onFocus?.invoke(tile)
                            }
                        },
                        focusModifier = if (memory != null) {
                            Modifier.rememberedFocus(memory, "$rowKey/${tile.id}", initial = initialFocus && index == 0)
                        } else {
                            Modifier.initialFocus(enabled = initialFocus && index == 0)
                        },
                    )
                }
                if (seeAll != null) {
                    item(key = SEE_ALL_KEY, contentType = SEE_ALL_KEY) {
                        TvSeeAllTile(
                            kind = kind,
                            label = seeAll.label,
                            onClick = seeAll.onClick,
                            modifier = Modifier.onFocusChanged { if (it.hasFocus) row.focusedIndex = tiles.size },
                            focusModifier = if (memory != null) Modifier.rememberedFocus(memory, "$rowKey/$SEE_ALL_KEY") else Modifier,
                        )
                    }
                }
            }
        }
    }
}

/** A row's "See all" tile: its [label] and what it opens. */
@Stable
class SeeAll(val label: String, val onClick: () -> Unit)

/**
 * A page of rows (design spec §2.4, §4.1): a lazy column that brings the
 * focused row's header to y = 188, hides tiles scrolled under the rail
 * ([railFade]) and, while a key repeats, jumps a row at a time without
 * animating. Give each [TvRow] its rowKey as its item key.
 *
 * A page with a hero (Home, item pages) makes it the first item and says
 * how tall it is ([heroHeight]): focus in the hero scrolls the page back
 * to its top, and Down from the hero's buttons finds the first row, since
 * both are in the one column.
 *
 * Once the page has scrolled, nothing is drawn above the focused row's
 * header ([clearAbove]): the rows pass beneath the page's info block
 * (design spec §4.1), leaving the backdrop clear there.
 */
@Composable
fun TvRowColumn(
    modifier: Modifier = Modifier,
    state: LazyListState = rememberLazyListState(),
    contentPadding: PaddingValues = PaddingValues(bottom = LastRowRoom),
    headerY: Dp = PleyTv.FocusedRowHeaderY,
    heroHeight: Dp = 0.dp,
    clearAbove: Boolean = true,
    content: LazyListScope.() -> Unit,
) {
    val density = LocalDensity.current
    val gate = LocalKeyRepeatGate.current
    val focusManager = LocalFocusManager.current
    val scope = rememberCoroutineScope()
    val holder = remember { TvRowColumnFocus() }
    val tileTop = headerY + PleyTv.RowHeaderHeight + PleyTv.HeaderToTiles
    val spec = remember(density, tileTop, state, heroHeight) { TvScrolling.page(density, tileTop, state, heroHeight) }
    CompositionLocalProvider(LocalBringIntoViewSpec provides spec, LocalTvRowColumn provides holder) {
        LazyColumn(
            state = state,
            contentPadding = contentPadding,
            verticalArrangement = Arrangement.spacedBy(PleyTv.RowGap),
            modifier = modifier
                .railFade()
                .clearAboveHeader(state, headerY, clearAbove)
                // Coming back from the rail: the row focus was in, then (its own restorer) the tile.
                // focusRestorer here would pick by composition key, which lazy rows share.
                .focusProperties {
                    onEnter = {
                        if (requestedFocusDirection == FocusDirection.Enter) {
                            holder.focusedRow?.let(holder.rows::get)?.let { runCatching { it.requestFocus() } }
                        }
                    }
                }
                .onPreviewKeyEvent { event ->
                    if (!gate.holding || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    val step = when (event.key) {
                        Key.DirectionUp -> -1
                        Key.DirectionDown -> 1
                        else -> return@onPreviewKeyEvent false
                    }
                    val focused = holder.focusedRow ?: return@onPreviewKeyEvent false
                    val index = state.layoutInfo.visibleItemsInfo.firstOrNull { it.key == focused }?.index
                        ?: return@onPreviewKeyEvent false
                    val target = index + step
                    if (target !in 0 until state.layoutInfo.totalItemsCount) return@onPreviewKeyEvent false
                    val before = with(density) { contentPadding.calculateTopPadding().roundToPx() }
                    val headerAt = with(density) { headerY.roundToPx() }
                    scope.launch {
                        if (target == 0 && heroHeight > 0.dp) state.scrollToItem(0) else state.scrollToItem(target, before - headerAt)
                        withFrameNanos { }
                        focusManager.moveFocus(if (step < 0) FocusDirection.Up else FocusDirection.Down)
                    }
                    true
                },
            content = content,
        )
    }
}

/**
 * Draws nothing above [headerY] (less a little room) once [state] has
 * scrolled from its top; at the top, everything (a hero) draws.
 */
private fun Modifier.clearAboveHeader(state: LazyListState, headerY: Dp, enabled: Boolean): Modifier =
    if (!enabled) {
        this
    } else {
        drawWithContent {
            val atTop = state.firstVisibleItemIndex == 0 && state.firstVisibleItemScrollOffset == 0
            if (atTop) {
                drawContent()
            } else {
                clipRect(top = (headerY - HeaderClearance).toPx()) { this@drawWithContent.drawContent() }
            }
        }
    }

/** Room kept above the focused row's header when the rows above it are cleared. */
private val HeaderClearance = 12.dp

/** Which row of a [TvRowColumn] has focus, by its key. */
@Stable
internal class TvRowColumnFocus {
    var focusedRow: Any? = null

    /** Each row's focus group, by its key, while it is composed. */
    val rows = HashMap<Any, FocusRequester>()
}

internal val LocalTvRowColumn = staticCompositionLocalOf<TvRowColumnFocus?> { null }

/** One row's focus: whether it has it (for drawing over its neighbours) and which tile. */
@Stable
private class RowFocus {
    var hasFocus by mutableStateOf(false)
    var focusedIndex = -1
}

/** The focused row draws over the rows around it, so its tile's ring and glow cross their edges (decided while placing). */
private fun Modifier.focusedRowOnTop(row: RowFocus): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    layout(placeable.width, placeable.height) { placeable.place(0, 0, zIndex = if (row.hasFocus) 1f else 0f) }
}

/**
 * Hides what scrolls under the collapsed rail (design spec §2.4): [Bg][PleyColors.Bg]
 * up to x = 56, clear from x = 72, drawn once over the content. The focused
 * tile's ring sits right of it.
 */
fun Modifier.railFade(): Modifier = drawWithContent {
    drawContent()
    val solid = PleyTv.RailFadeSolidUntil.toPx()
    val clear = PleyTv.RailFadeClearFrom.toPx()
    drawRect(
        brush = Brush.horizontalGradient(0f to PleyColors.Bg, solid / clear to PleyColors.Bg, 1f to Color.Transparent, startX = 0f, endX = clear),
        topLeft = Offset.Zero,
        size = Size(clear, size.height),
    )
}

/**
 * Lays this out [vertical] taller above and below than the room it takes,
 * and draws it that much higher: a lazy row clips 15 dp past its edges, and
 * a focused tile's growth, ring and glow reach up to 29 dp.
 */
internal fun Modifier.bleed(vertical: Dp): Modifier = layout { measurable, constraints ->
    val extra = vertical.roundToPx()
    val placeable = measurable.measure(
        constraints.copy(maxHeight = if (constraints.hasBoundedHeight) constraints.maxHeight + 2 * extra else constraints.maxHeight),
    )
    layout(placeable.width, (placeable.height - 2 * extra).coerceAtLeast(0)) { placeable.place(0, -extra) }
}

/** Room above and below a row's tiles for the focus look, beyond the 15 dp a lazy row draws past its edges. */
private val TileBleed = 16.dp

/**
 * Room under a page's last row, so it too can come up to the header line
 * (y = 188): the screen, less the line and the shortest row (landscape
 * tiles with their header and caption).
 */
val LastRowRoom: Dp = PleyTv.CanvasHeight - PleyTv.FocusedRowHeaderY -
    (PleyTv.RowHeaderHeight + PleyTv.HeaderToTiles + TileKind.Landscape.tvHeight + PleyTv.TilesToCaption + PleyTv.CaptionHeight)

private const val SEE_ALL_KEY = "pley.seeAll"

/** The test tag of the lazy row of [TvRow] `rowKey`: this, then the key. */
const val ROW_TAG = "pley.tv.row."
