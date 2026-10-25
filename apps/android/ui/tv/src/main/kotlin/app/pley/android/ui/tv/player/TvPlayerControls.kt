package app.pley.android.ui.tv.player

import android.text.format.DateFormat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.constrainWidth
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.Image
import androidx.tv.material3.Border
import androidx.tv.material3.Glow
import androidx.tv.material3.IconButton
import androidx.tv.material3.IconButtonDefaults
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyFocus
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.image.Artwork
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.model.MarkerKind
import app.pley.android.player.PlaybackController
import app.pley.android.player.PlayerPhase
import app.pley.android.player.PlayerUiState
import app.pley.android.player.markers.Markers
import app.pley.android.ui.tv.R

/*
 * The controls of design spec §2.7, on the 960 × 540 canvas: the title and
 * clock at the top, the scrubber with its times and preview, and the
 * control row. Each part fades on its own small layer ([alpha]); the
 * playhead is read where it is drawn, so the rest doesn't recompose with it.
 */

/** The title, the episode's line, the clock and, while paused, when it ends. */
@Composable
internal fun BoxScope.TvPlayerTopBar(state: State<PlayerUiState>, alpha: State<Float>, now: () -> Long) {
    val title by remember(state) { derivedStateOf { state.value.title } }
    val line by remember(state) { derivedStateOf { TvPlayerText.episodeLine(state.value.subtitle) } }
    val paused by remember(state) { derivedStateOf { state.value.phase == PlayerPhase.Paused } }
    Column(
        Modifier
            .align(Alignment.TopStart)
            .padding(start = TvSafeArea.Horizontal, top = TvSafeArea.Vertical, end = 300.dp)
            .graphicsLayer { this.alpha = alpha.value },
    ) {
        BasicText(title, style = PleyType.Tv.Title.copy(color = PleyColors.Fg), maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (line != null) {
            Spacer(Modifier.height(4.dp))
            BasicText(line!!, style = PleyType.Tv.Label.copy(color = PleyColors.Muted), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
    Column(
        Modifier
            .align(Alignment.TopEnd)
            .padding(end = TvSafeArea.Horizontal, top = TvSafeArea.Vertical)
            .graphicsLayer { this.alpha = alpha.value },
        horizontalAlignment = Alignment.End,
    ) {
        val clock = rememberClock(now)
        BasicText(clock, style = PleyType.Tv.Label.copy(color = PleyColors.Muted))
        if (paused) {
            val context = LocalContext.current
            val ends by remember(state) {
                derivedStateOf {
                    val s = state.value
                    val left = (s.durationMs - s.positionMs).coerceAtLeast(0)
                    DateFormat.getTimeFormat(context).format(now() + left)
                }
            }
            BasicText(stringResource(R.string.tv_player_ends_at, ends), style = PleyType.Tv.Label.copy(color = PleyColors.Muted))
        }
    }
}

/** The time of day, as the TV writes it (12 or 24 hours), changing on the minute. */
@Composable
private fun rememberClock(now: () -> Long): String {
    val context = LocalContext.current
    val format = remember(context) { DateFormat.getTimeFormat(context) }
    val text by produceState(format.format(now())) {
        while (true) {
            val t = now()
            value = format.format(t)
            kotlinx.coroutines.delay(MINUTE_MS - t % MINUTE_MS + 50)
        }
    }
    return text
}

/**
 * The scrubber (design spec §2.7) from x = 48 to 912, its track's centre
 * at y = 397: the played part in accent, buffered in fg 45 %, gaps at
 * chapter starts; the thumb grows with the focus ring while focused; the
 * times above, a running marker's name below. Left and Right on it seek
 * ([TvSeeker], keys handled by the screen); it says its place to screen
 * readers as a slider.
 */
@Composable
internal fun BoxScope.TvScrubber(
    state: State<PlayerUiState>,
    seeker: TvSeeker,
    ui: TvPlayerUi,
    controller: PlaybackController,
    alpha: State<Float>,
    skipShown: Boolean,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    val thumbScale by animateFloatAsState(if (focused) 1f else 0f, PleyMotion.focus(), label = "thumb")
    val elapsed by remember(state) { derivedStateOf { state.value.positionMs / 1000 } }
    val duration by remember(state) { derivedStateOf { state.value.durationMs } }
    val remaining by remember(state) { derivedStateOf { (state.value.durationMs - state.value.positionMs).coerceAtLeast(0) / 1000 } }
    val skip = ui.requester(PlayerFocus.Skip)
    val down = ui.requester(PlayerFocus.PlayPause)
    Box(
        Modifier
            .align(Alignment.TopStart)
            .padding(top = TIMES_TOP)
            .fillMaxWidth()
            .padding(horizontal = TvSafeArea.Horizontal)
            .graphicsLayer { this.alpha = alpha.value },
    ) {
        BasicText(TvPlayerText.time(elapsed * 1000), style = PleyType.Tv.Caption.copy(color = PleyColors.Fg), modifier = Modifier.align(Alignment.TopStart))
        BasicText(TvPlayerText.remaining(0, remaining * 1000), style = PleyType.Tv.Caption.copy(color = PleyColors.Fg), modifier = Modifier.align(Alignment.TopEnd))
        val spoken = stringResource(R.string.tv_player_position, TvPlayerText.spoken(elapsed * 1000), TvPlayerText.spoken(duration))
        Box(
            Modifier
                .padding(top = TRACK_CENTRE - TIMES_TOP - SCRUB_HEIGHT / 2)
                .fillMaxWidth()
                .height(SCRUB_HEIGHT)
                .testTag(TAG_SCRUBBER)
                .focusRequester(ui.requester(PlayerFocus.Scrubber))
                .onFocusChanged { if (it.isFocused) ui.focused = PlayerFocus.Scrubber else if (ui.focused == PlayerFocus.Scrubber) ui.focused = null }
                .focusProperties {
                    up = if (skipShown) skip else FocusRequester.Cancel
                    this.down = down
                    left = FocusRequester.Cancel
                    right = FocusRequester.Cancel
                }
                .semantics {
                    contentDescription = spoken
                    stateDescription = spoken
                    val d = state.value.durationMs.coerceAtLeast(1)
                    progressBarRangeInfo = ProgressBarRangeInfo(state.value.positionMs.toFloat() / d, 0f..1f)
                    setProgress { fraction ->
                        controller.seekTo((fraction * d).toLong())
                        true
                    }
                }
                .focusable(interactionSource = interactions)
                .drawBehind { drawTrack(state.value, seeker.target, thumbScale) },
        )
        MarkerLabel(state)
    }
}

/** The track, the played and buffered parts, chapter gaps and the thumb. */
private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawTrack(s: PlayerUiState, target: Long?, focus: Float) {
    val h = TRACK_HEIGHT.toPx()
    val y = size.height / 2f - h / 2f
    val w = size.width
    val d = s.durationMs.toFloat().coerceAtLeast(1f)
    val gap = CHAPTER_GAP.toPx()
    val r = CornerRadius(h / 2f, h / 2f)
    fun x(ms: Long) = (ms / d).coerceIn(0f, 1f) * w
    val starts = s.chapters.map { x(it.startMs) }.filter { it > gap && it < w - gap }
    // Segments between chapter gaps, each drawn in three layers.
    val edges = buildList {
        add(0f)
        starts.forEach { add(it) }
        add(w)
    }
    val played = x(s.positionMs)
    val buffered = x(s.bufferedMs.coerceAtLeast(s.positionMs))
    for (i in 0 until edges.size - 1) {
        val a = edges[i] + if (i == 0) 0f else gap / 2f
        val b = edges[i + 1] - if (i == edges.size - 2) 0f else gap / 2f
        if (b <= a) continue
        drawRoundRect(TRACK, Offset(a, y), Size(b - a, h), r)
        if (buffered > a) drawRoundRect(BUFFERED, Offset(a, y), Size(minOf(buffered, b) - a, h), r)
        if (played > a) drawRoundRect(PleyColors.Accent, Offset(a, y), Size(minOf(played, b) - a, h), r)
    }
    val thumbX = x(target ?: s.positionMs)
    val radius = (THUMB.toPx() + (THUMB_FOCUSED.toPx() - THUMB.toPx()) * focus) / 2f
    val centre = Offset(thumbX.coerceIn(radius, w - radius), size.height / 2f)
    drawCircle(PleyColors.Fg, radius, centre)
    if (focus > 0f) {
        drawCircle(
            PleyColors.AccentBright.copy(alpha = focus),
            radius + PleyFocus.RingOffset.toPx(),
            centre,
            style = Stroke(PleyFocus.RingStroke.toPx()),
        )
    }
}

/** "Intro" or "Credits" under the span of the marker playing, in caption muted. */
@Composable
private fun MarkerLabel(state: State<PlayerUiState>) {
    val marker by remember(state) { derivedStateOf { Markers.active(state.value.markers, state.value.positionMs) } }
    val m = marker ?: return
    val label = stringResource(if (m.kind == MarkerKind.Intro) R.string.tv_player_intro else R.string.tv_player_credits)
    BasicText(
        label,
        style = PleyType.Tv.Caption.copy(color = PleyColors.Muted, textAlign = TextAlign.Center),
        maxLines = 1,
        modifier = Modifier
            .padding(top = TRACK_CENTRE - TIMES_TOP + 6.dp)
            .fillMaxWidth()
            .layout { measurable, constraints ->
                val placeable = measurable.measure(constraints.copy(minWidth = 0))
                val d = state.value.durationMs.toFloat().coerceAtLeast(1f)
                val mid = ((m.startMs + m.endMs) / 2f / d).coerceIn(0f, 1f) * constraints.maxWidth
                val x = (mid - placeable.width / 2f).toInt().coerceIn(0, (constraints.maxWidth - placeable.width).coerceAtLeast(0))
                layout(constraints.maxWidth, placeable.height) { placeable.place(x, 0) }
            },
    )
}

/**
 * The seek preview while scrubbing (design spec §2.7): 240 × 135 with a
 * 2 dp fg 20 % edge, its bottom at y = 319, centred on the thumb but kept
 * in the safe area; the target time under it and the chapter's name. The
 * server's seek previews when the file has them, else the chapter's
 * picture; without either, only the time and chapter show.
 */
@Composable
internal fun BoxScope.TvSeekPreview(state: State<PlayerUiState>, seeker: TvSeeker, controller: PlaybackController) {
    val target = seeker.target ?: return
    val hasThumbs by remember(state) { derivedStateOf { state.value.hasThumbnails } }
    val chapters by remember(state) { derivedStateOf { state.value.chapters } }
    // Previews are asked for every 2 s of the target at most.
    val bucket = target / PREVIEW_STEP_MS * PREVIEW_STEP_MS
    val chapterIndex = TvPlayerText.chapterAt(chapters, target)
    val chapter = chapterIndex?.let { chapters[it] }
    val picture by produceState<ImageBitmap?>(null, bucket, hasThumbs) {
        if (hasThumbs) controller.thumbnail(bucket)?.let { value = it.asImageBitmap() }
    }
    val chapterImage = chapter?.imageUrl?.takeIf { !hasThumbs }
    val showsPicture = hasThumbs || chapterImage != null
    Column(
        Modifier
            .align(Alignment.TopStart)
            .padding(top = if (showsPicture) PREVIEW_TOP else PREVIEW_TOP + PREVIEW_HEIGHT + 6.dp)
            .fillMaxWidth()
            .padding(horizontal = TvSafeArea.Horizontal)
            .layout { measurable, constraints ->
                val placeable = measurable.measure(constraints.copy(minWidth = 0))
                val d = state.value.durationMs.toFloat().coerceAtLeast(1f)
                val t = (seeker.target ?: target).toFloat()
                val centre = (t / d).coerceIn(0f, 1f) * constraints.maxWidth
                val x = (centre - placeable.width / 2f).toInt().coerceIn(0, (constraints.maxWidth - placeable.width).coerceAtLeast(0))
                layout(constraints.maxWidth, placeable.height) { placeable.place(x, 0) }
            }
            .testTag(TAG_PREVIEW),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (showsPicture) {
            Box(
                Modifier
                    .size(PREVIEW_WIDTH, PREVIEW_HEIGHT)
                    .clip(PleyShape.TileTv)
                    .drawBehind { drawRect(PleyColors.Surface2) }
                    .border(2.dp, PleyColors.Fg.copy(alpha = 0.2f), PleyShape.TileTv),
            ) {
                val bitmap = picture
                when {
                    bitmap != null -> Image(bitmap, contentDescription = null, modifier = Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
                    chapterImage != null -> Artwork(ArtworkImage(listOf(chapterImage), maxWidth = PREVIEW_IMAGE_WIDTH), title = "", modifier = Modifier.fillMaxSize())
                }
            }
        }
        Spacer(Modifier.height(6.dp))
        BasicText(TvPlayerText.time(target), style = PleyType.Tv.Label.copy(color = PleyColors.Fg))
        if (chapterIndex != null) {
            val name = chapter?.title?.takeIf { it.isNotBlank() }
            val text = when {
                name == null -> stringResource(R.string.tv_player_chapter, chapterIndex + 1)
                name.startsWith("Chapter", ignoreCase = true) -> name
                else -> stringResource(R.string.tv_player_chapter_named, chapterIndex + 1, name)
            }
            BasicText(text, style = PleyType.Tv.Caption.copy(color = PleyColors.Muted), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** What the control row offers beyond playing and seeking. */
internal data class RowOptions(
    val previousNext: Boolean,
    val subtitles: Boolean,
    val audio: Boolean,
    val quality: Boolean,
    val chapters: Boolean,
)

/**
 * The control row, top y = 413 (design spec §2.7): Previous · −10 s ·
 * Play/Pause (64 dp) · +10 s · Next centred; Subtitles · Audio · Quality ·
 * Info at the right safe edge; Chapters, when the file has them, at the
 * left one. Up goes back to the scrubber.
 */
@Composable
internal fun BoxScope.TvControlRow(
    playing: Boolean,
    options: RowOptions,
    ui: TvPlayerUi,
    alpha: State<Float>,
    onPlayPause: () -> Unit,
    onBack10: () -> Unit,
    onForward10: () -> Unit,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
) {
    val up = Modifier.focusProperties {
        this.up = ui.requester(PlayerFocus.Scrubber)
        down = FocusRequester.Cancel
    }
    Box(
        Modifier
            .align(Alignment.TopStart)
            .padding(top = ROW_TOP)
            .fillMaxWidth()
            .padding(horizontal = TvSafeArea.Horizontal)
            .graphicsLayer { this.alpha = alpha.value },
    ) {
        if (options.chapters) {
            PlayerRoundButton(
                stringResource(R.string.tv_player_chapters), PleyIcons.ListVideo, { ui.openPanel(PlayerPanel.Chapters) },
                Modifier.align(Alignment.TopStart).padding(top = 8.dp).then(up),
                ui, PlayerFocus.Chapters,
            )
        }
        Row(
            Modifier.align(Alignment.TopCenter),
            horizontalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            if (options.previousNext) {
                PlayerRoundButton(stringResource(R.string.tv_player_previous), PleyIcons.SkipBack, onPrevious, Modifier.padding(top = 8.dp).then(up), ui, null)
            }
            PlayerRoundButton(stringResource(R.string.tv_player_back_10), PleyIcons.RotateCcw, onBack10, Modifier.padding(top = 8.dp).then(up), ui, null)
            PlayerRoundButton(
                stringResource(if (playing) R.string.tv_player_pause else R.string.tv_player_play),
                if (playing) PleyIcons.Pause else PleyIcons.Play,
                onPlayPause,
                up,
                ui,
                PlayerFocus.PlayPause,
                size = 64.dp,
                iconSize = 32.dp,
            )
            PlayerRoundButton(stringResource(R.string.tv_player_forward_10), PleyIcons.RotateCw, onForward10, Modifier.padding(top = 8.dp).then(up), ui, null)
            if (options.previousNext) {
                PlayerRoundButton(stringResource(R.string.tv_player_next), PleyIcons.SkipForward, onNext, Modifier.padding(top = 8.dp).then(up), ui, null)
            }
        }
        Row(
            Modifier.align(Alignment.TopEnd).padding(top = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(PleyTv.ButtonGap),
        ) {
            if (options.subtitles) {
                PlayerRoundButton(stringResource(R.string.tv_player_subtitles), PleyIcons.Captions, { ui.openPanel(PlayerPanel.Subtitles) }, up, ui, PlayerFocus.Subtitles)
            }
            if (options.audio) {
                PlayerRoundButton(stringResource(R.string.tv_player_audio), PleyIcons.AudioLines, { ui.openPanel(PlayerPanel.Audio) }, up, ui, PlayerFocus.Audio)
            }
            if (options.quality) {
                PlayerRoundButton(stringResource(R.string.tv_player_quality), PleyIcons.Gauge, { ui.openPanel(PlayerPanel.Quality) }, up, ui, PlayerFocus.Quality)
            }
            PlayerRoundButton(stringResource(R.string.tv_player_info), PleyIcons.Info, { ui.openPanel(PlayerPanel.Info) }, up, ui, PlayerFocus.Info)
        }
    }
}

/**
 * A round button of the player (design spec §2.5): `bg` 80 % over the
 * video, accent when focused and 1.08 larger, its label fading in 8 dp
 * below while focused (always there for screen readers). [target] names it
 * for focus requests.
 */
@Composable
private fun PlayerRoundButton(
    label: String,
    icon: PleyIcon,
    onClick: () -> Unit,
    modifier: Modifier,
    ui: TvPlayerUi,
    target: PlayerFocus?,
    size: Dp = 48.dp,
    iconSize: Dp = 24.dp,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    val labelAlpha by animateFloatAsState(if (focused) 1f else 0f, PleyMotion.reveal(), label = "label")
    Box(modifier.width(size), contentAlignment = Alignment.TopCenter) {
        IconButton(
            onClick = onClick,
            interactionSource = interactions,
            modifier = Modifier
                .size(size)
                .then(if (target != null) Modifier.focusRequester(ui.requester(target)).testTag(tagOf(target)) else Modifier)
                .onFocusChanged { state ->
                    if (target != null) {
                        if (state.isFocused) ui.focused = target else if (ui.focused == target) ui.focused = null
                    } else if (state.isFocused) {
                        ui.focused = null
                    }
                }
                .semantics { contentDescription = label },
            shape = IconButtonDefaults.shape(PleyShape.Circle),
            colors = IconButtonDefaults.colors(
                containerColor = PleyColors.OnArt,
                contentColor = PleyColors.Fg,
                focusedContainerColor = PleyColors.Accent,
                focusedContentColor = PleyColors.OnAccent,
                pressedContainerColor = PleyColors.AccentBright,
                pressedContentColor = PleyColors.OnAccent,
            ),
            scale = IconButtonDefaults.scale(focusedScale = PleyFocus.ROUND_SCALE, pressedScale = 1f),
            glow = IconButtonDefaults.glow(Glow.None, Glow.None, Glow.None),
            border = IconButtonDefaults.border(Border.None, Border.None, Border.None, Border.None, Border.None),
        ) {
            Icon(icon, contentDescription = null, size = iconSize, tint = androidx.tv.material3.LocalContentColor.current)
        }
        BasicText(
            label,
            style = PleyType.Tv.Caption.copy(color = PleyColors.Fg, textAlign = TextAlign.Center),
            maxLines = 1,
            softWrap = false,
            modifier = Modifier
                .padding(top = size + 8.dp)
                .graphicsLayer { alpha = labelAlpha }
                .layout { measurable, constraints ->
                    val placeable = measurable.measure(constraints.copy(minWidth = 0, maxWidth = Constraints.Infinity))
                    val width = constraints.constrainWidth(placeable.width)
                    layout(width, placeable.height) { placeable.place((width - placeable.width) / 2, 0) }
                },
        )
    }
}

/**
 * The scrims behind the controls (design spec §2.7, black as the player's
 * exception): 120 dp at the top, black 60 % to 0 %, and 200 dp at the
 * bottom, 0 % to 85 %. They fade by their brush's alpha, read while
 * drawing: no layer over the video.
 */
internal fun Modifier.controlScrims(alpha: State<Float>): Modifier = drawBehind {
    val a = alpha.value
    if (a <= 0f) return@drawBehind
    val top = TOP_SCRIM.toPx()
    val bottom = BOTTOM_SCRIM.toPx()
    drawRect(
        androidx.compose.ui.graphics.Brush.verticalGradient(0f to Color.Black.copy(alpha = 0.6f * a), 1f to Color.Transparent, startY = 0f, endY = top),
        size = Size(size.width, top),
    )
    drawRect(
        androidx.compose.ui.graphics.Brush.verticalGradient(0f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.85f * a), startY = size.height - bottom, endY = size.height),
        topLeft = Offset(0f, size.height - bottom),
        size = Size(size.width, bottom),
    )
}

/** Whether a phase counts as playing for Play/Pause: it will play once ready. */
internal fun PlayerPhase.wantsToPlay(): Boolean = this == PlayerPhase.Playing || this == PlayerPhase.Buffering || this == PlayerPhase.Preparing || this == PlayerPhase.Switching

private const val MINUTE_MS = 60_000L
private const val PREVIEW_STEP_MS = 2_000L
private const val PREVIEW_IMAGE_WIDTH = 480

private val TRACK = PleyColors.Fg.copy(alpha = 0.25f)
private val BUFFERED = PleyColors.Fg.copy(alpha = 0.45f)

private val TRACK_HEIGHT = 4.dp
private val CHAPTER_GAP = 2.dp
private val THUMB = 14.dp
private val THUMB_FOCUSED = 20.dp
private val SCRUB_HEIGHT = 40.dp
private val TOP_SCRIM = 120.dp
private val BOTTOM_SCRIM = 200.dp

/** Where things sit on the 540 dp canvas (design spec §2.7). */
internal val TIMES_TOP = 371.dp
internal val TRACK_CENTRE = 397.dp
internal val ROW_TOP = 413.dp
internal val PREVIEW_TOP = 184.dp
internal val PREVIEW_WIDTH = 240.dp
internal val PREVIEW_HEIGHT = 135.dp

/** A control row button's test tag. */
internal fun tagOf(target: PlayerFocus): String = when (target) {
    PlayerFocus.PlayPause -> TAG_PLAY_PAUSE
    PlayerFocus.Subtitles -> TAG_SUBTITLES
    PlayerFocus.Info -> TAG_INFO
    else -> "pley.tv.player." + target.name.lowercase()
}

internal const val TAG_SCRUBBER = "pley.tv.player.scrubber"
internal const val TAG_PREVIEW = "pley.tv.player.preview"
internal const val TAG_PLAY_PAUSE = "pley.tv.player.playPause"
internal const val TAG_SUBTITLES = "pley.tv.player.subtitles"
internal const val TAG_INFO = "pley.tv.player.info"
