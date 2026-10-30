package app.pley.android.ui.tv.livetv

import android.view.KeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusGroup
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Glow
import androidx.tv.material3.Surface
import app.pley.android.common.TestTags
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.LiveChannel
import app.pley.android.player.engine.MediaKeys
import app.pley.android.player.engine.VideoSurface
import app.pley.android.player.live.LivePhase
import app.pley.android.player.live.LiveState
import app.pley.android.player.live.LiveTrouble
import app.pley.android.screens.livetv.ChannelDigits
import app.pley.android.screens.livetv.LivePlayerViewModel
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.TvPillButton
import app.pley.android.ui.tv.kit.TvRoundButton
import app.pley.android.ui.tv.kit.TvSidePanel
import app.pley.android.ui.tv.kit.initialFocus
import kotlinx.coroutines.delay

/** What shows over the picture. */
private enum class Overlay { None, Controls, Channels }

/**
 * The TV's live player (the web's LivePlayerPage, for a remote), the
 * channel full screen. With nothing over it:
 *
 * - Up and Down, and Channel up and down, change channel (Up the next
 *   number), digits tune a number, play and pause work;
 * - Left and Right go 10 s back and on in what the server keeps;
 * - OK shows the controls (play and pause, 10 s back and on, Go live,
 *   the channel list, the favourite) over what's on; Menu or Guide shows
 *   the channel list; Back hides what shows, then leaves.
 *
 * What's on and next shows for 5 s on tuning and after a seek, the
 * window and Live (or how far behind) with it. A channel that doesn't
 * start says why (the server's sentence), with Try again and Other
 * channels.
 */
@Composable
internal fun TvLivePlayerScreen(vm: LivePlayerViewModel, onExit: () -> Unit) {
    val state by vm.state.collectAsStateWithLifecycle()
    val engine by vm.engine.collectAsStateWithLifecycle()
    val channel by vm.channel.collectAsStateWithLifecycle()
    val channels by vm.channels.collectAsStateWithLifecycle()
    val origin by vm.origin.collectAsStateWithLifecycle()
    val now by vm.now.collectAsStateWithLifecycle()
    val digits by vm.digits.collectAsStateWithLifecycle()
    var overlay by remember { mutableStateOf(Overlay.None) }
    var peek by remember { mutableIntStateOf(1) }
    var poked by remember { mutableIntStateOf(0) }
    var showInfo by remember { mutableStateOf(true) }
    val root = remember { FocusRequester() }
    val playing = state.phase == LivePhase.Playing
    val failed = state.phase is LivePhase.Failed

    // What's on shows for a while on tuning and after a seek.
    LaunchedEffect(state.channelId) { peek++ }
    LaunchedEffect(peek, playing) {
        showInfo = true
        if (playing) {
            delay(INFO_MS)
            showInfo = false
        }
    }
    // The controls hide when left alone while it plays.
    LaunchedEffect(overlay, poked, state.playWhenReady) {
        if (overlay == Overlay.Controls && state.playWhenReady && playing) {
            delay(CONTROLS_MS)
            overlay = Overlay.None
        }
    }
    // Focus goes back to the picture when nothing shows over it.
    LaunchedEffect(overlay, failed) {
        if (overlay == Overlay.None && !failed) runCatching { root.requestFocus() }
    }
    BackHandler(enabled = overlay != Overlay.None) { overlay = Overlay.None }

    Box(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .testTag(TV_LIVE_PLAYER)
            .onPreviewKeyEvent { event ->
                val code = event.key.nativeKeyCode
                val down = event.type == KeyEventType.KeyDown
                if (overlay == Overlay.Controls) poked++
                when {
                    code == KeyEvent.KEYCODE_CHANNEL_UP -> {
                        if (down) vm.step(1)
                        true
                    }
                    code == KeyEvent.KEYCODE_CHANNEL_DOWN -> {
                        if (down) vm.step(-1)
                        true
                    }
                    code in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 -> {
                        if (down) vm.digit('0' + (code - KeyEvent.KEYCODE_0))
                        true
                    }
                    code == KeyEvent.KEYCODE_GUIDE || code == KeyEvent.KEYCODE_MENU -> {
                        if (!down) overlay = if (overlay == Overlay.Channels) Overlay.None else Overlay.Channels
                        true
                    }
                    code == KeyEvent.KEYCODE_INFO -> {
                        if (!down) peek++
                        true
                    }
                    engine != null && MediaKeys.handle(engine!!.player, event.nativeKeyEvent) -> true
                    overlay != Overlay.None || failed -> false
                    else -> when (code) {
                        KeyEvent.KEYCODE_DPAD_UP -> {
                            if (down) vm.step(1)
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_DOWN -> {
                            if (down) vm.step(-1)
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT -> {
                            if (down && state.canRewind) {
                                vm.seekBy(if (code == KeyEvent.KEYCODE_DPAD_LEFT) -SEEK_MS else SEEK_MS)
                                peek++
                            }
                            true
                        }
                        KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                            if (!down) overlay = Overlay.Controls
                            true
                        }
                        else -> false
                    }
                }
            }
            .focusRequester(root)
            .focusable(),
    ) {
        VideoSurface(engine?.player, Modifier.fillMaxSize().testTag(TestTags.PLAYER_SURFACE))
        val shown = channel
        if (state.audioOnly && playing && shown != null) SoundOnly(shown, origin, now)
        when (val phase = state.phase) {
            LivePhase.Tuning, LivePhase.Idle -> if (shown != null) Tuning(shown, origin, now)
            is LivePhase.Failed -> Trouble(
                trouble = phase.trouble,
                channel = shown,
                origin = origin,
                onRetry = if (phase.trouble.canRetry) vm::retry else null,
                onChannels = { overlay = Overlay.Channels },
                onBack = onExit,
            )
            LivePhase.Playing -> Unit
        }
        if (state.buffering && playing) {
            Icon(PleyIcons.LoaderCircle, contentDescription = null, size = 48.dp, tint = PleyColors.Fg, modifier = Modifier.align(Alignment.Center))
        }
        // What's on, with the window and the controls when asked for.
        AnimatedVisibility(
            visible = shown != null && playing && overlay != Overlay.Channels && (overlay == Overlay.Controls || showInfo),
            enter = fadeIn() + slideInVertically { it / 3 },
            exit = fadeOut() + slideOutVertically { it / 3 },
            modifier = Modifier.align(Alignment.BottomCenter),
        ) {
            if (shown != null) {
                InfoBar(
                    channel = shown,
                    state = state,
                    origin = origin,
                    now = now,
                    controls = overlay == Overlay.Controls,
                    vm = vm,
                    onChannels = { overlay = Overlay.Channels },
                )
            }
        }
        Digits(digits, Modifier.align(Alignment.TopEnd))
        TvSidePanel(visible = overlay == Overlay.Channels, onDismiss = { overlay = Overlay.None }) {
            ChannelList(
                channels = channels,
                current = state.channelId,
                origin = origin,
                now = now,
                onTune = {
                    overlay = Overlay.None
                    vm.tune(it.id)
                },
            )
        }
    }
}

/** The channel's logo, name and what's on while it is tuned. */
@Composable
private fun Tuning(channel: LiveChannel, origin: String, now: Long) {
    Column(Modifier.fillMaxSize().background(Color.Black), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        ChannelTile(channel, origin, Modifier.size(width = 200.dp, height = 112.dp), name = true, style = PleyType.Tv.Title, well = Color.White.copy(alpha = 0.06f), shape = PleyShape.Dialog)
        Spacer(Modifier.height(20.dp))
        BasicText(listOfNotNull(channel.number, channel.name).joinToString("  "), style = PleyType.Tv.Section.copy(color = PleyColors.Fg))
        BasicText(
            channel.onAt(now)?.title ?: stringResource(R.string.tv_live_player_tuning),
            style = PleyType.Tv.Body.copy(color = PleyColors.Muted),
            modifier = Modifier.padding(top = 6.dp),
        )
    }
}

/** A channel with sound only (radio): its logo where the picture would be, with what's on. */
@Composable
private fun SoundOnly(channel: LiveChannel, origin: String, now: Long) {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        ChannelTile(channel, origin, Modifier.size(width = 240.dp, height = 135.dp), name = true, style = PleyType.Tv.Title, well = Color.White.copy(alpha = 0.06f), shape = PleyShape.Dialog)
        Spacer(Modifier.height(18.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(PleyIcons.Radio, contentDescription = null, size = 20.dp, tint = PleyColors.Muted)
            Spacer(Modifier.width(8.dp))
            BasicText(stringResource(R.string.tv_live_player_radio), style = PleyType.Tv.Label.copy(color = PleyColors.Muted))
        }
        channel.onAt(now)?.let { BasicText(it.title, style = PleyType.Tv.Section.copy(color = PleyColors.Fg), modifier = Modifier.padding(top = 8.dp)) }
    }
}

/**
 * The bar at the foot: the channel's logo, number and name, what's on with
 * its times, how far it is and what's next; the window the server keeps
 * with Live; and with [controls], the buttons, play and pause first.
 */
@Composable
private fun InfoBar(
    channel: LiveChannel,
    state: LiveState,
    origin: String,
    now: Long,
    controls: Boolean,
    vm: LivePlayerViewModel,
    onChannels: () -> Unit,
) {
    val clock = rememberClock()
    val on = channel.onAt(now)
    val next = channel.nextAt(now)
    Column(
        Modifier
            .fillMaxWidth()
            .background(Brush.verticalGradient(listOf(Color.Transparent, Color.Black.copy(alpha = 0.75f), Color.Black.copy(alpha = 0.92f))))
            .padding(start = TvSafeArea.Horizontal, end = TvSafeArea.Horizontal, top = 64.dp, bottom = TvSafeArea.Vertical)
            .testTag(TV_LIVE_PLAYER_INFO),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            ChannelTile(channel, origin, Modifier.size(width = 112.dp, height = 63.dp), name = true, well = Color.White.copy(alpha = 0.08f))
            Spacer(Modifier.width(18.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    channel.number?.let { BasicText(it, style = PleyType.Tv.Label.copy(color = PleyColors.Muted), modifier = Modifier.padding(end = 8.dp)) }
                    BasicText(channel.name, style = PleyType.Tv.Label.copy(color = PleyColors.Muted), maxLines = 1)
                    if (channel.favorite) Icon(PleyIcons.Star, contentDescription = null, size = 14.dp, tint = PleyColors.Accent, modifier = Modifier.padding(start = 6.dp))
                }
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
                    if (on != null) BadgeOf(on, Modifier.padding(end = 8.dp))
                    BasicText(
                        on?.title ?: stringResource(if (channel.hasGuide) R.string.tv_livetv_nothing_now else R.string.tv_livetv_no_guide),
                        style = PleyType.Tv.Section.copy(color = PleyColors.Fg),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (on != null) {
                    Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        BasicText("${spanOf(on, clock)} · ${timeLeft(on, now)}", style = PleyType.Tv.Caption.copy(color = PleyColors.Muted))
                        Spacer(Modifier.width(12.dp))
                        ProgramProgress(on, now, Modifier.width(180.dp), track = Color.White.copy(alpha = 0.18f))
                    }
                }
                if (next != null) {
                    BasicText(
                        stringResource(R.string.tv_livetv_next, clock(next.start), next.title),
                        style = PleyType.Tv.Caption.copy(color = PleyColors.Subtle),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
            if (state.phase == LivePhase.Playing) LiveBadge(state)
        }
        if (state.canRewind && state.phase == LivePhase.Playing) {
            Row(Modifier.padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                BasicText(
                    stringResource(R.string.tv_live_player_window, (state.windowMs / 60_000).toInt().coerceAtLeast(1)),
                    style = PleyType.Tv.Caption.copy(color = PleyColors.Muted),
                )
                Spacer(Modifier.width(12.dp))
                Box(Modifier.weight(1f).height(4.dp).clip(PleyShape.Badge).background(Color.White.copy(alpha = 0.2f))) {
                    val fraction = (state.positionMs.toFloat() / state.windowMs.coerceAtLeast(1)).coerceIn(0f, 1f)
                    Box(Modifier.fillMaxHeight().fillMaxWidth(fraction).background(PleyColors.Accent))
                }
            }
        }
        if (controls) {
            Row(
                Modifier.padding(top = 16.dp).focusGroup(),
                horizontalArrangement = Arrangement.spacedBy(PleyTv.ButtonGap),
            ) {
                TvRoundButton(
                    stringResource(if (state.playWhenReady) R.string.tv_live_player_pause else R.string.tv_live_player_play),
                    if (state.playWhenReady) PleyIcons.Pause else PleyIcons.Play,
                    onClick = vm::togglePlay,
                    modifier = Modifier.initialFocus().testTag(TV_LIVE_PLAYER_PLAY),
                )
                if (state.canRewind) {
                    TvRoundButton(stringResource(R.string.tv_live_player_back_10), PleyIcons.RotateCcw, onClick = { vm.seekBy(-SEEK_MS) })
                    TvRoundButton(stringResource(R.string.tv_live_player_forward_10), PleyIcons.RotateCw, onClick = { vm.seekBy(SEEK_MS) }, enabled = !state.atLive)
                    TvRoundButton(stringResource(R.string.tv_live_player_go_live), PleyIcons.ChevronsRight, onClick = vm::goLive, enabled = !state.atLive)
                }
                TvRoundButton(stringResource(R.string.tv_live_player_channels), PleyIcons.List, onClick = onChannels)
                TvRoundButton(
                    stringResource(if (channel.favorite) R.string.tv_livetv_favorite_remove else R.string.tv_livetv_favorite_add),
                    if (channel.favorite) PleyIcons.Star else PleyIcons.StarOutline,
                    onClick = vm::toggleFavorite,
                )
            }
        } else {
            BasicText(stringResource(R.string.tv_live_player_hint), style = PleyType.Tv.Caption.copy(color = PleyColors.Subtle), modifier = Modifier.padding(top = 12.dp))
        }
    }
}

/** Live (red, with a dot) at the live edge; behind it, how far ("−1:23"). */
@Composable
private fun LiveBadge(state: LiveState) {
    val atLive = state.atLive
    Row(
        Modifier
            .clip(PleyShape.Circle)
            .background(if (atLive) PleyColors.Danger else Color.White.copy(alpha = 0.18f))
            .padding(horizontal = 12.dp, vertical = 6.dp)
            .testTag(TV_LIVE_PLAYER_LIVE),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(7.dp).background(if (atLive) Color.White else Color.White.copy(alpha = 0.6f), PleyShape.Circle))
        Spacer(Modifier.width(7.dp))
        BasicText(
            (if (atLive) stringResource(R.string.tv_live_player_live) else stringResource(R.string.tv_live_player_behind, behind(state.behindMs))).uppercase(),
            style = PleyType.Tv.Badge.copy(color = Color.White),
        )
    }
}

/** "1:23" behind live, or "12 min" from 10 minutes. */
private fun behind(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return if (s >= 600) "${s / 60} min" else "${s / 60}:${(s % 60).toString().padStart(2, '0')}"
}

/** A channel number being typed on the remote, or the one that matched nothing. */
@Composable
private fun Digits(digits: ChannelDigits, modifier: Modifier = Modifier) {
    val text = digits.typed.ifEmpty { null } ?: digits.notFound?.let { stringResource(R.string.tv_live_player_no_channel, it) } ?: return
    BasicText(
        text,
        style = (if (digits.typed.isNotEmpty()) PleyType.Tv.Code else PleyType.Tv.Section).copy(color = PleyColors.Fg),
        modifier = modifier
            .padding(top = TvSafeArea.Vertical, end = TvSafeArea.Horizontal)
            .clip(PleyShape.Dialog)
            .background(Color.Black.copy(alpha = 0.85f))
            .padding(horizontal = 24.dp, vertical = 12.dp)
            .testTag(TV_LIVE_PLAYER_DIGITS),
    )
}

/** Why the channel doesn't play, with Try again, Other channels and Back to the guide. */
@Composable
private fun Trouble(
    trouble: LiveTrouble,
    channel: LiveChannel?,
    origin: String,
    onRetry: (() -> Unit)?,
    onChannels: () -> Unit,
    onBack: () -> Unit,
) {
    val name = channel?.name.orEmpty()
    val title = when (trouble.kind) {
        LiveTrouble.Kind.Busy -> stringResource(R.string.tv_live_player_busy)
        LiveTrouble.Kind.Stopped -> stringResource(R.string.tv_live_player_stopped)
        LiveTrouble.Kind.Missing -> stringResource(R.string.tv_live_player_missing)
        LiveTrouble.Kind.NotAllowed -> stringResource(R.string.tv_live_player_not_allowed)
        LiveTrouble.Kind.Unavailable -> stringResource(R.string.tv_live_player_unavailable, name)
        LiveTrouble.Kind.Offline -> stringResource(R.string.tv_live_player_offline)
        LiveTrouble.Kind.Other -> stringResource(R.string.tv_live_player_failed, name)
    }
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.88f)), contentAlignment = Alignment.Center) {
        Column(
            Modifier
                .widthIn(max = 520.dp)
                .clip(PleyShape.Dialog)
                .background(PleyColors.Surface2)
                .border(1.dp, PleyColors.Surface4, PleyShape.Dialog)
                .padding(32.dp)
                .testTag(TV_LIVE_PLAYER_TROUBLE),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            if (channel != null) {
                ChannelTile(channel, origin, Modifier.size(width = 120.dp, height = 68.dp), name = true, well = Color.White.copy(alpha = 0.06f))
            } else {
                Icon(PleyIcons.TriangleAlert, contentDescription = null, size = 40.dp, tint = PleyColors.Accent)
            }
            Spacer(Modifier.height(18.dp))
            BasicText(title, style = PleyType.Tv.Section.copy(color = PleyColors.Fg, textAlign = TextAlign.Center))
            if (trouble.message.isNotBlank()) {
                BasicText(
                    trouble.message,
                    style = PleyType.Tv.Body.copy(color = PleyColors.Muted, textAlign = TextAlign.Center),
                    modifier = Modifier.padding(top = 10.dp),
                )
            }
            Spacer(Modifier.height(24.dp))
            Row(Modifier.focusGroup(), horizontalArrangement = Arrangement.spacedBy(PleyTv.ButtonGap)) {
                if (onRetry != null) TvPillButton(stringResource(R.string.tv_live_player_retry), onClick = onRetry, icon = PleyIcons.RefreshCw, modifier = Modifier.initialFocus())
                TvPillButton(
                    stringResource(R.string.tv_live_player_other_channels),
                    onClick = onChannels,
                    icon = PleyIcons.List,
                    modifier = if (onRetry == null) Modifier.initialFocus() else Modifier,
                )
                TvPillButton(stringResource(R.string.tv_live_player_back_to_guide), onClick = onBack)
            }
        }
    }
}

/** The lineup in the side panel: what's on each, the channel watched first in focus. */
@Composable
private fun ChannelList(channels: List<LiveChannel>, current: Long?, origin: String, now: Long, onTune: (LiveChannel) -> Unit) {
    val list = rememberLazyListState()
    val index = channels.indexOfFirst { it.id == current }.coerceAtLeast(0)
    LaunchedEffect(Unit) { list.scrollToItem((index - 2).coerceAtLeast(0)) }
    BasicText(stringResource(R.string.tv_live_player_channels), style = PleyType.Tv.Title.copy(color = PleyColors.Fg), modifier = Modifier.padding(start = 8.dp, bottom = 12.dp))
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.testTag(TV_LIVE_PLAYER_CHANNELS)) {
        itemsIndexed(channels, key = { _, c -> c.id }) { i, channel ->
            val on = channel.onAt(now)
            val isCurrent = channel.id == current
            Surface(
                onClick = { onTune(channel) },
                modifier = Modifier.fillMaxWidth().then(if (i == index) Modifier.initialFocus() else Modifier),
                shape = ClickableSurfaceDefaults.shape(PleyShape.Control),
                colors = ClickableSurfaceDefaults.colors(
                    containerColor = if (isCurrent) PleyColors.Surface3 else Color.Transparent,
                    contentColor = PleyColors.Fg,
                    focusedContainerColor = PleyColors.Accent,
                    focusedContentColor = PleyColors.OnAccent,
                    pressedContainerColor = PleyColors.AccentBright,
                    pressedContentColor = PleyColors.OnAccent,
                ),
                scale = ClickableSurfaceDefaults.scale(focusedScale = 1f, pressedScale = 1f),
                glow = ClickableSurfaceDefaults.glow(Glow.None, Glow.None, Glow.None),
            ) {
                val content = androidx.tv.material3.LocalContentColor.current
                Row(Modifier.padding(horizontal = 10.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    ChannelTile(channel, origin, Modifier.size(width = 52.dp, height = 32.dp), well = Color.White.copy(alpha = 0.08f))
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        BasicText(
                            listOfNotNull(channel.number, channel.name).joinToString("  "),
                            style = PleyType.Tv.Label.copy(color = content),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (on != null) {
                            BasicText(on.title, style = PleyType.Tv.Caption.copy(color = content.copy(alpha = 0.75f)), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                    if (channel.favorite) Icon(PleyIcons.Star, contentDescription = null, size = 14.dp, tint = content)
                }
            }
        }
    }
}

/** What's on shows this long after tuning or a seek. */
private const val INFO_MS = 5_000L

/** The controls hide after this long untouched while playing. */
private const val CONTROLS_MS = 6_000L

/** Left, Right and the buttons go this far back or on. */
private const val SEEK_MS = 10_000L

internal const val TV_LIVE_PLAYER = "pley.tv.live.player"
internal const val TV_LIVE_PLAYER_INFO = "pley.tv.live.player.info"
internal const val TV_LIVE_PLAYER_PLAY = "pley.tv.live.player.play"
internal const val TV_LIVE_PLAYER_LIVE = "pley.tv.live.player.live"
internal const val TV_LIVE_PLAYER_DIGITS = "pley.tv.live.player.digits"
internal const val TV_LIVE_PLAYER_TROUBLE = "pley.tv.live.player.trouble"
internal const val TV_LIVE_PLAYER_CHANNELS = "pley.tv.live.player.channels"
