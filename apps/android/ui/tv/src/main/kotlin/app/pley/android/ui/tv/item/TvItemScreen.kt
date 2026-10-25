@file:OptIn(ExperimentalFoundationApi::class)

package app.pley.android.ui.tv.item

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.BringIntoViewSpec
import androidx.compose.foundation.gestures.LocalBringIntoViewSpec
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.describe
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.image.ArtworkCandidates
import app.pley.android.designsystem.reducedMotion
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.screens.item.EpisodeLine
import app.pley.android.screens.item.ItemEvent
import app.pley.android.screens.item.ItemPage
import app.pley.android.screens.item.ItemUiState
import app.pley.android.screens.item.ItemViewModel
import app.pley.android.screens.item.extraSubtitle
import app.pley.android.screens.PleyKey
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.BackdropScrim
import app.pley.android.ui.tv.kit.TvBackdrop
import app.pley.android.ui.tv.kit.TvRow
import app.pley.android.ui.tv.kit.railFade
import app.pley.android.ui.tv.kit.rememberFocusMemory
import app.pley.android.ui.tv.kit.rememberRested
import app.pley.android.ui.tv.shell.LocalTvNavigator
import app.pley.android.ui.tv.shell.TvNavigator
import app.pley.android.ui.tv.theme.LocalTvTier

/**
 * An item's page on TV (design spec §4.3, §4.4; mockups tv-movie and
 * tv-show): a movie, show, season or episode over its full-bleed
 * backdrop. Focus starts on Resume or Play. Down goes to the episodes
 * (shows, seasons, episodes) and the rows (Extras, Cast, More Like
 * This), then Details; once focus has left the buttons, an info block
 * stays at the top, for the focused episode in the episode row.
 * Artists, albums and tracks open their music pages instead, an extra
 * its movie's or show's page.
 */
@Composable
internal fun TvItemScreen(itemId: Long) {
    TvItemScreen(viewModel<ItemViewModel>(factory = ItemViewModel.factory(itemId)))
}

/** The page of [model]'s item. */
@Composable
internal fun TvItemScreen(model: ItemViewModel) {
    val state by model.state.collectAsStateWithLifecycle()
    val navigator = LocalTvNavigator.current
    val toasts = remember { TvToastState() }
    val resources = LocalResources.current
    LaunchedEffect(model) {
        model.events.collect { event ->
            when (event) {
                ItemEvent.NothingToPlay -> toasts.show(resources.getString(R.string.tv_item_nothing_to_play))
                is ItemEvent.Failed -> toasts.report(event.error)
            }
        }
    }
    Box(Modifier.fillMaxSize().testTag(TV_ITEM_PAGE)) {
        when (val s = state) {
            ItemUiState.Loading -> TvItemSkeleton()
            ItemUiState.NotFound -> TvMessage(
                title = stringResource(R.string.tv_item_not_found),
                body = stringResource(R.string.tv_item_not_found_body),
                action = stringResource(R.string.tv_item_back),
                onAction = navigator::back,
            )
            is ItemUiState.Failed -> TvMessage(
                title = stringResource(R.string.tv_item_failed),
                body = describe(s.error).replaceFirstChar { it.uppercase() },
                action = stringResource(R.string.tv_item_retry),
                onAction = model::retry,
            )
            is ItemUiState.Elsewhere -> LaunchedEffect(s.key) {
                navigator.back()
                navigator.open(s.key)
            }
            is ItemUiState.Ready -> {
                LaunchedEffect(Unit) {
                    withFrameNanos { }
                    model.onShown()
                }
                (s.status as? app.pley.android.data.Loadable.Failed)?.let { failed ->
                    LaunchedEffect(failed.error) { toasts.report(failed.error) }
                }
                TvItemPage(s.page, model, navigator, toasts)
            }
        }
        TvToastHost(toasts)
    }
}

/** Where focus is on the page, which says where the page scrolls and what the info block shows. */
internal enum class Area { Hero, Tabs, Episodes, Rows }

/** The page's focus, shared by its sections and the scrolling. */
@Stable
internal class ItemPageFocus {
    var area by mutableStateOf(Area.Hero)

    /** The focused hero element's top inside the hero, in px, for scrolling back to it from far down. */
    var heroElementY: Float = 0f
}

/** The side panel open over the page. */
internal sealed interface Panel {
    data object Tracks : Panel

    data object More : Panel

    /** The actions of a tile's [item] (a long press of Select); focus returns to [returnTo] after. */
    data class Actions(val item: ItemSummary, val returnTo: FocusRequester?) : Panel
}

@Composable
private fun TvItemPage(page: ItemPage, model: ItemViewModel, navigator: TvNavigator, toasts: TvToastState) {
    val focus = remember { ItemPageFocus() }
    val memory = rememberFocusMemory()
    val list = rememberLazyListState()
    val tier = LocalTvTier.current
    var panel by remember { mutableStateOf<Panel?>(null) }
    var summaryOpen by remember { mutableStateOf(false) }
    var restore by remember { mutableStateOf<FocusRequester?>(null) }
    var focusedEpisode by remember { mutableStateOf<EpisodeLine?>(null) }
    val buttons = remember { HeroButtons() }
    val episodesRow = remember { FocusRequester() }
    val sections = remember { SectionFocus() }
    val backdrop = remember(page.item, page.origin, tier) { ArtworkCandidates.backdrop(page.item, page.origin, tier) }
    val actions = remember(model, navigator) { TvPageActions(model, navigator) }

    Box(Modifier.fillMaxSize()) {
        TvBackdrop(backdrop, page.context?.showTitle ?: page.title, scrim = BackdropScrim.Item)
        ItemColumn(list, focus) {
            item(key = "hero", contentType = "hero") {
                TvHero(
                    page = page,
                    focus = focus,
                    memory = memory,
                    buttons = buttons,
                    down = if (page.seasons?.episodes?.isNotEmpty() == true) episodesRow else null,
                    actions = actions,
                    onTracks = { panel = Panel.Tracks },
                    onMore = { panel = Panel.More },
                    onSummary = { summaryOpen = true },
                    onPrimaryActions = { panel = Panel.Actions(page.item, buttons.play) },
                )
            }
            page.seasons?.let { seasons ->
                item(key = "episodes", contentType = "episodes") {
                    TvEpisodesSection(
                        page = page,
                        seasons = seasons,
                        focus = focus,
                        memory = memory,
                        rowRequester = episodesRow,
                        onSelectSeason = model::selectSeason,
                        onFocusEpisode = { focusedEpisode = it },
                        onOpen = { actions.open(it.id) },
                        onActions = { panel = Panel.Actions(it, episodesRow) },
                    )
                }
            }
            rowsOf(page, focus, memory, sections, actions) { item, returnTo -> panel = Panel.Actions(item, returnTo) }
            if (!page.details.isEmpty || page.mediaInfo.isNotEmpty()) {
                item(key = "details", contentType = "details") {
                    TvDetails(page, onChooseVersion = model::chooseVersion, modifier = Modifier.onFocusChanged { if (it.hasFocus) focus.area = Area.Rows })
                }
            }
        }
        InfoBlock(page, focus, focusedEpisode)
        TvPanels(
            panel = panel,
            page = page,
            actions = actions,
            onClose = { returnTo ->
                panel = null
                restore = returnTo
            },
            buttons = buttons,
        )
        if (summaryOpen) {
            val text = listOfNotNull(page.tagline, page.summary).joinToString("\n\n")
            TvTextDialog(title = page.title, text = text, onDismiss = {
                summaryOpen = false
                restore = buttons.summary
            })
        }
    }
    // Focus goes back to what opened the panel or dialog once it has left the
    // screen: asked at once, the closing panel still holds it and it falls to the rail.
    LaunchedEffect(restore) {
        val target = restore ?: return@LaunchedEffect
        withFrameNanos { }
        withFrameNanos { }
        runCatching { target.requestFocus() }
        restore = null
    }
}

/** The focus requesters of the hero's buttons, for panels to give focus back to. */
@Stable
internal class HeroButtons {
    val play = FocusRequester()
    val tracks = FocusRequester()
    val more = FocusRequester()
    val summary = FocusRequester()
}

/** Each row's focus group, by key, so focus can go back to it after a panel. */
@Stable
internal class SectionFocus {
    private val requesters = HashMap<String, FocusRequester>()

    fun of(key: String): FocusRequester = requesters.getOrPut(key) { FocusRequester() }
}

/** What the page's parts do: the model's actions and the navigation. */
@Stable
internal class TvPageActions(val model: ItemViewModel, private val navigator: TvNavigator) {
    fun open(id: Long) = navigator.open(PleyKey.Item(id))

    fun openPerson(id: Long) = navigator.open(PleyKey.Person(id))

    fun open(key: PleyKey) = navigator.open(key)
}

/** The rows under the episodes: Extras, Cast or Guest Stars, More Like This. */
private fun LazyListScope.rowsOf(
    page: ItemPage,
    focus: ItemPageFocus,
    memory: app.pley.android.ui.tv.kit.FocusMemory,
    sections: SectionFocus,
    actions: TvPageActions,
    onTileActions: (ItemSummary, FocusRequester) -> Unit,
) {
    val origin = page.origin
    if (page.extras.isNotEmpty()) {
        item(key = "extras", contentType = TileKind.Landscape) {
            val tiles = remember(page.extras, origin) {
                page.extras.map { ArtworkTileModel.of(it, TileKind.Landscape, origin).copy(subtitle = extraSubtitle(it)) }
            }
            val byId = remember(page.extras) { page.extras.associateBy { it.id } }
            val group = sections.of("extras")
            Box(Modifier.focusRequester(group).onFocusChanged { if (it.hasFocus) focus.area = Area.Rows }) {
                TvRow(
                    rowKey = "extras",
                    title = stringResource(R.string.tv_item_extras),
                    tiles = tiles,
                    kind = TileKind.Landscape,
                    onOpen = { tile -> byId[tile.id]?.let { actions.model.play(it) } },
                    memory = memory,
                )
            }
        }
    }
    if (page.cast.isNotEmpty()) {
        item(key = "cast", contentType = TileKind.Person) {
            val tiles = remember(page.cast, origin) {
                page.cast.map { ArtworkTileModel(id = it.id, title = it.name, subtitle = it.character.orEmpty(), image = ArtworkImage.of(it.photoUrl, origin)) }
            }
            Box(Modifier.focusRequester(sections.of("cast")).onFocusChanged { if (it.hasFocus) focus.area = Area.Rows }) {
                TvRow(
                    rowKey = "cast",
                    title = stringResource(if (page.kind == ItemKind.Episode) R.string.tv_item_guest_stars else R.string.tv_item_cast),
                    tiles = tiles,
                    kind = TileKind.Person,
                    onOpen = { actions.openPerson(it.id) },
                    memory = memory,
                )
            }
        }
    }
    if (page.related.isNotEmpty()) {
        item(key = "related", contentType = TileKind.Poster) {
            val tiles = remember(page.related, origin) { page.related.map { ArtworkTileModel.of(it, TileKind.Poster, origin) } }
            val byId = remember(page.related) { page.related.associateBy { it.id } }
            val group = sections.of("related")
            Box(Modifier.focusRequester(group).onFocusChanged { if (it.hasFocus) focus.area = Area.Rows }) {
                TvRow(
                    rowKey = "related",
                    title = stringResource(R.string.tv_item_more_like_this),
                    tiles = tiles,
                    kind = TileKind.Poster,
                    onOpen = { actions.open(it.id) },
                    onActions = { tile -> byId[tile.id]?.let { onTileActions(it, group) } },
                    memory = memory,
                )
            }
        }
    }
}

/**
 * The page's column (design spec §2.4, §4.4): one list from the hero down,
 * which scrolls so that what has focus sits where the spec puts it — the
 * hero at the top; the season tabs at y = 250 and the episodes under them
 * at 306; other rows' tiles at 230, their header at 188 — and, once focus
 * has left the hero, draws nothing above that line, so the rows pass under
 * the info block. Tiles that scroll left pass under the rail ([railFade]).
 */
@Composable
private fun ItemColumn(state: LazyListState, focus: ItemPageFocus, content: LazyListScope.() -> Unit) {
    val density = LocalDensity.current
    val spec = remember(density, state, focus) { ItemPageSpec(density, state, focus) }
    CompositionLocalProvider(LocalBringIntoViewSpec provides spec) {
        LazyColumn(
            state = state,
            contentPadding = PaddingValues(bottom = BottomRoom),
            verticalArrangement = Arrangement.spacedBy(PleyTv.RowGap),
            modifier = Modifier
                .fillMaxSize()
                .railFade()
                .drawWithContent {
                    val line = when (focus.area) {
                        Area.Hero -> null
                        Area.Tabs, Area.Episodes -> TabsTop - Clearance
                        Area.Rows -> RowTilesTop - PleyTv.HeaderToTiles - PleyTv.RowHeaderHeight - Clearance
                    }
                    if (line == null) drawContent() else clipRect(top = line.toPx()) { this@drawWithContent.drawContent() }
                },
            content = content,
        )
    }
}

/** Where the column scrolls what has focus, by the area it is in. */
private class ItemPageSpec(
    density: Density,
    private val state: LazyListState,
    private val focus: ItemPageFocus,
) : BringIntoViewSpec {
    private val tabs = with(density) { TabsTop.toPx() }
    private val episodes = with(density) { EpisodeTilesTop.toPx() }
    private val rows = with(density) { RowTilesTop.toPx() }

    override fun calculateScrollDistance(offset: Float, size: Float, containerSize: Float): Float = when (focus.area) {
        Area.Hero -> if (state.firstVisibleItemIndex == 0) -state.firstVisibleItemScrollOffset.toFloat() else offset - focus.heroElementY
        Area.Tabs -> offset - tabs
        Area.Episodes -> offset - episodes
        Area.Rows -> offset - rows
    }
}

/**
 * The info block (design spec §4.1, §4.4; mockup tv-show) once focus has
 * left the hero: the title and metadata line, then the focused episode's
 * title, line and two lines of summary in the episode row, or the item's
 * own summary by the other rows. It follows the focused episode once
 * focus has rested ([rememberRested]).
 */
@Composable
private fun InfoBlock(page: ItemPage, focus: ItemPageFocus, focusedEpisode: EpisodeLine?) {
    val area = focus.area
    val visible by animateFloatAsState(if (area == Area.Hero) 0f else 1f, if (reducedMotion()) androidx.compose.animation.core.snap() else PleyMotion.reveal(), label = "info")
    if (area == Area.Hero && visible == 0f) return
    val episode = rememberRested(focusedEpisode)
    Column(
        Modifier
            .padding(start = PleyTv.Gutter, top = 30.dp, end = TvSafeArea.Horizontal)
            .width(PleyTv.TextColumnEnd - PleyTv.Gutter + 160.dp)
            .graphicsLayer { alpha = visible },
    ) {
        BasicText(
            heroTitle(page),
            style = PleyType.Tv.HeroTitle.copy(color = PleyColors.Fg),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(12.dp))
        TvMetaLine(page.meta)
        val showEpisode = (area == Area.Episodes || area == Area.Tabs) && episode != null
        if (showEpisode && episode != null) {
            Spacer(Modifier.height(16.dp))
            BasicText(episode.item.title, style = PleyType.Tv.Title.copy(color = PleyColors.Fg), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(4.dp))
            val aired = episode.aired?.let { stringResource(R.string.tv_item_aired, it) }
            BasicText(
                listOfNotNull(episode.longLabel.ifEmpty { null }, episode.runtime.ifEmpty { null }, aired).joinToString(" · "),
                style = PleyType.Tv.Caption.copy(color = PleyColors.Muted),
                maxLines = 1,
            )
            episode.summary?.let {
                Spacer(Modifier.height(6.dp))
                BasicText(it, style = PleyType.Tv.Body.copy(color = PleyColors.Fg.copy(alpha = 0.85f)), maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        } else {
            page.summary?.let {
                Spacer(Modifier.height(10.dp))
                BasicText(it, style = PleyType.Tv.Body.copy(color = PleyColors.Fg.copy(alpha = 0.85f)), maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** The hero's title: an episode's or season's show, else the item's own. */
internal fun heroTitle(page: ItemPage): String = when (page.kind) {
    ItemKind.Episode, ItemKind.Season -> page.context?.showTitle ?: page.title
    else -> page.title
}

/** The loading page (design spec §8): still blocks where the hero's text and buttons will be. No spinner. */
@Composable
private fun TvItemSkeleton() {
    Column(Modifier.fillMaxSize().padding(start = PleyTv.Gutter, top = 70.dp)) {
        Block(360.dp, 56.dp)
        Spacer(Modifier.height(30.dp))
        Block(260.dp, 20.dp)
        Spacer(Modifier.height(28.dp))
        Block(464.dp, 18.dp)
        Spacer(Modifier.height(10.dp))
        Block(440.dp, 18.dp)
        Spacer(Modifier.height(10.dp))
        Block(300.dp, 18.dp)
        Spacer(Modifier.height(44.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(PleyTv.ButtonGap)) {
            Box(Modifier.size(160.dp, 48.dp).background(PleyColors.Surface2, PleyShape.Pill))
            repeat(3) { Box(Modifier.size(48.dp).background(PleyColors.Surface2, PleyShape.Circle)) }
        }
    }
}

@Composable
private fun Block(width: androidx.compose.ui.unit.Dp, height: androidx.compose.ui.unit.Dp) {
    Box(Modifier.size(width, height).background(PleyColors.Surface2, PleyShape.Badge))
}

/** The season tabs' top once focus is in the episodes (mockup tv-show). */
internal val TabsTop = 250.dp

/** The episode tiles' top, under the tabs. */
internal val EpisodeTilesTop = 306.dp

/** Other rows' tiles' top: their header at y = 188. */
internal val RowTilesTop = PleyTv.FocusedRowHeaderY + PleyTv.RowHeaderHeight + PleyTv.HeaderToTiles

/** Room kept above the line under the info block. */
private val Clearance = 12.dp

/** Room under the last section, so it too can come up to its line. */
private val BottomRoom = 300.dp

internal const val TV_ITEM_PAGE = "pley.tv.item"
