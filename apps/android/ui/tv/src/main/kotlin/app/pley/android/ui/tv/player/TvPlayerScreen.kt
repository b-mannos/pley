package app.pley.android.ui.tv.player

import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.media3.common.Player
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.reducedMotion
import app.pley.android.player.PlaybackController
import app.pley.android.player.PlayerPhase
import app.pley.android.player.PlayerUiState
import app.pley.android.player.engine.EngineState
import app.pley.android.player.engine.SubtitleStyle
import app.pley.android.player.engine.Subtitles
import app.pley.android.player.engine.VideoSurface
import app.pley.android.ui.tv.R
import kotlinx.coroutines.delay

/**
 * The TV player (design spec §2.7, TV addendum §5.2) over a
 * [PlaybackController]: the video on its SurfaceView, subtitles, and the
 * remote's keys.
 *
 * - **Controls hidden.** Only the video and subtitles show; Skip and Up
 *   Next are all that appear unasked. Centre plays or pauses (a showing
 *   Skip button takes it), Left and Right seek 10 s (held: scrub, seeking
 *   once on release), Up shows the controls on the scrubber, Down on the
 *   track buttons.
 * - **Controls shown.** Focus moves as usual; they hide after 5 s without
 *   a key unless paused or in a panel.
 * - **Back** walks out: panel, then Up Next, then the controls, then the
 *   player ([onExit]).
 * - **Media keys** work whatever shows.
 *
 * [onExit] leaves the player for where playing started; [now] is the wall
 * clock, for the clock and "Ends at".
 */
@Composable
internal fun TvPlayerScreen(
    controller: PlaybackController,
    onExit: () -> Unit,
    modifier: Modifier = Modifier,
    subtitleStyle: SubtitleStyle = SubtitleStyle(),
    systemCaptions: Boolean = false,
    now: () -> Long = System::currentTimeMillis,
) {
    val state = controller.state.collectAsState()
    val engineState = rememberEngineState(controller)
    val ui = remember(controller) { TvPlayerUi() }
    val seeker = remember(controller) { TvSeeker({ state.value.positionMs }, { state.value.durationMs }, controller::seekTo) }
    val resources = LocalResources.current
    val exit by rememberUpdatedState(onExit)

    val phase by remember(state) { derivedStateOf { state.value.phase } }
    val error = phase as? PlayerPhase.Error
    val switching by remember(engineState) { derivedStateOf { engineState.value?.switchingDisplayMode == true } }
    val switchingHz by remember(engineState) { derivedStateOf { engineState.value?.switchingToHz } }
    val card by remember(state) { derivedStateOf { state.value.upNextCard } }
    val skipLabel by remember(state) { derivedStateOf { state.value.skipLabel } }
    val playing by remember(state) { derivedStateOf { state.value.phase.wantsToPlay() } }
    val options by remember(state) {
        derivedStateOf {
            val s = state.value
            RowOptions(
                previousNext = s.subtitle != null || s.chapters.size > 1,
                subtitles = s.subtitles.isNotEmpty(),
                audio = s.audio.size > 1,
                quality = s.qualities.isNotEmpty(),
                chapters = s.chapters.size > 1,
            )
        }
    }
    val previewing by remember(seeker) { derivedStateOf { seeker.target != null } }
    // Skip shows while its marker runs, unless a seek preview, Up Next or an error is on screen.
    val skipShown = skipLabel != null && card == null && error == null && !switching && !(ui.controls && previewing)

    // ---------------------------------------------------------------- what the keys do

    fun playPause() {
        if (state.value.phase.wantsToPlay()) {
            controller.pause()
            if (!ui.controls) ui.showControls(PlayerFocus.Scrubber)
        } else {
            controller.play()
        }
    }

    fun focusFor(focus: ControlFocus): PlayerFocus = when (focus) {
        ControlFocus.Scrubber -> PlayerFocus.Scrubber
        ControlFocus.Tracks -> if (state.value.subtitles.isNotEmpty()) PlayerFocus.Subtitles else PlayerFocus.Info
        ControlFocus.Default -> if (skipShown) PlayerFocus.Skip else PlayerFocus.Scrubber
    }

    fun next() {
        val s = state.value
        val pos = seeker.target ?: s.positionMs
        val chapter = s.chapters.firstOrNull { it.startMs > pos + CHAPTER_SLACK_MS }
        when {
            s.subtitle != null -> controller.playNext()
            chapter != null -> controller.seekTo(chapter.startMs)
        }
    }

    fun previous() {
        val s = state.value
        val pos = seeker.target ?: s.positionMs
        val current = TvPlayerText.chapterAt(s.chapters, pos)
        val to = when {
            current == null -> 0L
            pos - s.chapters[current].startMs > RESTART_CHAPTER_MS || current == 0 -> s.chapters[current].startMs
            else -> s.chapters[current - 1].startMs
        }
        controller.seekTo(to)
    }

    fun toggleCaptions() {
        val subs = state.value.subtitles
        if (subs.isEmpty()) return
        val on = subs.firstOrNull { it.selected && it.index != null }
        if (on != null) {
            ui.rememberedSubtitle = on.index
            controller.selectSubtitle(null)
            ui.toast(resources.getString(R.string.tv_player_subtitles_off_toast), TvPlayerUi.CAPTIONS_TOAST_MS)
        } else {
            val pick = subs.firstOrNull { it.index != null && it.index == ui.rememberedSubtitle }
                ?: subs.firstOrNull { it.index != null && !it.burnIn }
                ?: subs.firstOrNull { it.index != null }
                ?: return
            controller.selectSubtitle(pick.index)
            ui.toast(resources.getString(R.string.tv_player_subtitles_toast, pick.label), TvPlayerUi.CAPTIONS_TOAST_MS)
        }
    }

    fun nextAudio() {
        val audio = state.value.audio
        if (audio.size < 2) return
        val i = audio.indexOfFirst { it.selected }
        val pick = audio[(i + 1).mod(audio.size)]
        pick.index?.let(controller::selectAudio)
        ui.toast(resources.getString(R.string.tv_player_audio_toast, pick.label), TvPlayerUi.CAPTIONS_TOAST_MS)
    }

    fun act(key: PlayerKey) {
        when (key) {
            PlayerKey.PlayPause -> playPause()
            PlayerKey.Play -> controller.play()
            PlayerKey.Pause -> {
                controller.pause()
                if (!ui.controls && card == null && error == null) ui.showControls(PlayerFocus.Scrubber)
            }
            is PlayerKey.Jump -> seeker.press(if (key.deltaMs < 0) -1 else 1, kotlin.math.abs(key.deltaMs))
            PlayerKey.Next -> next()
            PlayerKey.Previous -> previous()
            PlayerKey.Captions -> toggleCaptions()
            PlayerKey.AudioTrack -> nextAudio()
            PlayerKey.Stop -> exit()
            is PlayerKey.Show -> ui.showControls(focusFor(key.focus))
            is PlayerKey.Step -> seeker.press(key.direction)
        }
    }

    /** Left, Right, Centre and the keys that show the controls, as [TvPlayerKeys] sorted them. */
    fun handle(key: PlayerKey, native: AndroidKeyEvent, busy: Boolean): Boolean {
        val down = native.action == AndroidKeyEvent.ACTION_DOWN
        val first = down && native.repeatCount == 0
        when (key) {
            is PlayerKey.Step -> when {
                busy -> Unit
                first -> seeker.press(key.direction)
                down -> {
                    seeker.repeat(key.direction, native.eventTime)
                    if (!ui.controls) ui.showControls(PlayerFocus.Scrubber)
                }
                else -> seeker.release()
            }
            PlayerKey.PlayPause -> if (first && !busy) playPause()
            else -> if (first) act(key)
        }
        return true
    }

    fun onKey(event: KeyEvent): Boolean {
        val native = event.nativeKeyEvent
        val code = native.keyCode
        TvPlayerKeys.media(code)?.let { key ->
            if (native.action == AndroidKeyEvent.ACTION_DOWN && native.repeatCount == 0) act(key)
            return true
        }
        if (state.value.phase is PlayerPhase.Error) return false
        if (state.value.upNextCard != null) {
            return code == AndroidKeyEvent.KEYCODE_DPAD_UP || code == AndroidKeyEvent.KEYCODE_DPAD_DOWN
        }
        if (ui.panel != null) {
            ui.touch()
            return false
        }
        val busy = state.value.phase == PlayerPhase.Preparing || switching
        if (!ui.controls) {
            val key = TvPlayerKeys.hidden(code, skipShowing = skipShown && ui.focused == PlayerFocus.Skip) ?: return false
            return handle(key, native, busy)
        }
        ui.touch()
        if (ui.focused == PlayerFocus.Scrubber) {
            val key = TvPlayerKeys.scrubber(code) ?: return false
            return handle(key, native, busy)
        }
        return false
    }

    BackHandler {
        when {
            ui.panel != null -> ui.closePanel()
            state.value.upNextCard != null -> controller.cancelUpNext()
            ui.controls -> ui.hideControls(if (skipShown) PlayerFocus.Skip else PlayerFocus.Video)
            else -> exit()
        }
    }

    // ---------------------------------------------------------------- what follows the playback

    // Focus goes where it was asked for, once that is laid out.
    LaunchedEffect(ui.focusAsk) {
        val requester = ui.requester(ui.focusAsk.target)
        repeat(FOCUS_TRIES) {
            withFrameNanos { }
            if (runCatching { requester.requestFocus() }.getOrDefault(false)) return@LaunchedEffect
        }
    }
    // Skip takes focus while its marker runs; when it goes, focus goes back to the video or the scrubber.
    LaunchedEffect(skipShown) {
        when {
            skipShown && !ui.controls && ui.panel == null -> ui.ask(PlayerFocus.Skip)
            !skipShown && ui.focused == PlayerFocus.Skip -> ui.ask(if (ui.controls) PlayerFocus.Scrubber else PlayerFocus.Video)
        }
    }
    // Up Next takes the screen's attention: the controls and panels make way, Play now has focus.
    LaunchedEffect(card != null) {
        if (card != null) {
            seeker.cancel()
            ui.closePanel(refocus = false)
            ui.hideControls(PlayerFocus.UpNext)
            ui.ask(PlayerFocus.UpNext)
        } else if (ui.focused == PlayerFocus.UpNext) {
            ui.ask(PlayerFocus.Video)
        }
    }
    // An error takes the screen, with Try again focused.
    LaunchedEffect(error != null) {
        if (error != null) {
            seeker.cancel()
            ui.closePanel(refocus = false)
            ui.hideControls(PlayerFocus.Error)
            ui.ask(PlayerFocus.Error)
        } else if (ui.focused == PlayerFocus.Error) {
            ui.ask(PlayerFocus.Video)
        }
    }
    // The controls hide after 5 s without a key, unless paused, scrubbing or in a panel.
    LaunchedEffect(ui.controls, ui.panel, ui.input, playing, previewing) {
        if (ui.controls && ui.panel == null && playing && !previewing) {
            delay(TvPlayerUi.HIDE_AFTER_MS)
            ui.hideControls(if (skipShown) PlayerFocus.Skip else PlayerFocus.Video)
        }
    }
    // Presses in a row seek once they stop; the badge goes a little later.
    LaunchedEffect(seeker.presses) {
        val press = seeker.presses
        delay(TvSeeker.SETTLE_MS)
        seeker.settle(press)
        delay(TvSeeker.BADGE_MS - TvSeeker.SETTLE_MS)
        seeker.hideBadge(press)
    }
    // The playback's messages, as toasts.
    val notice by remember(state) { derivedStateOf { state.value.notice } }
    LaunchedEffect(notice) {
        notice?.let {
            ui.toast(it)
            controller.dismissNotice()
        }
    }
    val hint by remember(engineState) { derivedStateOf { engineState.value?.frameRateHint == true } }
    LaunchedEffect(hint) {
        if (hint) ui.toast(resources.getString(R.string.tv_player_frame_rate_hint), TvPlayerUi.ERROR_TOAST_MS)
    }
    // The Info panel's numbers are asked for while it is open.
    val infoOpen = ui.panel == PlayerPanel.Info
    val details = remember(controller) { DetailsWatch(controller) }
    LaunchedEffect(infoOpen) { details.open(infoOpen) }
    DisposableEffect(details) { onDispose { details.open(false) } }

    // ---------------------------------------------------------------- the screen

    val reduced = reducedMotion()
    val controlsAlpha = animateFloatAsState(if (ui.controls) 1f else 0f, PleyMotion.overlay(reduced), label = "controls")
    val controlsPresent by remember { derivedStateOf { ui.controls || controlsAlpha.value > 0f } }
    val player by remember(controller) { derivedStateOf { engineState.value.let { controller.engine?.player } } }
    val videoLabel = stringResource(R.string.tv_player_play)

    Box(
        modifier
            .fillMaxSize()
            .background(Color.Black)
            .onPreviewKeyEvent(::onKey)
            .testTag(TAG_PLAYER),
    ) {
        VideoSurface(player, Modifier.fillMaxSize())
        PlayerSubtitles(controller, player, subtitleStyle, systemCaptions, lifted = ui.controls)
        // Holds focus while the controls are hidden: the keys arrive through it.
        Box(Modifier.fillMaxSize().videoFocus(ui, enabled = !ui.controls, label = videoLabel))
        Box(Modifier.fillMaxSize().controlScrims(controlsAlpha))
        if (controlsPresent) {
            TvPlayerTopBar(state, controlsAlpha, now)
            if (ui.controls) TvSeekPreview(state, seeker, controller)
            TvScrubber(state, seeker, ui, controller, controlsAlpha, skipShown)
            TvControlRow(
                playing = playing,
                options = options,
                ui = ui,
                alpha = controlsAlpha,
                onPlayPause = ::playPause,
                onBack10 = { seeker.press(-1) },
                onForward10 = { seeker.press(1) },
                onPrevious = ::previous,
                onNext = ::next,
            )
        }
        TvSkipButton(state, skipShown, ui.controls, ui) { controller.skip() }
        TvUpNextCard(card, ui, onPlay = controller::playNext, onCancel = controller::cancelUpNext)
        if (!ui.controls) TvSeekBadge(seeker.badgeMs)
        TvBufferingRing(waiting = (phase == PlayerPhase.Buffering || phase == PlayerPhase.Preparing) && !switching)
        if (switching || phase == PlayerPhase.Switching) TvSwitchingCover(switchingHz)
        TvPlayerPanels(ui, state, engineState, controller)
        if (error != null) TvPlayerError(error.message, error.canRetry, ui, onRetry = controller::retry, onBack = exit)
        TvPlayerToast(ui.toast, ui.controls, ui::clearToast)
    }
}

/**
 * Subtitles over the video: the server's when it sends them, else the
 * file's own through the player; moved up above the controls while they
 * show. Collects the cues itself, so they change nothing else.
 */
@Composable
private fun PlayerSubtitles(controller: PlaybackController, player: Player?, style: SubtitleStyle, system: Boolean, lifted: Boolean) {
    val cues by controller.cues.collectAsState()
    Subtitles(
        player,
        style,
        Modifier.fillMaxSize().padding(bottom = if (lifted) SUBTITLES_LIFT else 0.dp),
        cues = cues.ifEmpty { null },
        system = system,
    )
}

/** Asks for the playback's details while the Info panel is open, and stops asking once it closes. */
private class DetailsWatch(private val controller: PlaybackController) {
    private var asked = false

    fun open(open: Boolean) {
        if (open == asked) return
        asked = open
        controller.setDetailsOpen(open)
    }
}

/** The engine's state, once the playback has an engine; `null` before (and in tests without one). */
@Composable
private fun rememberEngineState(controller: PlaybackController): State<EngineState?> = produceState<EngineState?>(null, controller) {
    var engine = controller.engine
    while (engine == null) {
        delay(ENGINE_POLL_MS)
        engine = controller.engine
    }
    engine.state.collect { value = it }
}

private const val FOCUS_TRIES = 8
private const val ENGINE_POLL_MS = 250L

/** Next goes to a chapter starting at least this far ahead. */
private const val CHAPTER_SLACK_MS = 1_000L

/** Previous restarts the chapter playing when it is this far in, else goes to the one before. */
private const val RESTART_CHAPTER_MS = 3_000L

/** Subtitles move up above the controls while they show. */
private val SUBTITLES_LIFT = 150.dp

internal const val TAG_PLAYER = "pley.tv.player"
