package app.pley.android.ui.phone.livetv

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.sentenceOf
import app.pley.android.data.Loadable
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveProgram
import app.pley.android.screens.livetv.ChannelFilter
import app.pley.android.screens.livetv.LivePage
import app.pley.android.screens.livetv.LiveTvEvent
import app.pley.android.screens.livetv.LiveTvViewModel
import app.pley.android.screens.livetv.LiveView
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.kit.EmptyState
import app.pley.android.ui.phone.kit.ErrorState
import app.pley.android.ui.phone.kit.PleyChip
import app.pley.android.ui.phone.kit.PleyIconButton
import app.pley.android.ui.phone.kit.PleyTopAppBar
import app.pley.android.ui.phone.kit.TonalButton
import app.pley.android.ui.phone.shell.LocalPhoneNavigator
import app.pley.android.ui.phone.shell.LocalPhoneShell
import app.pley.android.ui.phone.shell.ReportLoad
import app.pley.android.ui.phone.theme.LocalPhoneMetrics

/**
 * Live TV on phones and tablets (the web's LiveTvPage): "Live TV" with a
 * button that switches between the guide and the channels, a search of
 * names, numbers and what's on, then chips for All, Favourites and each
 * group. The guide is channels down and time across; the channels are
 * cards under Recently watched. A channel plays at a tap; a programme's
 * details open in a sheet, with Watch and the favourite.
 */
@Composable
internal fun PhoneLiveTvScreen() {
    val vm: LiveTvViewModel = viewModel(factory = LiveTvViewModel.Factory)
    val page by vm.state.collectAsStateWithLifecycle()
    val now by vm.now.collectAsStateWithLifecycle()
    val navigator = LocalPhoneNavigator.current
    val shell = LocalPhoneShell.current
    val resources = LocalResources.current
    var details by remember { mutableStateOf<Pair<Long, LiveProgram>?>(null) }
    LaunchedEffect(vm) {
        vm.events.collect { event ->
            when (event) {
                is LiveTvEvent.Failed -> shell.toasts.show(resources.getString(R.string.livetv_favorite_failed, sentenceOf(event.error)))
            }
        }
    }
    ReportLoad(
        when {
            page.error != null -> Loadable.Failed(page.error!!, Unit)
            page.channels != null -> Loadable.Ready(Unit, fresh = true)
            else -> null
        },
    )
    val grid = rememberLazyGridState()
    Column(Modifier.fillMaxSize().testTag(LIVE_TV_PAGE)) {
        PleyTopAppBar(
            title = stringResource(R.string.livetv_title),
            onBack = navigator::back,
            scrolled = { page.view == LiveView.Channels && grid.canScrollBackward },
            actions = {
                if (!page.channels.isNullOrEmpty()) {
                    val guide = page.view == LiveView.Guide
                    PleyIconButton(
                        if (guide) PleyIcons.LayoutGrid else PleyIcons.CalendarDays,
                        stringResource(if (guide) R.string.livetv_show_channels else R.string.livetv_show_guide),
                        onClick = { vm.setView(if (guide) LiveView.Channels else LiveView.Guide) },
                        modifier = Modifier.testTag(LIVE_TV_VIEW),
                    )
                }
            },
        )
        val channels = page.channels
        when {
            !page.allowed -> Message(stringResource(R.string.livetv_not_allowed_title), stringResource(R.string.livetv_not_allowed_body))
            channels == null && page.error != null -> Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                ErrorState(stringResource(R.string.livetv_failed), page.error!!, onRetry = vm::retry)
            }
            channels == null -> Box(Modifier.fillMaxSize())
            channels.isEmpty() -> Message(stringResource(R.string.livetv_none_title), stringResource(R.string.livetv_none_body))
            else -> {
                Filters(page, onQuery = vm::setQuery, onFilter = vm::setFilter)
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    when {
                        page.shown.isEmpty() -> Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                            EmptyState(
                                title = stringResource(R.string.livetv_no_match_title),
                                body = if (page.query.isNotBlank()) {
                                    stringResource(R.string.livetv_no_match_search, page.query.trim())
                                } else {
                                    stringResource(R.string.livetv_no_match_group)
                                },
                                icon = PleyIcons.Search,
                                action = { TonalButton(stringResource(R.string.livetv_show_every_channel), onClick = vm::showAll) },
                            )
                        }
                        page.view == LiveView.Guide -> PhoneGuide(
                            channels = page.shown,
                            vm = vm,
                            origin = page.origin,
                            now = now,
                            onWatch = vm::watch,
                            onDetails = { channel, program -> details = channel.id to program },
                        )
                        else -> ChannelCards(page, now, grid, onWatch = vm::watch, onFavorite = vm::toggleFavorite) { channel, program ->
                            details = channel.id to program
                        }
                    }
                }
            }
        }
    }
    val shown = details?.let { (id, program) -> page.channels?.firstOrNull { it.id == id }?.let { it to program } }
    if (shown != null) {
        ProgramSheet(
            channel = shown.first,
            program = shown.second,
            origin = page.origin,
            now = now,
            onWatch = {
                details = null
                vm.watch(shown.first)
            },
            onFavorite = { vm.toggleFavorite(shown.first) },
            onDismiss = { details = null },
        )
    }
}

@Composable
private fun Message(title: String, body: String) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(top = 32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        EmptyState(title = title, body = body, icon = PleyIcons.TvMinimal)
    }
}

/** The search field, then the chips: All, Favourites (with any), and each group (with two or more). */
@Composable
private fun Filters(page: LivePage, onQuery: (String) -> Unit, onFilter: (ChannelFilter) -> Unit) {
    val metrics = LocalPhoneMetrics.current
    Column(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
        Row(
            Modifier
                .fillMaxWidth()
                .padding(start = metrics.marginStart, end = metrics.marginEnd, bottom = 8.dp)
                .height(44.dp)
                .background(PleyColors.Surface3, PleyShape.Pill)
                .padding(start = 14.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(PleyIcons.Search, contentDescription = null, size = 18.dp, tint = PleyColors.Muted)
            Spacer(Modifier.width(10.dp))
            val label = stringResource(R.string.livetv_search)
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (page.query.isEmpty()) {
                    Text(label, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                BasicTextField(
                    value = page.query,
                    onValueChange = onQuery,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = PleyColors.Fg),
                    cursorBrush = SolidColor(PleyColors.AccentBright),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    modifier = Modifier.fillMaxWidth().semantics { contentDescription = label }.testTag(LIVE_TV_SEARCH),
                )
            }
            if (page.query.isNotEmpty()) {
                PleyIconButton(PleyIcons.X, stringResource(R.string.livetv_search_clear), onClick = { onQuery("") }, tint = PleyColors.Muted)
            } else {
                Spacer(Modifier.width(10.dp))
            }
        }
        LazyRow(
            contentPadding = PaddingValues(start = metrics.marginStart, end = metrics.marginEnd),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier.testTag(LIVE_TV_CHIPS),
        ) {
            item(key = "all") {
                PleyChip("${stringResource(R.string.livetv_all)}  ${page.channels?.size ?: 0}", page.filter == ChannelFilter.All, { onFilter(ChannelFilter.All) })
            }
            if (page.favorites > 0) {
                item(key = "favorites") {
                    PleyChip(
                        "${stringResource(R.string.livetv_favorites)}  ${page.favorites}",
                        page.filter == ChannelFilter.Favorites,
                        { onFilter(ChannelFilter.Favorites) },
                        icon = PleyIcons.Star,
                    )
                }
            }
            if (page.groups.size > 1) {
                items(page.groups, key = { "group:${it.name}" }) { group ->
                    val filter = ChannelFilter.Group(group.name)
                    PleyChip("${group.name}  ${group.count}", page.filter == filter, { onFilter(filter) })
                }
            }
        }
    }
}

/**
 * The channels as cards, as many across as fit (one on a phone held
 * upright), under Recently watched when nothing is searched or filtered.
 */
@Composable
private fun ChannelCards(
    page: LivePage,
    now: Long,
    grid: androidx.compose.foundation.lazy.grid.LazyGridState,
    onWatch: (LiveChannel) -> Unit,
    onFavorite: (LiveChannel) -> Unit,
    onDetails: (LiveChannel, LiveProgram) -> Unit,
) {
    val metrics = LocalPhoneMetrics.current
    val showRecent = page.recent.isNotEmpty() && page.query.isBlank() && page.filter == ChannelFilter.All
    LazyVerticalGrid(
        columns = GridCells.Adaptive(CARD_MIN),
        state = grid,
        modifier = Modifier.fillMaxSize().testTag(LIVE_TV_CHANNELS),
        contentPadding = PaddingValues(start = metrics.marginStart, end = metrics.marginEnd, top = 4.dp, bottom = 32.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (showRecent) {
            item(key = "recent-title", span = { GridItemSpan(maxLineSpan) }, contentType = "header") {
                Text(
                    stringResource(R.string.livetv_recent),
                    style = MaterialTheme.typography.titleMedium,
                    color = PleyColors.Fg,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            item(key = "recent", span = { GridItemSpan(maxLineSpan) }, contentType = "recent") {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    items(page.recent, key = { it.id }) { channel ->
                        RecentCard(channel, page.origin, now, onClick = { onWatch(channel) })
                    }
                }
            }
            item(key = "all-title", span = { GridItemSpan(maxLineSpan) }, contentType = "header") {
                Text(
                    stringResource(R.string.livetv_all),
                    style = MaterialTheme.typography.titleMedium,
                    color = PleyColors.Fg,
                    modifier = Modifier.padding(top = 12.dp),
                )
            }
        }
        items(page.shown, key = { it.id }, contentType = { "channel" }) { channel ->
            ChannelCard(channel, page.origin, now, onWatch = { onWatch(channel) }, onFavorite = { onFavorite(channel) }, onDetails = { onDetails(channel, it) })
        }
    }
}

/** A channel watched lately: its logo over its name and what's on. */
@Composable
private fun RecentCard(channel: LiveChannel, origin: String, now: Long, onClick: () -> Unit) {
    val on = channel.onAt(now)
    Column(
        Modifier
            .width(148.dp)
            .clip(PleyShape.Card)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) {},
    ) {
        Box {
            ChannelTile(channel, origin, Modifier.fillMaxWidth().height(80.dp), name = true, style = MaterialTheme.typography.titleSmall)
            if (on != null) ProgramProgress(on, now, Modifier.fillMaxWidth().align(Alignment.BottomCenter), track = PleyColors.ProgressTrackOnArt)
        }
        Spacer(Modifier.height(6.dp))
        Text(channel.name, style = MaterialTheme.typography.titleSmall, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(
            on?.title ?: stringResource(if (channel.hasGuide) R.string.livetv_nothing_now else R.string.livetv_no_guide),
            style = MaterialTheme.typography.bodySmall,
            color = PleyColors.Muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/**
 * A channel: its logo (number in its corner), name and star, what's on
 * with how far it is and when it ends, and what's next. The card plays the
 * channel; what's on and what's next open their details.
 */
@Composable
private fun ChannelCard(
    channel: LiveChannel,
    origin: String,
    now: Long,
    onWatch: () -> Unit,
    onFavorite: () -> Unit,
    onDetails: (LiveProgram) -> Unit,
) {
    val clock = rememberClock()
    val on = channel.onAt(now)
    val next = channel.nextAt(now)
    val watch = if (on != null) {
        stringResource(R.string.livetv_watch_with_now, listOfNotNull(channel.number, channel.name).joinToString(" "), on.title)
    } else {
        stringResource(R.string.livetv_watch, listOfNotNull(channel.number, channel.name).joinToString(" "))
    }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(PleyShape.Card)
            .background(PleyColors.Surface)
            .clickable(role = Role.Button, onClickLabel = watch, onClick = onWatch)
            .padding(10.dp)
            .testTag(LIVE_TV_CHANNEL + channel.id),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box {
            ChannelTile(channel, origin, Modifier.size(width = 88.dp, height = 56.dp), name = true, style = MaterialTheme.typography.labelMedium)
            channel.number?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.labelSmall,
                    color = PleyColors.Fg,
                    modifier = Modifier.padding(3.dp).clip(PleyShape.Badge).background(PleyColors.OnArt).padding(horizontal = 4.dp, vertical = 1.dp),
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(channel.name, style = MaterialTheme.typography.titleSmall, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (on != null) {
                Row(
                    Modifier.clickable(role = Role.Button) { onDetails(on) }.padding(vertical = 2.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    BadgeOf(on, Modifier.padding(end = 6.dp))
                    Text(on.title, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ProgramProgress(on, now, Modifier.weight(1f))
                    Spacer(Modifier.width(8.dp))
                    Text(clock(on.stop), style = MaterialTheme.typography.bodySmall, color = PleyColors.Subtle)
                }
            } else {
                Text(
                    stringResource(if (channel.hasGuide) R.string.livetv_nothing_now else R.string.livetv_no_guide),
                    style = MaterialTheme.typography.bodyMedium,
                    color = PleyColors.Subtle,
                    maxLines = 1,
                )
            }
            if (next != null) {
                Text(
                    stringResource(R.string.livetv_next, clock(next.start), next.title),
                    style = MaterialTheme.typography.bodySmall,
                    color = PleyColors.Subtle,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 2.dp).clickable(role = Role.Button) { onDetails(next) },
                )
            }
        }
        PleyIconButton(
            if (channel.favorite) PleyIcons.Star else PleyIcons.StarOutline,
            stringResource(if (channel.favorite) R.string.livetv_favorite_remove else R.string.livetv_favorite_add),
            onClick = onFavorite,
            tint = if (channel.favorite) PleyColors.Accent else PleyColors.Muted,
        )
    }
}

/** Cards at least this wide: one column on a phone held upright, two or three on tablets. */
private val CARD_MIN = 320.dp

internal const val LIVE_TV_PAGE = "pley.live"
internal const val LIVE_TV_VIEW = "pley.live.view"
internal const val LIVE_TV_SEARCH = "pley.live.search"
internal const val LIVE_TV_CHIPS = "pley.live.chips"
internal const val LIVE_TV_CHANNELS = "pley.live.channels"
internal const val LIVE_TV_CHANNEL = "pley.live.channel."
