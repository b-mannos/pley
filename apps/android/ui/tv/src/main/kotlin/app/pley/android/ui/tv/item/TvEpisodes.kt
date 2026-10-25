package app.pley.android.ui.tv.item

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyFocus
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.reducedMotion
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.model.ItemSummary
import app.pley.android.screens.item.EpisodeLine
import app.pley.android.screens.item.ItemPage
import app.pley.android.screens.item.SeasonsSection
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.FocusMemory
import app.pley.android.ui.tv.kit.LocalTvRailFocus
import app.pley.android.ui.tv.kit.TvScrolling
import app.pley.android.ui.tv.kit.TvTile
import app.pley.android.ui.tv.kit.railOnLeftStopOnRight
import app.pley.android.ui.tv.kit.rememberRested
import app.pley.android.ui.tv.kit.rememberedFocus

/**
 * The seasons and episodes (design spec §2.4, §4.4; mockup tv-show): the
 * season tabs, when the show has more than one season, then the season's
 * episodes as 208 by 117 tiles captioned "5. Glasshouse" and "26 min left".
 * Focus comes into the row on the episode to watch (or the page's own
 * episode), and stays at the gutter while the row moves. A tab chosen
 * with focus resting on it shows its season.
 */
@Composable
internal fun TvEpisodesSection(
    page: ItemPage,
    seasons: SeasonsSection,
    focus: ItemPageFocus,
    memory: FocusMemory,
    rowRequester: FocusRequester,
    onSelectSeason: (Long) -> Unit,
    onFocusEpisode: (EpisodeLine) -> Unit,
    onOpen: (ItemSummary) -> Unit,
    onActions: (ItemSummary) -> Unit,
) {
    Column(Modifier.fillMaxWidth()) {
        if (seasons.seasons.size > 1) {
            SeasonTabs(seasons, onSelectSeason, Modifier.onFocusChanged { if (it.hasFocus) focus.area = Area.Tabs })
            Spacer(Modifier.height(PleyTv.HeaderToTiles))
        } else {
            BasicText(
                seasons.selectedTab?.title ?: page.context?.seasonTitle ?: stringResource(R.string.tv_item_episodes),
                style = PleyType.Tv.Section.copy(color = PleyColors.Fg),
                modifier = Modifier.padding(start = PleyTv.Gutter),
            )
            Spacer(Modifier.height(PleyTv.HeaderToTiles))
        }
        val episodes = seasons.episodes
        when {
            episodes == null -> Box(Modifier.height(TileKind.Episode.tvHeight + PleyTv.TilesToCaption + PleyTv.CaptionHeight))
            episodes.isEmpty() -> BasicText(
                stringResource(R.string.tv_item_no_episodes),
                style = PleyType.Tv.Body.copy(color = PleyColors.Muted),
                modifier = Modifier.padding(start = PleyTv.Gutter).height(TileKind.Episode.tvHeight),
            )
            else -> key(seasons.selected) {
                EpisodeRow(
                    episodes = episodes,
                    origin = page.origin,
                    target = seasons.focus,
                    memory = memory,
                    rowRequester = rowRequester,
                    modifier = Modifier.onFocusChanged { if (it.hasFocus) focus.area = Area.Episodes },
                    onFocusEpisode = onFocusEpisode,
                    onOpen = onOpen,
                    onActions = onActions,
                )
            }
        }
    }
}

/**
 * The season tabs (design spec §2.6): 40 dp pills, labels in [muted][PleyColors.Muted];
 * the shown season on [surface3][PleyColors.Surface3] in [fg][PleyColors.Fg];
 * the focused tab in [accent][PleyColors.Accent], at 1.05. Focus comes back
 * to the shown season's tab.
 */
@Composable
private fun SeasonTabs(seasons: SeasonsSection, onSelect: (Long) -> Unit, modifier: Modifier = Modifier) {
    var focused by remember { mutableStateOf<Long?>(null) }
    val rested = rememberRested(focused)
    LaunchedEffect(rested) {
        if (rested != null && rested != seasons.selected) onSelect(rested)
    }
    val selected = remember { FocusRequester() }
    Row(
        modifier
            .padding(start = PleyTv.Gutter - 20.dp, end = TvSafeArea.Horizontal)
            .focusRestorer(selected)
            .testTag(TV_SEASON_TABS),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        seasons.seasons.forEach { season ->
            val current = season.id == seasons.selected
            SeasonTab(
                title = season.title,
                current = current,
                onFocus = { focused = season.id },
                onClick = { onSelect(season.id) },
                modifier = if (current) Modifier.focusRequester(selected) else Modifier,
            )
        }
    }
}

@Composable
private fun SeasonTab(title: String, current: Boolean, onFocus: () -> Unit, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    val scale by animateFloatAsState(
        if (focused) PleyFocus.PILL_SCALE else 1f,
        if (reducedMotion()) androidx.compose.animation.core.snap() else PleyMotion.focus(),
        label = "tab",
    )
    val fill = when {
        focused -> PleyColors.Accent
        current -> PleyColors.Surface3
        else -> androidx.compose.ui.graphics.Color.Transparent
    }
    val text = when {
        focused -> PleyColors.OnAccent
        current -> PleyColors.Fg
        else -> PleyColors.Muted
    }
    Box(
        modifier
            .onFocusChanged { if (it.isFocused) onFocus() }
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .heightIn(min = TabHeight)
            .background(fill, PleyShape.Control)
            .semantics { selected = current }
            .clickable(interactionSource = interactions, indication = null, role = Role.Tab, onClick = onClick)
            .padding(horizontal = 20.dp),
        contentAlignment = Alignment.Center,
    ) {
        BasicText(title, style = PleyType.Tv.Label.copy(color = text))
    }
}

/**
 * The episodes, as a row of the TV kit's tiles: the focused tile's left
 * edge at the gutter, focus coming back to the tile it left (or to
 * [target]), Left from the first tile to the rail, Right stopping at the
 * end.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun EpisodeRow(
    episodes: List<EpisodeLine>,
    origin: String,
    target: Long?,
    memory: FocusMemory,
    rowRequester: FocusRequester,
    modifier: Modifier,
    onFocusEpisode: (EpisodeLine) -> Unit,
    onOpen: (ItemSummary) -> Unit,
    onActions: (ItemSummary) -> Unit,
) {
    val tiles = remember(episodes, origin) { episodes.map { ArtworkTileModel.of(it.item, TileKind.Episode, origin) } }
    val start = remember(episodes, target) { episodes.indexOfFirst { it.item.id == target }.coerceAtLeast(0) }
    val state = rememberLazyListState(initialFirstVisibleItemIndex = start)
    val density = LocalDensity.current
    val spec = remember(density) { TvScrolling.row(density) }
    val targetRequester = remember { FocusRequester() }
    val targetId = episodes.getOrNull(start)?.item?.id
    val rail = LocalTvRailFocus.current
    CompositionLocalProvider(LocalBringIntoViewSpec provides spec) {
        LazyRow(
            state = state,
            modifier = modifier
                .fillMaxWidth()
                .bleed(RowBleed)
                .focusRequester(rowRequester)
                .railOnLeftStopOnRight(rail)
                .focusRestorer(targetRequester)
                .testTag(TV_EPISODE_ROW),
            contentPadding = PaddingValues(
                start = PleyTv.Gutter,
                end = TvSafeArea.Horizontal + TileKind.Episode.tvWidth,
                top = RowBleed,
                bottom = RowBleed,
            ),
            horizontalArrangement = Arrangement.spacedBy(TileKind.Episode.tvGap),
        ) {
            itemsIndexed(tiles, key = { _, tile -> tile.id }, contentType = { _, _ -> TileKind.Episode }) { index, tile ->
                val episode = episodes[index]
                TvTile(
                    model = tile,
                    kind = TileKind.Episode,
                    onClick = { onOpen(episode.item) },
                    onLongClick = { onActions(episode.item) },
                    modifier = Modifier.onFocusChanged { if (it.hasFocus) onFocusEpisode(episode) },
                    focusModifier = (if (tile.id == targetId) Modifier.focusRequester(targetRequester) else Modifier)
                        .rememberedFocus(memory, "episode/${tile.id}"),
                )
            }
        }
    }
}

/**
 * Lays this out [vertical] taller above and below than the room it takes:
 * a lazy row clips its content near its edges, and a focused tile's
 * growth, ring and glow reach beyond them.
 */
private fun Modifier.bleed(vertical: Dp): Modifier = layout { measurable, constraints ->
    val extra = vertical.roundToPx()
    val placeable = measurable.measure(
        constraints.copy(maxHeight = if (constraints.hasBoundedHeight) constraints.maxHeight + 2 * extra else constraints.maxHeight),
    )
    layout(placeable.width, (placeable.height - 2 * extra).coerceAtLeast(0)) { placeable.place(0, -extra) }
}

private val RowBleed = 16.dp

/** Tabs are 40 tall (design spec §2.6). */
private val TabHeight = 40.dp

internal const val TV_SEASON_TABS = "pley.tv.item.seasons"
internal const val TV_EPISODE_ROW = "pley.tv.item.episodes"
