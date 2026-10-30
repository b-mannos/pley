package app.pley.android.ui.tv.livetv

import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.sentenceOf
import app.pley.android.data.Loadable
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.image.Artwork
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.designsystem.tile.TileCaption
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.designsystem.tile.TileStyle
import app.pley.android.designsystem.tile.tvTileSize
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveProgram
import app.pley.android.screens.livetv.ChannelFilter
import app.pley.android.screens.livetv.LivePage
import app.pley.android.screens.livetv.LiveTvEvent
import app.pley.android.screens.livetv.LiveTvViewModel
import app.pley.android.screens.livetv.LiveView
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.LocalTvRailFocus
import app.pley.android.ui.tv.kit.TvPillButton
import app.pley.android.ui.tv.kit.TvSidePanel
import app.pley.android.ui.tv.kit.TvTileFrame
import app.pley.android.ui.tv.kit.initialFocus
import app.pley.android.ui.tv.kit.railFade
import app.pley.android.ui.tv.kit.railOnLeftStopOnRight
import app.pley.android.ui.tv.kit.rememberFocusMemory
import app.pley.android.ui.tv.kit.rememberedFocus
import app.pley.android.ui.tv.library.TvChip
import app.pley.android.ui.tv.library.TvMessage
import app.pley.android.ui.tv.library.TvTab
import app.pley.android.ui.tv.shell.ReportOffline
import app.pley.android.ui.tv.shell.failedBody
import app.pley.android.ui.tv.shell.failedTitle

/**
 * Live TV on TV (the web's LiveTvPage, for a remote): "Live TV" with the
 * Guide and Channels tabs, then chips for All, Favourites and each group.
 * The guide ([TvGuide]) is the page's first focus; the channels are tiles
 * under Recently watched. OK on a channel watches it; holding OK (or
 * Menu) opens a side panel with what's on, Watch and the favourite.
 */
@Composable
internal fun TvLiveTvScreen() {
    val vm: LiveTvViewModel = viewModel(factory = LiveTvViewModel.Factory)
    val page by vm.state.collectAsStateWithLifecycle()
    val now by vm.now.collectAsStateWithLifecycle()
    val resources = LocalResources.current
    var details by remember { mutableStateOf<Pair<Long, LiveProgram?>?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(vm) {
        vm.events.collect { event ->
            when (event) {
                is LiveTvEvent.Failed -> notice = resources.getString(R.string.tv_livetv_favorite_failed, sentenceOf(event.error))
            }
        }
    }
    ReportOffline(
        when {
            page.error != null -> Loadable.Failed(page.error!!, Unit)
            page.channels != null -> Loadable.Ready(Unit, fresh = true)
            else -> null
        },
    )
    val rail = LocalTvRailFocus.current
    Box(Modifier.fillMaxSize().testTag(TV_LIVE_TV)) {
        Column(Modifier.fillMaxSize()) {
            Row(
                Modifier.fillMaxWidth().padding(start = PleyTv.Gutter, end = TvSafeArea.Horizontal, top = TvSafeArea.Vertical),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                BasicText(
                    stringResource(R.string.tv_livetv_title),
                    style = PleyType.Tv.PageTitle.copy(color = PleyColors.Fg),
                    modifier = Modifier.weight(1f).semantics { heading() },
                )
                if (!page.channels.isNullOrEmpty()) {
                    Row(Modifier.railOnLeftStopOnRight(rail).focusGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        TvTab(stringResource(R.string.tv_livetv_guide), page.view == LiveView.Guide, { vm.setView(LiveView.Guide) })
                        TvTab(stringResource(R.string.tv_livetv_channels), page.view == LiveView.Channels, { vm.setView(LiveView.Channels) })
                    }
                }
            }
            val channels = page.channels
            when {
                !page.allowed -> TvMessage(stringResource(R.string.tv_livetv_not_allowed_title), stringResource(R.string.tv_livetv_not_allowed_body), top = 40.dp)
                channels == null && page.error != null -> TvMessage(
                    title = failedTitle(page.error!!, stringResource(R.string.tv_livetv_failed)),
                    body = failedBody(page.error!!),
                    action = stringResource(R.string.tv_livetv_retry),
                    onAction = vm::retry,
                    top = 40.dp,
                )
                channels == null -> Unit
                channels.isEmpty() -> TvMessage(stringResource(R.string.tv_livetv_none_title), stringResource(R.string.tv_livetv_none_body), top = 40.dp)
                else -> {
                    Chips(page, onFilter = vm::setFilter)
                    when {
                        page.shown.isEmpty() -> TvMessage(
                            title = stringResource(R.string.tv_livetv_no_match),
                            body = "",
                            action = stringResource(R.string.tv_livetv_show_all),
                            onAction = vm::showAll,
                            top = 24.dp,
                        )
                        page.view == LiveView.Guide -> androidx.compose.runtime.key(page.filter) {
                            TvGuide(
                                channels = page.shown,
                                vm = vm,
                                origin = page.origin,
                                now = now,
                                onWatch = vm::watch,
                                onDetails = { channel, program -> details = channel.id to program },
                                modifier = Modifier.weight(1f).padding(top = 12.dp),
                            )
                        }
                        else -> ChannelTiles(page, now, onWatch = vm::watch, onDetails = { details = it.id to it.onAt(now) })
                    }
                }
            }
        }
        val shown = details?.let { (id, program) -> page.channels?.firstOrNull { it.id == id }?.let { it to program } }
        TvSidePanel(visible = shown != null, onDismiss = { details = null }) {
            if (shown != null) {
                ProgramPanel(
                    channel = shown.first,
                    program = shown.second,
                    origin = page.origin,
                    now = now,
                    onWatch = {
                        details = null
                        vm.watch(shown.first)
                    },
                    onFavorite = { vm.toggleFavorite(shown.first) },
                )
            }
        }
        notice?.let { text ->
            LaunchedEffect(text) {
                kotlinx.coroutines.delay(NOTICE_MS)
                notice = null
            }
            BasicText(
                text,
                style = PleyType.Tv.Body.copy(color = PleyColors.Fg),
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 32.dp)
                    .clip(PleyShape.Control)
                    .background(PleyColors.Surface3)
                    .padding(horizontal = 20.dp, vertical = 12.dp),
            )
        }
    }
}

/** All, Favourites (with any) and each group (with two or more), from the gutter. */
@Composable
private fun Chips(page: LivePage, onFilter: (ChannelFilter) -> Unit) {
    val rail = LocalTvRailFocus.current
    LazyRow(
        modifier = Modifier.fillMaxWidth().padding(top = 16.dp).railFade().railOnLeftStopOnRight(rail).testTag(TV_LIVE_CHIPS),
        contentPadding = PaddingValues(start = PleyTv.Gutter, end = TvSafeArea.Horizontal),
        horizontalArrangement = Arrangement.spacedBy(PleyTv.ButtonGap),
    ) {
        item(key = "all") {
            TvChip(stringResource(R.string.tv_livetv_count, stringResource(R.string.tv_livetv_all), page.channels?.size ?: 0), { onFilter(ChannelFilter.All) }, selected = page.filter == ChannelFilter.All)
        }
        if (page.favorites > 0) {
            item(key = "favorites") {
                TvChip(
                    stringResource(R.string.tv_livetv_count, stringResource(R.string.tv_livetv_favorites), page.favorites),
                    { onFilter(ChannelFilter.Favorites) },
                    selected = page.filter == ChannelFilter.Favorites,
                    icon = PleyIcons.Star,
                )
            }
        }
        if (page.groups.size > 1) {
            items(page.groups, key = { "group:${it.name}" }) { group ->
                val filter = ChannelFilter.Group(group.name)
                TvChip(stringResource(R.string.tv_livetv_count, group.name, group.count), { onFilter(filter) }, selected = page.filter == filter)
            }
        }
    }
}

/** The channels as tiles, four across, under Recently watched when every channel shows. */
@Composable
private fun ChannelTiles(page: LivePage, now: Long, onWatch: (LiveChannel) -> Unit, onDetails: (LiveChannel) -> Unit) {
    val rail = LocalTvRailFocus.current
    val memory = rememberFocusMemory()
    val recent = if (page.filter == ChannelFilter.All && page.query.isBlank()) page.recent.take(COLUMNS) else emptyList()
    LazyVerticalGrid(
        columns = GridCells.Fixed(COLUMNS),
        modifier = Modifier.fillMaxSize().railFade().railOnLeftStopOnRight(rail).testTag(TV_LIVE_TILES),
        contentPadding = PaddingValues(start = PleyTv.Gutter, end = TvSafeArea.Horizontal, top = 20.dp, bottom = 48.dp),
        horizontalArrangement = Arrangement.spacedBy(PleyTv.WideTileGap),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        if (recent.isNotEmpty()) {
            item(key = "recent-title", span = { GridItemSpan(maxLineSpan) }, contentType = "header") {
                BasicText(stringResource(R.string.tv_livetv_recent), style = PleyType.Tv.Section.copy(color = PleyColors.Fg))
            }
            itemsIndexed(recent, key = { _, c -> "recent:${c.id}" }, contentType = { _, _ -> "channel" }) { index, channel ->
                ChannelTileCard(channel, page.origin, now, Modifier.rememberedFocus(memory, "recent/${channel.id}", initial = index == 0), onWatch, onDetails)
            }
            item(key = "all-title", span = { GridItemSpan(maxLineSpan) }, contentType = "header") {
                BasicText(stringResource(R.string.tv_livetv_all_channels), style = PleyType.Tv.Section.copy(color = PleyColors.Fg), modifier = Modifier.padding(top = 8.dp))
            }
        }
        itemsIndexed(page.shown, key = { _, c -> c.id }, contentType = { _, _ -> "channel" }) { index, channel ->
            ChannelTileCard(
                channel,
                page.origin,
                now,
                Modifier.rememberedFocus(memory, "channel/${channel.id}", initial = recent.isEmpty() && index == 0),
                onWatch,
                onDetails,
            )
        }
    }
}

/** A channel's tile: its logo with its number and how far what's on is, captioned with its name and what's on. */
@Composable
private fun ChannelTileCard(
    channel: LiveChannel,
    origin: String,
    now: Long,
    focusModifier: Modifier,
    onWatch: (LiveChannel) -> Unit,
    onDetails: (LiveChannel) -> Unit,
) {
    val on = channel.onAt(now)
    TvTileFrame(
        kind = TileKind.Episode,
        onClick = { onWatch(channel) },
        onLongClick = { onDetails(channel) },
        focusModifier = focusModifier,
        caption = { focused ->
            TileCaption(
                channel.name,
                on?.title ?: stringResource(if (channel.hasGuide) R.string.tv_livetv_nothing_now else R.string.tv_livetv_no_guide),
                TileStyle.Tv,
                emphasized = focused,
            )
        },
    ) {
        Box(Modifier.tvTileSize(TileKind.Episode)) {
            ChannelTile(channel, origin, Modifier.fillMaxSize(), name = true, style = PleyType.Tv.Title)
            channel.number?.let {
                BasicText(
                    it,
                    style = PleyType.Tv.Badge.copy(color = PleyColors.Fg),
                    modifier = Modifier.padding(6.dp).clip(PleyShape.Badge).background(PleyColors.OnArt).padding(horizontal = 5.dp, vertical = 2.dp),
                )
            }
            if (channel.favorite) {
                Icon(PleyIcons.Star, contentDescription = null, size = 16.dp, tint = PleyColors.Accent, modifier = Modifier.align(Alignment.TopEnd).padding(6.dp))
            }
            if (on != null) ProgramProgress(on, now, Modifier.align(Alignment.BottomCenter).fillMaxWidth(), track = PleyColors.ProgressTrackOnArt)
        }
    }
}

/**
 * A programme's details in the side panel (the web's ProgramModal): the
 * channel, the title, episode, day and times with its badges, how far it
 * is, what the guide says, then Watch and the favourite. Without a
 * programme, the channel alone.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ProgramPanel(
    channel: LiveChannel,
    program: LiveProgram?,
    origin: String,
    now: Long,
    onWatch: () -> Unit,
    onFavorite: () -> Unit,
) {
    val clock = rememberClock()
    val airing = program?.airingAt(now) ?: true
    Column(Modifier.fillMaxSize().testTag(TV_LIVE_PROGRAM)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            ChannelTile(channel, origin, Modifier.size(width = 64.dp, height = 38.dp))
            Spacer(Modifier.width(12.dp))
            BasicText(listOfNotNull(channel.number, channel.name).joinToString("  "), style = PleyType.Tv.Label.copy(color = PleyColors.Muted), maxLines = 1)
        }
        if (program != null) {
            ArtworkImage.of(program.image, origin)?.let { image ->
                Spacer(Modifier.height(14.dp))
                Artwork(image, program.title, Modifier.fillMaxWidth().height(160.dp).clip(PleyShape.TileTv))
            }
            Spacer(Modifier.height(14.dp))
            BasicText(program.title, style = PleyType.Tv.Title.copy(color = PleyColors.Fg), maxLines = 2, overflow = TextOverflow.Ellipsis)
            val episode = listOfNotNull(program.episodeLabel, program.subtitle).joinToString(" · ")
            if (episode.isNotEmpty()) BasicText(episode, style = PleyType.Tv.Body.copy(color = PleyColors.Muted), maxLines = 1, modifier = Modifier.padding(top = 2.dp))
            Spacer(Modifier.height(8.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                BasicText("${dayOf(program.start, now)} · ${spanOf(program, clock)}", style = PleyType.Tv.Caption.copy(color = PleyColors.Muted))
                if (program.airingAt(now)) ProgramBadge(stringResource(R.string.tv_livetv_on_now), live = true)
                if (program.isLive) ProgramBadge(stringResource(R.string.tv_livetv_badge_live), live = true)
                if (program.isNew) ProgramBadge(stringResource(R.string.tv_livetv_badge_new), live = false)
                program.rating?.let {
                    BasicText(
                        it,
                        style = PleyType.Tv.Badge.copy(color = PleyColors.Muted),
                        modifier = Modifier.border(1.dp, PleyColors.Outline, PleyShape.Badge).padding(horizontal = 5.dp, vertical = 2.dp),
                    )
                }
            }
            if (program.airingAt(now)) {
                Row(Modifier.padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    ProgramProgress(program, now, Modifier.weight(1f))
                    Spacer(Modifier.width(10.dp))
                    BasicText(timeLeft(program, now), style = PleyType.Tv.Caption.copy(color = PleyColors.Subtle))
                }
            }
            Spacer(Modifier.height(10.dp))
            BasicText(
                program.description?.takeIf { it.isNotBlank() } ?: stringResource(R.string.tv_livetv_no_description),
                style = PleyType.Tv.Body.copy(color = if (program.description.isNullOrBlank()) PleyColors.Subtle else PleyColors.Fg),
                maxLines = 6,
                overflow = TextOverflow.Ellipsis,
            )
            if (program.stop <= now) {
                BasicText(stringResource(R.string.tv_livetv_ended), style = PleyType.Tv.Caption.copy(color = PleyColors.Subtle), modifier = Modifier.padding(top = 8.dp))
            }
        }
        Spacer(Modifier.weight(1f))
        Row(horizontalArrangement = Arrangement.spacedBy(PleyTv.ButtonGap), modifier = Modifier.focusGroup()) {
            TvPillButton(
                stringResource(if (airing) R.string.tv_livetv_watch_now else R.string.tv_livetv_watch_channel),
                onClick = onWatch,
                icon = PleyIcons.Play,
                modifier = Modifier.initialFocus(),
            )
            TvPillButton(
                stringResource(if (channel.favorite) R.string.tv_livetv_favorite_remove else R.string.tv_livetv_favorite_add),
                onClick = onFavorite,
                icon = if (channel.favorite) PleyIcons.Star else PleyIcons.StarOutline,
            )
        }
    }
}

/** Four 184 dp tiles, 18 apart, from the gutter. */
private const val COLUMNS = 4

private const val NOTICE_MS = 4_000L

internal const val TV_LIVE_TV = "pley.tv.live"
internal const val TV_LIVE_CHIPS = "pley.tv.live.chips"
internal const val TV_LIVE_TILES = "pley.tv.live.tiles"
internal const val TV_LIVE_PROGRAM = "pley.tv.live.program"
