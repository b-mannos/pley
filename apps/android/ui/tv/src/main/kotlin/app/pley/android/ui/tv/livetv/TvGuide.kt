package app.pley.android.ui.tv.livetv

import android.view.KeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.reducedMotion
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveProgram
import app.pley.android.screens.livetv.LiveTv
import app.pley.android.screens.livetv.LiveTvViewModel
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.LocalTvRailFocus
import app.pley.android.ui.tv.kit.initialFocus
import java.util.Calendar
import kotlin.math.roundToInt

/**
 * The programme guide on TV (the web's GuideGrid, for a remote): what has
 * focus is described above the grid, channels down and time across, now
 * marked by a line. Focus is a programme, moved by the guide itself rather
 * than by geometry (rows only compose what is in view):
 *
 * - Left and Right go to the programme before and after, the grid
 *   following by half hours; Left from the guide's first half hour goes to
 *   the rail;
 * - Up and Down keep the time and change channel; Up from the first goes
 *   to the chips above, Channel up and down page through the lineup;
 * - OK on a programme on now (or a channel without guide) watches the
 *   channel, on another opens its details, as does holding OK or Menu;
 * - Back away from now comes back to now first.
 */
@Composable
internal fun TvGuide(
    channels: List<LiveChannel>,
    vm: LiveTvViewModel,
    origin: String,
    now: Long,
    onWatch: (LiveChannel) -> Unit,
    onDetails: (LiveChannel, LiveProgram) -> Unit,
    modifier: Modifier = Modifier,
) {
    val guide by vm.guide.collectAsStateWithLifecycle()
    val start = vm.guideStart
    val end = start + LiveTv.GUIDE_SPAN_S
    val rail = LocalTvRailFocus.current
    val focusManager = LocalFocusManager.current
    val density = LocalDensity.current
    val reduced = reducedMotion()
    val clock = rememberClock()
    var row by rememberSaveable { mutableStateOf(0) }
    var cursor by rememberSaveable { mutableStateOf(now) }
    var left by rememberSaveable { mutableStateOf(leftFor(now, start)) }
    var focused by remember { mutableStateOf(false) }
    var held by remember { mutableStateOf(false) }
    val requester = remember { FocusRequester() }
    row = row.coerceIn(0, (channels.size - 1).coerceAtLeast(0))
    val channel = channels.getOrNull(row)
    val programs = guide.byChannel
    val current = channel?.let { at(programs[it.id], cursor) }

    LaunchedEffect(left) { vm.showing(left, left + VIEW_S) }

    /** Puts the cursor at [time] (a programme's start, or a half hour), the grid following it. */
    fun moveTo(time: Long) {
        cursor = time.coerceIn(start, end - 1)
        left = when {
            cursor < left -> slot(cursor).coerceAtLeast(start)
            cursor >= left + VIEW_S - LiveTv.SLOT_S -> (slot(cursor) - LiveTv.SLOT_S).coerceIn(start, end - VIEW_S)
            else -> left
        }
    }

    fun backToNow() {
        cursor = now
        left = leftFor(now, start)
    }

    BackHandler(enabled = focused && (current?.airingAt(now) != true || left != leftFor(now, start))) { backToNow() }

    val pxPerSecond = with(density) { DP_PER_MINUTE.toPx() } / 60f
    val x by animateFloatAsState(
        targetValue = (left - start) * pxPerSecond,
        animationSpec = tween(if (reduced) 0 else PleyMotion.OVERLAY_MS),
        label = "guide",
    )
    val rows = rememberLazyListState()
    LaunchedEffect(row) {
        val visible = rows.layoutInfo.visibleItemsInfo
        val first = visible.firstOrNull()?.index ?: 0
        val last = visible.lastOrNull()?.index ?: 0
        if (row <= first || row >= last) rows.animateScrollToItem((row - 1).coerceAtLeast(0))
    }

    Column(modifier.testTag(TV_LIVE_GUIDE)) {
        Focused(channel, current, origin, now, clock, Modifier.padding(start = PleyTv.Gutter, end = TvSafeArea.Horizontal).height(INFO_HEIGHT))
        // The times across the top, the day at the left.
        Row(Modifier.fillMaxWidth().height(28.dp).padding(start = PleyTv.Gutter - CHANNEL_INSET)) {
            Box(Modifier.width(CHANNEL_WIDTH).fillMaxHeight().padding(start = CHANNEL_INSET), contentAlignment = Alignment.CenterStart) {
                BasicText(dayOf(maxOf(left, start), now), style = PleyType.Tv.Caption.copy(color = PleyColors.Fg), maxLines = 1)
            }
            Box(Modifier.weight(1f).fillMaxHeight().padding(end = TvSafeArea.Horizontal).clipToBounds()) {
                var t = slot(left - VIEW_S)
                while (t < left + 2 * VIEW_S) {
                    val slotStart = t
                    if (slotStart >= start) {
                        val midnight = Calendar.getInstance().apply { timeInMillis = slotStart * 1000 }.let { it.get(Calendar.HOUR_OF_DAY) == 0 && it.get(Calendar.MINUTE) == 0 }
                        BasicText(
                            if (midnight) dayOf(slotStart, now) else clock(slotStart),
                            style = PleyType.Tv.Caption.copy(color = if (midnight) PleyColors.Fg else PleyColors.Muted),
                            maxLines = 1,
                            modifier = Modifier
                                .align(Alignment.CenterStart)
                                .offset { IntOffset(((slotStart - start) * pxPerSecond - x).roundToInt() + 6.dp.roundToPx(), 0) },
                        )
                    }
                    t += LiveTv.SLOT_S
                }
            }
        }
        Box(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .initialFocus()
                .focusRequester(requester)
                .onFocusChanged { focused = it.isFocused }
                .onPreviewKeyEvent { event ->
                    val code = event.key.nativeKeyCode
                    val down = event.type == KeyEventType.KeyDown
                    val native = event.nativeKeyEvent
                    when (code) {
                        KeyEvent.KEYCODE_DPAD_LEFT -> {
                            if (!down) return@onPreviewKeyEvent true
                            val target = current?.start?.minus(1) ?: (cursor - LiveTv.SLOT_S)
                            if (target < start) {
                                rail?.focusCurrent()
                            } else {
                                val before = channel?.let { at(programs[it.id], target) }
                                moveTo(before?.start?.coerceAtLeast(slot(target)) ?: slot(target))
                            }
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_RIGHT -> {
                            if (down) {
                                val target = current?.stop ?: (slot(cursor) + LiveTv.SLOT_S)
                                if (target < end) moveTo(target)
                            }
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_UP -> {
                            if (!down) return@onPreviewKeyEvent true
                            if (row == 0) focusManager.moveFocus(FocusDirection.Up) else row--
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_DOWN -> {
                            if (down && row < channels.lastIndex) row++
                            true
                        }
                        KeyEvent.KEYCODE_CHANNEL_UP, KeyEvent.KEYCODE_PAGE_UP -> {
                            if (down) row = (row - PAGE).coerceAtLeast(0)
                            true
                        }
                        KeyEvent.KEYCODE_CHANNEL_DOWN, KeyEvent.KEYCODE_PAGE_DOWN -> {
                            if (down) row = (row + PAGE).coerceAtMost(channels.lastIndex)
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                            val c = channel ?: return@onPreviewKeyEvent true
                            if (down) {
                                // Held: the details, once.
                                if (native.repeatCount > 0 && !held && current != null) {
                                    held = true
                                    onDetails(c, current)
                                }
                            } else {
                                if (!held) {
                                    if (current == null || current.airingAt(now)) onWatch(c) else onDetails(c, current)
                                }
                                held = false
                            }
                            true
                        }
                        KeyEvent.KEYCODE_MENU, KeyEvent.KEYCODE_INFO -> {
                            if (!down && channel != null && current != null) onDetails(channel, current)
                            true
                        }
                        else -> false
                    }
                }
                .focusable(),
        ) {
            LazyColumn(
                state = rows,
                userScrollEnabled = false,
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(bottom = TvSafeAreaBottom),
            ) {
                itemsIndexed(channels, key = { _, c -> c.id }, contentType = { _, _ -> "row" }) { index, c ->
                    GuideRow(
                        channel = c,
                        programs = programs[c.id],
                        loaded = guide.loaded,
                        origin = origin,
                        start = start,
                        left = left,
                        now = now,
                        pxPerSecond = pxPerSecond,
                        x = { x },
                        focusedStart = if (index == row && focused) current?.start ?: -1L else null,
                        rowFocused = index == row && focused,
                    )
                }
            }
            // Now, down the grid.
            val nowX = (now - start) * pxPerSecond
            Box(
                Modifier
                    .padding(start = PleyTv.Gutter - CHANNEL_INSET + CHANNEL_WIDTH, end = TvSafeArea.Horizontal)
                    .fillMaxSize()
                    .clipToBounds(),
            ) {
                Box(
                    Modifier
                        .offset { IntOffset((nowX - x).roundToInt() - 1.dp.roundToPx(), 0) }
                        .width(2.dp)
                        .fillMaxHeight()
                        .background(PleyColors.Accent.copy(alpha = 0.85f)),
                )
            }
        }
    }
}

/** What has focus, above the grid: its title, channel and times, and what the guide says of it. */
@Composable
private fun Focused(channel: LiveChannel?, program: LiveProgram?, origin: String, now: Long, clock: (Long) -> String, modifier: Modifier) {
    Row(modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.Top) {
        Column(Modifier.weight(1f)) {
            if (channel == null) return@Column
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (program != null) BadgeOf(program, Modifier.padding(end = 8.dp))
                BasicText(
                    program?.title ?: channel.name,
                    style = PleyType.Tv.Section.copy(color = PleyColors.Fg),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            val meta = buildList {
                // Without a programme the title is the channel's name already.
                if (program != null) add(listOfNotNull(channel.number, channel.name).joinToString(" ")) else channel.number?.let(::add)
                if (program != null) {
                    add("${dayOf(program.start, now)} ${spanOf(program, clock)}")
                    if (program.airingAt(now)) add(timeLeft(program, now))
                    program.episodeLabel?.let(::add)
                } else {
                    add(stringResource(if (channel.hasGuide) R.string.tv_livetv_no_info else R.string.tv_livetv_no_guide))
                }
            }
            BasicText(meta.joinToString("  ·  "), style = PleyType.Tv.Label.copy(color = PleyColors.Muted), maxLines = 1, modifier = Modifier.padding(top = 2.dp))
            val text = program?.let { p -> listOfNotNull(p.subtitle, p.description).joinToString(" — ").ifEmpty { null } }
            if (text != null) {
                BasicText(
                    text,
                    style = PleyType.Tv.Body.copy(color = PleyColors.Muted),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 6.dp).width(560.dp),
                )
            }
        }
        if (channel != null) {
            ChannelTile(channel, origin, Modifier.padding(start = 24.dp).size(width = 120.dp, height = 68.dp), name = true)
        }
    }
}

/** One channel's row: its cell and its programmes, the one with focus filled with the accent. */
@Composable
private fun GuideRow(
    channel: LiveChannel,
    programs: List<LiveProgram>?,
    loaded: Boolean,
    origin: String,
    start: Long,
    left: Long,
    now: Long,
    pxPerSecond: Float,
    x: () -> Float,
    focusedStart: Long?,
    rowFocused: Boolean,
) {
    val density = LocalDensity.current
    val clock = rememberClock()
    // What may come into view as the grid moves a screen either way.
    val visible = remember(programs, left, start) { LiveTv.between(programs, maxOf(left - VIEW_S, start), left + 2 * VIEW_S) }
    Row(
        Modifier
            .fillMaxWidth()
            .height(ROW_HEIGHT)
            .padding(start = PleyTv.Gutter - CHANNEL_INSET)
            .drawBehind { drawLine(PleyColors.Surface4.copy(alpha = 0.6f), Offset(0f, size.height - 0.5f), Offset(size.width, size.height - 0.5f), 1f) },
    ) {
        Row(
            Modifier
                .width(CHANNEL_WIDTH)
                .fillMaxHeight()
                .padding(vertical = 4.dp)
                .clip(PleyShape.Control)
                .background(if (rowFocused) PleyColors.Surface3 else androidx.compose.ui.graphics.Color.Transparent)
                .padding(start = CHANNEL_INSET, end = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ChannelTile(channel, origin, Modifier.size(width = 56.dp, height = 34.dp))
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    BasicText(channel.number.orEmpty(), style = PleyType.Tv.Caption.copy(color = PleyColors.Subtle), maxLines = 1)
                    if (channel.favorite) Icon(PleyIcons.Star, contentDescription = null, size = 12.dp, tint = PleyColors.Accent, modifier = Modifier.padding(start = 4.dp))
                }
                BasicText(
                    channel.name,
                    style = PleyType.Tv.Label.copy(color = if (rowFocused) PleyColors.Fg else PleyColors.Muted),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Box(Modifier.weight(1f).fillMaxHeight().padding(end = TvSafeArea.Horizontal).clipToBounds()) {
            if (visible.isEmpty() && loaded) {
                val label = stringResource(if (channel.hasGuide) R.string.tv_livetv_no_info else R.string.tv_livetv_no_guide)
                Box(
                    Modifier
                        .fillMaxSize()
                        .padding(vertical = 4.dp, horizontal = 2.dp)
                        .clip(PleyShape.Control)
                        .background(if (focusedStart == -1L) PleyColors.Accent else PleyColors.Surface.copy(alpha = 0.6f))
                        .padding(start = 12.dp),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    BasicText(label, style = PleyType.Tv.Caption.copy(color = if (focusedStart == -1L) PleyColors.OnAccent else PleyColors.Subtle))
                }
            }
            for (program in visible) {
                val from = maxOf(program.start, start)
                val leftPx = (from - start) * pxPerSecond
                val widthPx = (program.stop - from) * pxPerSecond
                val isFocused = focusedStart == program.start
                val airing = program.airingAt(now)
                val span = spanOf(program, clock)
                Box(
                    Modifier
                        .offset { IntOffset((leftPx - x()).roundToInt(), 0) }
                        .width(with(density) { widthPx.toDp() })
                        .fillMaxHeight()
                        .padding(vertical = 4.dp, horizontal = 2.dp)
                        .alpha(if (program.stop <= now && !isFocused) 0.55f else 1f)
                        .clip(PleyShape.Control)
                        .background(
                            when {
                                isFocused -> PleyColors.Accent
                                airing -> PleyColors.PrimaryContainer
                                else -> PleyColors.Surface2
                            },
                        )
                        .then(if (airing && !isFocused) Modifier.border(1.dp, PleyColors.Accent.copy(alpha = 0.35f), PleyShape.Control) else Modifier)
                        .semantics { contentDescription = "${program.title}, $span" },
                ) {
                    if (widthPx > MIN_TEXT_PX) {
                        Column(
                            Modifier
                                .fillMaxHeight()
                                .offset {
                                    // A programme begun before the left edge keeps its title in view.
                                    val room = (widthPx - 90.dp.toPx()).coerceAtLeast(0f)
                                    IntOffset((x() - leftPx).coerceIn(0f, room).roundToInt(), 0)
                                }
                                .padding(horizontal = 10.dp),
                            verticalArrangement = Arrangement.Center,
                        ) {
                            BasicText(
                                program.title,
                                style = PleyType.Tv.Label.copy(
                                    color = if (isFocused) PleyColors.OnAccent else PleyColors.Fg,
                                    fontWeight = if (airing || isFocused) FontWeight.Bold else FontWeight.Medium,
                                ),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            BasicText(
                                span,
                                style = PleyType.Tv.Caption.copy(color = if (isFocused) PleyColors.OnAccent.copy(alpha = 0.8f) else PleyColors.Subtle),
                                maxLines = 1,
                            )
                        }
                    }
                    if (airing) {
                        Box(
                            Modifier
                                .align(Alignment.BottomStart)
                                .fillMaxWidth(program.progressAt(now))
                                .height(2.dp)
                                .background(if (isFocused) PleyColors.OnAccent else PleyColors.Accent),
                        )
                    }
                }
            }
        }
    }
}

/** The programme on at [time] of [programs]. */
private fun at(programs: List<LiveProgram>?, time: Long): LiveProgram? = programs?.firstOrNull { it.start <= time && it.stop > time }

/** The half hour [time] is in. */
private fun slot(time: Long): Long = Math.floorDiv(time, LiveTv.SLOT_S) * LiveTv.SLOT_S

/** Where the grid starts for [now]: the half hour before the one it is in, never before the guide. */
private fun leftFor(now: Long, start: Long): Long = (slot(now) - LiveTv.SLOT_S).coerceAtLeast(start)

/** The grid shows this much time: 2 h 40 at 4 dp a minute in its 640 dp. */
private const val VIEW_S = 160 * 60L
private val DP_PER_MINUTE = 4.dp
private val ROW_HEIGHT = 52.dp
private val CHANNEL_WIDTH = 176.dp

/** The channel cell's filling starts this far left of the page's gutter, so its text lines up with it. */
private val CHANNEL_INSET = 10.dp
private val INFO_HEIGHT = 92.dp
private val TvSafeAreaBottom = 27.dp

/** Channel up and down move this many rows. */
private const val PAGE = 5

/** Narrower than this, a programme shows no text. */
private const val MIN_TEXT_PX = 70f

internal const val TV_LIVE_GUIDE = "pley.tv.live.guide"
