package app.pley.android.ui.tv.player

import android.view.KeyEvent as AndroidKeyEvent
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyElevation
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.reducedMotion
import app.pley.android.player.PlaybackController
import app.pley.android.player.PlayerUiState
import app.pley.android.player.engine.EngineState
import app.pley.android.player.engine.PlaybackStats
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.item.PanelHeading
import app.pley.android.ui.tv.item.PanelRow
import kotlinx.coroutines.launch
import java.util.Locale

/**
 * The player's side panels (design spec §2.7): Subtitles, Audio, Quality,
 * Chapters and Info. 360 dp from the right edge, full height, surface2
 * with 16 dp left corners. It slides in on its own layer over 200 ms (in
 * place with reduced motion) and the scrim behind it fades by its colour
 * while drawing, so no layer covers the video. Focus starts on the current
 * choice and stays inside; choosing closes the panel, and Back does too.
 */
@Composable
internal fun BoxScope.TvPlayerPanels(
    ui: TvPlayerUi,
    state: State<PlayerUiState>,
    engine: State<EngineState?>,
    controller: PlaybackController,
) {
    val open = ui.panel != null
    val reduced = reducedMotion()
    val shown by animateFloatAsState(if (open) 1f else 0f, PleyMotion.overlay(reduced), label = "panel")
    val present by remember { derivedStateOf { open || shown > 0f } }
    if (!present) return
    Box(Modifier.fillMaxSize().drawBehind { drawRect(PleyColors.Scrim.copy(alpha = PleyColors.Scrim.alpha * shown)) })
    val panel = ui.panel ?: ui.lastPanel ?: return
    Box(
        Modifier
            .align(Alignment.CenterEnd)
            .width(PANEL_WIDTH)
            .fillMaxHeight()
            .graphicsLayer { translationX = (1f - shown) * size.width }
            .clip(RoundedCornerShape(topStart = PleyShape.DialogRadius, bottomStart = PleyShape.DialogRadius))
            .drawBehind {
                drawRect(PleyColors.Surface2)
                drawRect(PleyColors.Surface4, size = androidx.compose.ui.geometry.Size(size.width, PleyElevation.Level2Hairline.toPx()))
            }
            .testTag(TAG_PANEL)
            // Focus stays in the panel until it closes.
            .focusProperties { onExit = { if (ui.panel != null) cancelFocusChange() } }
            .focusGroup()
            .padding(start = 24.dp, end = TvSafeArea.Horizontal, top = TvSafeArea.Vertical, bottom = TvSafeArea.Vertical),
    ) {
        when (panel) {
            PlayerPanel.Info -> InfoPanel(state, engine, ui)
            else -> ChoicePanel(panel, state, ui, controller)
        }
    }
}

/** A list of choices, the current one ticked and focused first. */
@Composable
private fun ChoicePanel(panel: PlayerPanel, state: State<PlayerUiState>, ui: TvPlayerUi, controller: PlaybackController) {
    val s by remember(state) { derivedStateOf { PanelData.of(state.value) } }
    val focus = ui.requester(PlayerFocus.Panel)
    val title = stringResource(
        when (panel) {
            PlayerPanel.Subtitles -> R.string.tv_player_subtitles
            PlayerPanel.Audio -> R.string.tv_player_audio
            PlayerPanel.Quality -> R.string.tv_player_quality
            PlayerPanel.Chapters -> R.string.tv_player_chapters
            PlayerPanel.Info -> R.string.tv_player_info_title
        },
    )
    val burnIn = stringResource(R.string.tv_player_burn_in)
    val original = stringResource(R.string.tv_player_play_original)
    val originalHint = stringResource(R.string.tv_player_play_original_hint)
    val resources = LocalResources.current
    LazyColumn(Modifier.fillMaxSize()) {
        item(key = "title") { PanelTitle(title) }
        when (panel) {
            PlayerPanel.Subtitles -> choices(s.subtitles.map { Row(it.label, it.selected, if (it.burnIn) burnIn else null) }, focus) { i ->
                controller.selectSubtitle(s.subtitles[i].index)
                ui.closePanel()
            }
            PlayerPanel.Audio -> choices(s.audio.map { Row(it.label, it.selected, null) }, focus) { i ->
                s.audio[i].index?.let(controller::selectAudio)
                ui.closePanel()
            }
            PlayerPanel.Quality -> {
                s.note?.let { note -> item(key = "note") { PanelNote(note) } }
                if (s.canPlayOriginal) {
                    item(key = "original") {
                        PanelRow(original, onClick = {
                            controller.playOriginal()
                            ui.closePanel()
                        }, supporting = originalHint, icon = PleyIcons.RotateCcw)
                    }
                }
                choices(s.qualities.map { Row(it.label, it.selected, it.hint) }, focus) { i ->
                    controller.selectQuality(s.qualities[i].id)
                    ui.closePanel()
                }
            }
            PlayerPanel.Chapters -> {
                val playing = TvPlayerText.chapterAt(s.chapters, s.positionMs)
                choices(
                    s.chapters.mapIndexed { i, c -> Row(c.title?.takeIf { it.isNotBlank() } ?: resources.getString(R.string.tv_player_chapter, i + 1), i == playing, TvPlayerText.time(c.startMs)) },
                    focus,
                ) { i ->
                    controller.seekTo(s.chapters[i].startMs)
                    ui.closePanel()
                }
            }
            PlayerPanel.Info -> Unit
        }
    }
}

private data class Row(val label: String, val checked: Boolean, val supporting: String?)

/** Rows of [rows]; the checked one (or the first) takes [focus]. */
private fun LazyListScope.choices(rows: List<Row>, focus: FocusRequester, onChoose: (Int) -> Unit) {
    val first = rows.indexOfFirst { it.checked }.takeIf { it >= 0 } ?: 0
    itemsIndexed(rows, key = { i, _ -> "row-$i" }) { i, row ->
        PanelRow(
            row.label,
            onClick = { onChoose(i) },
            checked = row.checked,
            supporting = row.supporting,
            focusRequester = if (i == first) focus else null,
        )
    }
}

/** What the choice panels read, so they recompose with their choices, not with the playhead. */
private data class PanelData(
    val subtitles: List<app.pley.android.player.TrackChoice>,
    val audio: List<app.pley.android.player.TrackChoice>,
    val qualities: List<app.pley.android.player.QualityChoice>,
    val chapters: List<app.pley.android.player.ChapterMark>,
    val note: String?,
    val canPlayOriginal: Boolean,
    /** To the chapter, so the chapters panel ticks the one playing. */
    val positionMs: Long,
) {
    companion object {
        fun of(s: PlayerUiState) = PanelData(
            subtitles = s.subtitles,
            audio = s.audio,
            qualities = s.qualities,
            chapters = s.chapters,
            note = s.note,
            canPlayOriginal = s.canPlayOriginal,
            positionMs = TvPlayerText.chapterAt(s.chapters, s.positionMs)?.let { s.chapters[it].startMs } ?: 0,
        )
    }
}

/**
 * Info, the stats for nerds: how the server sends it (method, why, the
 * streams) and what this TV does with it (decoders, the picture, the
 * display mode, audio passed through or decoded, dropped frames, the
 * buffer). Up and Down scroll it.
 */
@Composable
private fun InfoPanel(state: State<PlayerUiState>, engine: State<EngineState?>, ui: TvPlayerUi) {
    val details by remember(state) { derivedStateOf { state.value.details } }
    val buffered by remember(state) { derivedStateOf { ((state.value.bufferedMs - state.value.positionMs).coerceAtLeast(0) / 1000).toInt() } }
    val stats = engine.value?.stats
    val scroll = rememberScrollState()
    val scope = rememberCoroutineScope()
    val step = with(LocalDensity.current) { 96.dp.toPx() }
    Column(
        Modifier
            .fillMaxSize()
            .focusRequester(ui.requester(PlayerFocus.Panel))
            .onKeyEvent { event ->
                val native = event.nativeKeyEvent
                if (native.action != AndroidKeyEvent.ACTION_DOWN) return@onKeyEvent false
                when (native.keyCode) {
                    AndroidKeyEvent.KEYCODE_DPAD_DOWN -> {
                        scope.launch { scroll.animateScrollBy(step) }
                        true
                    }
                    AndroidKeyEvent.KEYCODE_DPAD_UP -> {
                        scope.launch { scroll.animateScrollBy(-step) }
                        true
                    }
                    else -> false
                }
            }
            .focusable()
            .verticalScroll(scroll),
    ) {
        PanelTitle(stringResource(R.string.tv_player_info_title))
        PanelHeading(stringResource(R.string.tv_player_info_how))
        val d = details
        if (d == null) {
            PanelNote(stringResource(R.string.tv_player_info_waiting))
        } else {
            InfoLine(d.method + (d.pipeline?.let { " · $it" } ?: ""), d.why)
            d.rows.forEach { InfoLine(it.label, it.value) }
        }
        PanelHeading(stringResource(R.string.tv_player_info_device))
        if (stats != null) {
            DeviceLines(stats, buffered)
        } else {
            d?.engine?.forEach { line ->
                val parts = line.split(": ", limit = 2)
                if (parts.size == 2) InfoLine(parts[0], parts[1]) else InfoLine("", line)
            }
        }
        Spacer(Modifier.height(24.dp))
    }
}

/** The lines about this TV, from the engine's stats. */
@Composable
private fun DeviceLines(stats: PlaybackStats, bufferedS: Int) {
    stats.videoDecoder?.let { name -> InfoLine(stringResource(R.string.tv_player_info_video_decoder), name + hardwareNote(stats.videoDecoderHardware)) }
    val picture = listOfNotNull(
        if (stats.width != null && stats.height != null) "${stats.width}×${stats.height}" else null,
        stats.frameRate?.takeIf { it > 0 }?.let { String.format(Locale.ROOT, "%.3f fps", it).replace(".000 fps", " fps") },
        stats.dynamicRange,
        stats.videoCodec?.let { codecName(it.substringBefore(' ')) },
    )
    if (picture.isNotEmpty()) InfoLine(stringResource(R.string.tv_player_info_picture), picture.joinToString(" · "))
    InfoLine(
        stringResource(R.string.tv_player_info_display),
        stringResource(if (stats.displayModeId != null) R.string.tv_player_info_display_switched else R.string.tv_player_info_display_kept),
    )
    stats.audioCodec?.let { codec ->
        val audio = listOfNotNull(
            codecName(codec),
            stats.audioChannels?.let { "$it ch" },
            stringResource(if (stats.passthrough) R.string.tv_player_info_passthrough else R.string.tv_player_info_decoded),
        )
        InfoLine(stringResource(R.string.tv_player_info_audio), audio.joinToString(" · "))
    }
    stats.audioDecoder?.takeIf { !stats.passthrough }?.let { InfoLine(stringResource(R.string.tv_player_info_audio_decoder), it) }
    InfoLine(
        stringResource(R.string.tv_player_info_dropped),
        stringResource(R.string.tv_player_info_dropped_value, stats.droppedFrames, stats.renderedFrames + stats.droppedFrames),
    )
    InfoLine(stringResource(R.string.tv_player_info_rebuffers), stats.rebuffers.toString())
    InfoLine(stringResource(R.string.tv_player_info_buffer), stringResource(R.string.tv_player_info_buffer_value, bufferedS))
    stats.firstFrameMs?.let { InfoLine(stringResource(R.string.tv_player_info_first_frame), stringResource(R.string.tv_player_info_first_frame_value, it.toInt())) }
}

@Composable
private fun hardwareNote(hardware: Boolean?): String = when (hardware) {
    true -> " (" + stringResource(R.string.tv_player_info_hardware) + ")"
    false -> " (" + stringResource(R.string.tv_player_info_software) + ")"
    null -> ""
}

/** A MIME type as people know the codec: "video/hevc" is HEVC, "audio/eac3-joc" is E-AC-3 JOC (Atmos). */
internal fun codecName(mime: String): String = when (mime.lowercase(Locale.ROOT)) {
    "video/hevc" -> "HEVC"
    "video/avc" -> "H.264"
    "video/dolby-vision" -> "Dolby Vision"
    "video/av01" -> "AV1"
    "video/x-vnd.on2.vp9" -> "VP9"
    "video/mpeg2" -> "MPEG-2"
    "audio/ac3" -> "AC-3"
    "audio/eac3" -> "E-AC-3"
    "audio/eac3-joc" -> "E-AC-3 JOC (Atmos)"
    "audio/true-hd" -> "TrueHD"
    "audio/vnd.dts" -> "DTS"
    "audio/vnd.dts.hd" -> "DTS-HD"
    "audio/mp4a-latm" -> "AAC"
    "audio/opus" -> "Opus"
    "audio/flac" -> "FLAC"
    "audio/raw" -> "PCM"
    else -> mime.substringAfter('/')
}

@Composable
private fun InfoLine(label: String, value: String) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        if (label.isNotEmpty()) BasicText(label, style = PleyType.Tv.Caption.copy(color = PleyColors.Muted), maxLines = 1)
        BasicText(value, style = PleyType.Tv.Body.copy(color = PleyColors.Fg), maxLines = 4, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun PanelTitle(text: String) {
    BasicText(text, style = PleyType.Tv.Title.copy(color = PleyColors.Fg), maxLines = 1, modifier = Modifier.padding(start = 16.dp, top = 8.dp, bottom = 8.dp))
}

@Composable
private fun PanelNote(text: String) {
    BasicText(text, style = PleyType.Tv.Body.copy(color = PleyColors.Muted), modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
}

private val PANEL_WIDTH = 360.dp

internal const val TAG_PANEL = "pley.tv.player.panel"
