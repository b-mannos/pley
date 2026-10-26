package app.pley.android.player.session

import android.graphics.Bitmap
import androidx.media3.common.text.Cue
import app.pley.android.caps.CapsSnapshot
import app.pley.android.common.describe
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.Marker
import app.pley.android.model.MarkerKind
import app.pley.android.model.MediaFile
import app.pley.android.model.MediaStream
import app.pley.android.model.PlaybackMethod
import app.pley.android.model.PlaybackMode
import app.pley.android.model.ProgressState
import app.pley.android.model.ServerEvent
import app.pley.android.model.StreamChoice
import app.pley.android.model.TranscodeInfo
import app.pley.android.model.format.episodeLabel
import app.pley.android.model.format.formatBitrate
import app.pley.android.model.format.resolutionLabel
import app.pley.android.model.format.streamLabel
import app.pley.android.network.ApiException
import app.pley.android.player.ChapterMark
import app.pley.android.player.EngineSource
import app.pley.android.player.PlayRequest
import app.pley.android.player.PlaybackController
import app.pley.android.player.PlayerEngine
import app.pley.android.player.PlayerPhase
import app.pley.android.player.PlayerUiState
import app.pley.android.player.QualityChoice
import app.pley.android.player.TrackChoice
import app.pley.android.player.UpNextUi
import app.pley.android.player.markers.CreditsGate
import app.pley.android.player.markers.Markers
import app.pley.android.player.queue.NextUp
import app.pley.android.player.sleep.SleepChoice
import app.pley.android.player.sleep.SleepTimer
import app.pley.android.player.subtitles.PictureSubtitles
import app.pley.android.player.subtitles.PictureTrack
import app.pley.android.player.subtitles.ServerSubtitles
import app.pley.android.player.subtitles.SubtitleSync
import app.pley.android.player.subtitles.TimedCueTrack
import app.pley.android.player.trickplay.TrickplayThumbnails
import java.io.IOException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The playback of one video, then of what plays after it, on one engine:
 * [PlaybackController] as the web player plays (apps/web/src/player/VideoPlayer.tsx).
 *
 * - **Start.** [start] runs at tap time: the item's details, the device's
 *   capabilities and the engine come together, then POST /api/playback
 *   ([PlaybackPlan]), the Dolby Vision guards ([DvGuards]), and the engine
 *   loads the answer.
 * - **Reports.** Progress every 10 s while playing and every 30 s paused,
 *   on pause, and 1 s after the last seek; `stopped` then DELETE at the end,
 *   on closing and before any new session, through the app-scoped
 *   [SessionReporter]. One server session at a time.
 * - **Recovery.** [Recovery]'s ladder for errors, 12 s stalls and dropped frames.
 * - **Tracks.** Switched by the engine in direct play; a new session when a
 *   stream needs other audio or subtitles burned in; choices are saved
 *   with PUT /api/items/{id}/streams.
 * - **Markers and Up Next.** Skip Intro and Skip Credits; entering credits,
 *   or the end, brings up what plays next with a 10 s countdown.
 * - **Subtitles.** The server's WebVTT for streams and files beside the
 *   media, and pictures side-loaded beside a stream (its .mks), drawn at
 *   the playhead moved by the subtitle offset; a file's own text tracks
 *   move through the server's WebVTT too while an offset is set.
 * - **Sleep timer**, and a session the server stopped: it says so and
 *   plays no more.
 *
 * Runs on [scope]'s dispatcher (the main thread); the engine's events come there too.
 */
class SessionPlaybackController(
    private val request: PlayRequest,
    private val env: PlaybackEnvironment,
    private val scope: CoroutineScope,
    private val onClosed: () -> Unit = {},
) : PlaybackController, EngineLink.Listener {
    private val ui = MutableStateFlow(PlayerUiState())
    override val state: StateFlow<PlayerUiState> = ui.asStateFlow()

    private val cueState = MutableStateFlow<List<Cue>>(emptyList())
    override val cues: StateFlow<List<Cue>> = cueState.asStateFlow()

    private val texts get() = env.texts
    private var link: EngineLink? = null
    override val engine: PlayerEngine? get() = link?.engine

    // ---------------------------------------------------------------- what plays

    private var current: PlayRequest = request
    private var item: ItemDetail? = null
    private var caps: CapsSnapshot? = null
    private var file: MediaFile? = null
    private var audio: MediaStream? = null
    private var subtitle: MediaStream? = null
    private var quality: QualityPreset = Qualities.ORIGINAL
    private var markers: List<Marker> = emptyList()

    /** The mode a Dolby Vision guard asked for, for this item's later sessions too. */
    private var guardMode: PlaybackMode? = null

    // ---------------------------------------------------------------- the session

    private val sessionState = MutableStateFlow<ServerPlaybackSession?>(null)
    private val session: ServerPlaybackSession? get() = sessionState.value
    private var sessionJob: Job? = null
    private var startJob: Job? = null

    /** The playhead as last known: kept while a session (re)starts and the engine sits at 0. */
    private var lastPositionMs = 0L

    /** Where playback last started or was sought to: an end that comes right after it can be media that falls short. */
    private var startedAtMs = 0L

    /** The engine shows this session's media: only then is its position the playhead. */
    private var mediaShown = false
    private val wantPlay = MutableStateFlow(true)
    private var ended = false
    private var closed = false
    private var tracksChecked = false

    private val recovery = Recovery()
    private val struggle = StruggleMeter()
    private val credits = CreditsGate()
    private val nextUp = NextUp(env.api)
    private val serverSubtitles = ServerSubtitles(scope, env.subtitles, env.parser, env.work, onError = { notice(texts.subtitlesFailed) })
    private val pictureSubtitles = env.pictures?.let { PictureSubtitles(scope, it, onError = ::picturesFailed) }

    private val offsetWrites = Mutex()

    /** The server has no .mks of picture subtitles (an older server, or its transcoder): they are burned in. */
    private var picturesUnavailable = false

    /** The subtitle offset of the file playing: positive shows them later. */
    private val offset = MutableStateFlow(0L)

    /** The server stopped the playback: nothing plays again. */
    private var stopped = false
    private val sleep = SleepTimer(scope, env.now, ::itemLeftMs, { item?.id }, ::onSleep)
    private var trickplay: TrickplayThumbnails? = null
    private var trickplayWatch: Job? = null

    private var lastAdvanceAt = 0L
    private var lastAdvancePos = -1L
    private var lastStruggleSample = 0L
    private var seekReport: Job? = null

    // ---------------------------------------------------------------- Up Next

    private var next: ItemSummary? = null
    private var nextDetail: Pair<Long, ItemDetail>? = null
    private var countdown: Job? = null
    private var nextLookup: Job? = null

    // ---------------------------------------------------------------- details

    private var detailsJob: Job? = null
    private var live: TranscodeInfo? = null

    /** Starts preparing: called once, when someone presses play. */
    fun start() {
        startJob = scope.launch { prepareFirst() }
        scope.launch { ticker() }
        scope.launch { progressReports() }
        scope.launch { subtitleCues() }
        scope.launch { serverStops() }
        scope.launch { sleep.state.collect { s -> ui.update { it.copy(sleep = s) } } }
    }

    private suspend fun prepareFirst() {
        try {
            val detail = scope.async { env.details.detail(current.itemId) }
            val snapshot = scope.async {
                try {
                    env.capabilities.snapshot.value ?: env.capabilities.warmUp()
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    null
                }
            }
            if (link == null) link = env.engines().also { it.setListener(this) }
            val found = detail.await()
            caps = snapshot.await()
            load(found, current)
            begin(PlaybackPlan.startMs(found, current))
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            failWith(e)
        }
    }

    /** Takes [detail] as the item playing, with [req]'s choices. */
    private suspend fun load(detail: ItemDetail, req: PlayRequest) {
        item = detail
        current = req
        val chosen = PlaybackPlan.file(detail, req.fileId, caps, env.policy)
        file = chosen
        audio = PlaybackPlan.initialAudio(chosen, req.audioIndex)
        subtitle = PlaybackPlan.initialSubtitle(chosen, req.subtitleIndex)
        val remembered = try {
            env.quality.remembered()
        } catch (e: CancellationException) {
            throw e
        } catch (_: Exception) {
            null
        }
        quality = Qualities.choose(chosen, remembered)
        offset.value = chosen?.let { f ->
            try {
                env.offsets.offset(offsetKey(f.id))
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
                0L
            }
        } ?: 0L
        markers = Markers.forFile(detail, chosen?.id)
        val episode = detail.kind == ItemKind.Episode
        ui.update {
            it.copy(
                itemId = detail.id,
                title = if (episode) detail.grandparentTitle ?: detail.title else if (detail.year != null) "${detail.title} (${detail.year})" else detail.title,
                subtitle = if (episode) listOf(episodeLabel(detail.parentIndex, detail.index), detail.title).filter { s -> s.isNotEmpty() }.joinToString(" · ") else null,
                durationMs = chosen?.durationMs ?: detail.durationMs ?: 0,
                markers = markers,
                chapters = chosen?.chapters.orEmpty().map { c -> ChapterMark(c.title, c.startMs, c.endMs, c.thumbUrl?.let { u -> env.api.url(u) }) },
                hasThumbnails = false,
            )
        }
        publishChoices()
        dropTrickplay()
        val sheets = env.sheets
        if (chosen != null && sheets != null) {
            val thumbs = TrickplayThumbnails(scope, env.api, chosen.id, sheets, env.sheetDecoder, env.lowMemory, env.work)
            trickplay = thumbs
            thumbs.start()
            trickplayWatch = scope.launch { thumbs.info.collect { info -> ui.update { it.copy(hasThumbnails = info != null) } } }
        }
    }

    // ---------------------------------------------------------------- sessions

    /**
     * Starts a server session at [atMs] and loads it, ending the one before.
     * The previous run of this is cancelled (a new choice wins).
     */
    private fun restart(atMs: Long) {
        if (stopped) return
        sessionJob?.cancel()
        startJob?.cancel()
        sessionJob = scope.launch {
            try {
                begin(atMs)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failWith(e)
            }
        }
    }

    private suspend fun begin(atMs: Long) {
        val detail = item ?: return
        val chosen = file ?: return fail(texts.noFile, canRetry = false)
        // The session before ends first ("stopped", then DELETE), unless the server is slow to say so.
        if (endSession(atMs)) withTimeoutOrNull(END_WAIT_MS) { env.reporter.flush() }
        lastPositionMs = atMs
        startedAtMs = atMs
        mediaShown = false
        tracksChecked = false
        ended = false
        struggle.reset()
        recovery.onJump()
        lastAdvanceAt = env.now()
        lastAdvancePos = -1
        ui.update { it.copy(phase = PlayerPhase.Preparing, positionMs = atMs, skipLabel = null) }

        val mode = recovery.fallbackMode ?: guardMode?.takeIf { quality.original } ?: quality.mode
        val decision = caps?.let { c -> runCatching { env.policy.decide(detail, chosen.id, c) }.getOrNull() }
        var choices = PlaybackPlan.Choices(
            file = chosen,
            startMs = atMs,
            audio = audio,
            subtitle = subtitle,
            quality = quality,
            mode = mode,
            streaming = PlaybackPlan.streams(mode, decision, chosen, audio, caps),
            sideload = sideload,
        )
        var sent = PlaybackPlan.request(detail.id, choices, env.clientId(), caps)
        var started = ServerPlaybackSession.start(env.api, sent, env.reporter)

        DvGuards.retryMode(started.info, chosen, caps)?.let { guarded ->
            started.discard()
            guardMode = guarded
            choices = choices.copy(mode = guarded, streaming = true)
            sent = PlaybackPlan.request(detail.id, choices, env.clientId(), caps)
            started = ServerPlaybackSession.start(env.api, sent, env.reporter)
        }
        if (PlaybackPlan.needsIndexes(started.info, sent, choices)) {
            started.discard()
            choices = choices.copy(streaming = true)
            sent = PlaybackPlan.request(detail.id, choices, env.clientId(), caps)
            started = ServerPlaybackSession.start(env.api, sent, env.reporter)
        }
        sessionState.value = started
        live = null
        val info = started.info
        ui.update {
            it.copy(
                method = texts.method(info),
                note = recovery.fallbackReason?.let(texts::fallback),
                canPlayOriginal = recovery.struggleSwitched && recovery.fallbackMode != null,
                durationMs = info.durationMs.takeIf { d -> d > 0 } ?: it.durationMs,
            )
        }
        publishChoices()
        refreshDetails()
        val engineLink = link ?: return
        // The video as it arrives, so a TV matches its display mode before the
        // first frame: the file's stream when it is copied; a transcode keeps
        // the frame rate, but its dynamic range is the server's to choose.
        val video = PlaybackPlan.videoStream(chosen)?.let { v ->
            if (info.method == PlaybackMethod.Transcode) v.copy(dynamicRange = null, dvProfile = null, dvBlCompat = null) else v
        }
        val source = EngineSource(
            url = env.api.url(info.url),
            hls = info.hls,
            mimeType = if (info.hls) null else PlaybackPlan.mimeTypeOf(chosen.container),
            video = video,
            durationMs = info.durationMs.takeIf { it > 0 } ?: chosen.durationMs ?: detail.durationMs,
        )
        engineLink.prepare(source, atMs, wantPlay.value)
        applyTracks()
    }

    /** Reports `stopped` at [positionMs] and deletes the session playing; false when none plays. */
    private fun endSession(positionMs: Long): Boolean {
        val s = session ?: return false
        sessionState.value = null
        s.end(positionMs)
        return true
    }

    /**
     * The tracks chosen, where they play: the engine in direct play, the
     * server's WebVTT for streams, files beside the media and text moved by
     * an offset, the server's .mks for pictures beside a stream.
     */
    private fun applyTracks() {
        val info = session?.info ?: return
        val engineLink = link ?: return
        val chosenFile = file
        val sub = subtitle
        if (!info.hls) {
            audio?.let { a -> engineLink.selectAudio(PlaybackPlan.audioStreams(chosenFile).indexOf(a), a.language) }
            when {
                sub == null -> {
                    engineLink.selectText(null, null)
                    hideServerSubtitles()
                }
                sub.external || (offset.value != 0L && !PlaybackPlan.isImage(sub)) -> {
                    engineLink.selectText(null, null)
                    showServerSubtitles(sub)
                }
                else -> {
                    val embedded = PlaybackPlan.subtitleStreams(chosenFile).filter { !it.external }
                    engineLink.selectText(embedded.indexOf(sub), sub.language)
                    hideServerSubtitles()
                }
            }
        } else {
            engineLink.selectText(null, null)
            when {
                sub == null -> hideServerSubtitles()
                !PlaybackPlan.isImage(sub) -> showServerSubtitles(sub)
                !info.burnSubtitles && sideload -> showPictures(sub)
                else -> hideServerSubtitles()
            }
        }
    }

    private fun showServerSubtitles(sub: MediaStream) {
        val info = session?.info ?: return
        pictureSubtitles?.hide()
        val path = info.subtitles.firstOrNull { it.index == sub.index }?.url ?: "/api/files/${info.fileId}/subtitles/${sub.index}.vtt"
        serverSubtitles.show(info.fileId, sub.index, env.api.url(path))
    }

    /** Pictures beside a stream: the server's .mks of the track, at the stream's own positions (the file's). */
    private fun showPictures(sub: MediaStream) {
        val info = session?.info ?: return
        val pictures = pictureSubtitles ?: return
        serverSubtitles.hide()
        pictures.show(info.fileId, sub.index, env.api.url(PlaybackPlan.pictureSubtitlesPath(info.fileId, sub.index)))
    }

    private fun hideServerSubtitles() {
        serverSubtitles.hide()
        pictureSubtitles?.hide()
    }

    /** Pictures chosen for a stream are drawn by this player, not burned in. */
    private val sideload: Boolean get() = !picturesUnavailable && PlaybackPlan.sideloadsPictures(caps, pictureSubtitles != null)

    /**
     * The pictures beside the stream couldn't be loaded. When the server
     * can't make them at all (404: a server without the route; 503: its
     * transcoder can't), the stream starts again with them burned in, as
     * before side-loading; otherwise the screen says so.
     */
    private fun picturesFailed(error: Throwable) {
        val status = (error as? ApiException)?.status
        val info = session?.info
        if ((status == 404 || status == 503) && !closed && !stopped && info != null && info.hls && !info.burnSubtitles) {
            picturesUnavailable = true
            if (PlaybackPlan.isImage(subtitle)) {
                publishChoices()
                restart(livePosition())
                return
            }
        }
        notice(texts.subtitlesFailed)
    }

    /** Where a file's subtitle offset is kept: the file's own address, so files of different servers don't mix. */
    private fun offsetKey(fileId: Long): String = env.api.url("/api/files/$fileId")

    // ---------------------------------------------------------------- engine events

    override fun onStatus(status: EngineStatus) {
        if (session == null || closed) return
        when (status) {
            EngineStatus.Buffering -> if (!ended) phase(PlayerPhase.Buffering)
            EngineStatus.Ready -> {
                mediaShown = true
                phase(if (link?.playWhenReady == true) PlayerPhase.Playing else PlayerPhase.Paused)
            }
            EngineStatus.Ended -> if (endedShort()) onEndedShort() else onEnded()
            EngineStatus.Idle -> Unit
        }
    }

    override fun onPlayWhenReady(playWhenReady: Boolean) {
        if (closed) return
        val was = wantPlay.value
        wantPlay.value = playWhenReady
        if (playWhenReady) sleep.resumed()
        if (session == null) return
        if (!playWhenReady) {
            struggle.reset()
            if (was && !ended) report(ProgressState.Paused)
            if (link?.status == EngineStatus.Ready) phase(PlayerPhase.Paused)
        } else if (link?.status == EngineStatus.Ready) {
            phase(PlayerPhase.Playing)
        }
    }

    override fun onPlaying(playing: Boolean) {
        if (playing && session != null) phase(PlayerPhase.Playing)
    }

    /** A video or an audio track that no decoder here takes: the server has to help, as for a decoder error. */
    override fun onTracks(tracks: EngineTracks) {
        if (tracksChecked || session == null) return
        tracksChecked = true
        val videoUnplayable = tracks.video.isNotEmpty() && tracks.video.none { it.supported }
        val audioUnplayable = tracks.audio.isNotEmpty() && tracks.audio.none { it.supported }
        if (videoUnplayable || audioUnplayable) {
            val what = if (videoUnplayable) "video" else "audio"
            onError(EngineError.Unsupported(audio = !videoUnplayable, container = false, detail = "no $what track plays here"))
        }
    }

    override fun onError(error: EngineError) {
        val s = session ?: return
        if (closed) return
        if (error is EngineError.Stopped) {
            // A stream answers 410 once the server stopped it, also to players that didn't hear the event.
            stopByServer(error.message, byAdmin = true)
            return
        }
        val videoCodec = PlaybackPlan.videoStream(file)?.codec
        act(recovery.decide(error, s.info.method, videoCodec), starving = false)
    }

    private fun act(action: Recovery.Action, starving: Boolean) {
        when (action) {
            is Recovery.Action.NewSession -> {
                if (action.reason == FallbackReason.NotSmooth && recovery.struggleSwitched) {
                    ui.update { it.copy(canPlayOriginal = true) }
                }
                restart(livePosition())
            }
            Recovery.Action.CheckServer -> {
                val s = session ?: return
                scope.launch {
                    val speed = try {
                        env.api.playbackStatus(s.id).transcode?.speed
                    } catch (e: CancellationException) {
                        throw e
                    } catch (_: IOException) {
                        null
                    }
                    if (session === s) act(recovery.afterServerCheck(speed, starving), starving)
                }
            }
            Recovery.Action.SuggestLowerQuality -> ui.update { it.copy(suggestLowerQuality = true, note = texts.slowServer) }
            is Recovery.Action.Fail -> fail(texts.failure(action.failure, action.detail), canRetry = action.failure != Failure.Unplayable)
        }
    }

    /**
     * The media ended moments after where it started, a minute or more
     * before the end the server gives: the stream or file doesn't hold that
     * part (resuming past its real end). That isn't the end of the video.
     */
    private fun endedShort(): Boolean {
        if (ended) return false
        val engineLink = link ?: return false
        val expected = session?.info?.durationMs?.takeIf { it > 0 } ?: file?.durationMs ?: return false
        val reached = engineLink.positionMs
        return reached + SHORT_END_MS < expected && reached < startedAtMs + SHORT_PLAY_MS
    }

    /**
     * Recovers from media that fell short ([Recovery.onEndedShort]). Where
     * it was to play from stays the playhead, for the next session and the
     * reports: nothing is reported at the media's end, nothing is marked watched.
     */
    private fun onEndedShort() {
        val s = session ?: return
        mediaShown = false
        lastPositionMs = startedAtMs
        ui.update { it.copy(positionMs = startedAtMs, skipLabel = null) }
        act(recovery.onEndedShort(s.info.method), starving = false)
    }

    private fun onEnded() {
        if (ended) return
        ended = true
        // The sleep timer ends with this item: what comes next waits for the person.
        val sleeping = sleep.itemEnded(item?.id)
        val s = session
        val duration = s?.info?.durationMs?.takeIf { it > 0 } ?: link?.durationMs?.takeIf { it > 0 } ?: lastPositionMs
        lastPositionMs = duration
        endSession(duration)
        ui.update { it.copy(positionMs = duration, skipLabel = null) }
        val detail = item ?: return phase(PlayerPhase.Ended)
        if (!nextUp.hasNext(detail.kind, current.queue)) return phase(PlayerPhase.Ended)
        phase(PlayerPhase.Paused)
        val known = next
        if (known != null) {
            showUpNext(known, restartCountdown = !sleeping && countdown?.isActive != true)
            return
        }
        lookUpNext { found -> if (found == null) phase(PlayerPhase.Ended) else showUpNext(found, restartCountdown = !sleeping) }
    }

    // ---------------------------------------------------------------- the clock

    private suspend fun ticker() {
        while (true) {
            delay(TICK_MS)
            tick()
        }
    }

    private fun tick() {
        val engineLink = link ?: return
        val s = session ?: return
        if (closed) return
        val now = env.now()
        val moving = mediaShown && engineLink.status == EngineStatus.Ready && engineLink.playWhenReady && !ended
        if (mediaShown && !ended) {
            val pos = engineLink.positionMs
            lastPositionMs = pos
            if (moving) recovery.onPosition(pos)
            val marker = if (ui.value.phase is PlayerPhase.Error) null else Markers.active(markers, pos)
            ui.update {
                it.copy(
                    positionMs = pos,
                    bufferedMs = engineLink.bufferedMs,
                    durationMs = s.info.durationMs.takeIf { d -> d > 0 } ?: engineLink.durationMs.takeIf { d -> d > 0 } ?: it.durationMs,
                    skipLabel = marker?.let { m -> if (m.kind == MarkerKind.Intro) texts.skipIntro else texts.skipCredits },
                )
            }
            credits.entered(marker)?.let { onCredits() }
        }
        // A stall: meant to play, not moving (web: STALL_MS).
        val stalling = wantPlay.value && !ended && ui.value.phase !is PlayerPhase.Error && engineLink.status != EngineStatus.Idle
        val pos = if (mediaShown) engineLink.positionMs else -1
        if (!stalling || pos != lastAdvancePos) {
            lastAdvanceAt = now
            lastAdvancePos = pos
        } else if (now - lastAdvanceAt >= STALL_MS) {
            lastAdvanceAt = now
            val starving = engineLink.status == EngineStatus.Buffering && engineLink.bufferedMs - engineLink.positionMs < STARVING_MS
            act(recovery.onStall(s.info.method, starving), starving)
            return
        }
        // Dropped frames, sampled every 2 s of playback.
        if (moving && now - lastStruggleSample >= STRUGGLE_SAMPLE_MS) {
            lastStruggleSample = now
            val counts = engineLink.frameCounts()
            if (counts != null && struggle.sample(now, counts)) {
                struggle.reset()
                recovery.onStruggling(s.info.method)?.let { act(it, starving = false) }
            }
        }
    }

    /** Progress every 10 s while playing and every 30 s paused, so the server keeps the session and its transcode. */
    private suspend fun progressReports() {
        combine(sessionState, wantPlay) { s, playing -> s to playing }.collectLatest { (s, playing) ->
            if (s == null) return@collectLatest
            while (true) {
                delay(if (playing) PROGRESS_MS else PAUSED_HEARTBEAT_MS)
                if (!ended && ui.value.phase !is PlayerPhase.Error) report(if (playing) ProgressState.Playing else ProgressState.Paused)
            }
        }
    }

    /**
     * The server's subtitles showing at the playhead moved by the offset
     * (text, or pictures beside a stream), worked out when what shows changes.
     */
    private suspend fun subtitleCues() {
        val pictures = pictureSubtitles?.track ?: MutableStateFlow<PictureTrack?>(null)
        combine(serverSubtitles.index, pictures, offset) { text, picture, shift -> Pair<TimedCueTrack?, Long>(text ?: picture, shift) }
            .collectLatest { (track, shift) ->
                if (track == null) {
                    cueState.value = emptyList()
                    return@collectLatest
                }
                while (true) {
                    val pos = livePosition()
                    val showing = track.cuesAt(SubtitleSync.trackTime(pos, shift))
                    if (showing != cueState.value) cueState.value = showing
                    val change = SubtitleSync.untilChange(track, pos, shift)
                    delay(change?.coerceIn(CUE_MIN_MS, CUE_MAX_MS) ?: CUE_IDLE_MS)
                }
            }
    }

    /** The server's word that it stopped this playback's session (an administrator, or the profile elsewhere). */
    private suspend fun serverStops() {
        env.events.collect { event ->
            if (event is ServerEvent.SessionStopped && event.sessionId == session?.id) stopByServer(event.message, event.byAdmin)
        }
    }

    /**
     * The server ended the session: the engine stops, nothing more is asked
     * of the server (only `stopped`, for the resume point), and the screen
     * says why, without Try again, then leaves.
     */
    private fun stopByServer(message: String?, byAdmin: Boolean) {
        if (closed || stopped) return
        stopped = true
        val position = livePosition()
        lastPositionMs = position
        session?.let { s ->
            sessionState.value = null
            s.stoppedByServer(position)
        }
        sessionJob?.cancel()
        startJob?.cancel()
        seekReport?.cancel()
        nextLookup?.cancel()
        hideUpNext()
        hideServerSubtitles()
        sleep.cancel()
        link?.stop()
        ui.update {
            it.copy(
                phase = PlayerPhase.Error(texts.stopped(message, byAdmin), canRetry = false),
                skipLabel = null,
                stoppedByServer = true,
            )
        }
    }

    /** How long the item playing has left, for the sleep timer; `null` while unknown. */
    private fun itemLeftMs(): Long? {
        if (session == null || !mediaShown || ended) return null
        val duration = ui.value.durationMs.takeIf { it > 0 } ?: return null
        return duration - livePosition()
    }

    /** The sleep timer ended: it pauses, and what plays next doesn't start by itself. */
    private fun onSleep() {
        countdown?.cancel()
        countdown = null
        ui.update { it.copy(upNextSecondsLeft = null, upNextCard = it.upNextCard?.copy(secondsLeft = null)) }
        link?.pause()
    }

    private fun report(state: ProgressState) {
        session?.progress(livePosition(), state)
    }

    private fun livePosition(): Long {
        val engineLink = link
        return if (mediaShown && engineLink != null && session != null && !ended) engineLink.positionMs else lastPositionMs
    }

    // ---------------------------------------------------------------- the screen's commands

    override fun play() {
        if (closed || stopped) return
        if (ended) {
            restart(0)
            return
        }
        link?.play()
    }

    override fun pause() {
        if (closed) return
        link?.pause()
    }

    override fun seekTo(positionMs: Long) {
        val engineLink = link ?: return
        if (closed || stopped) return
        val duration = ui.value.durationMs
        val target = if (duration > 0) positionMs.coerceIn(0, (duration - SEEK_END_MARGIN_MS).coerceAtLeast(0)) else positionMs.coerceAtLeast(0)
        hideUpNext()
        if (ended || session == null) {
            lastPositionMs = target
            ui.update { it.copy(positionMs = target) }
            restart(target)
            return
        }
        engineLink.seekTo(target)
        lastPositionMs = target
        startedAtMs = target
        recovery.onJump()
        struggle.reset()
        lastAdvanceAt = env.now()
        ui.update { it.copy(positionMs = target) }
        seekReport?.cancel()
        seekReport = scope.launch {
            delay(SEEK_REPORT_MS)
            report(if (wantPlay.value) ProgressState.Playing else ProgressState.Paused)
        }
    }

    override fun selectAudio(index: Long) {
        val chosen = PlaybackPlan.audioStreams(file).firstOrNull { it.index.toLong() == index } ?: return
        if (chosen.index == audio?.index) return
        audio = chosen
        saveStreams()
        publishChoices()
        val s = session ?: return
        val engineLink = link
        if (!s.info.hls && engineLink != null && PlaybackPlan.audioPlaysHere(chosen, caps)) {
            engineLink.selectAudio(PlaybackPlan.audioStreams(file).indexOf(chosen), chosen.language)
        } else {
            restart(livePosition())
        }
    }

    override fun selectSubtitle(index: Long?) {
        val chosen = index?.let { i -> PlaybackPlan.subtitleStreams(file).firstOrNull { it.index.toLong() == i } ?: return }
        if (chosen?.index == subtitle?.index) return
        subtitle = chosen
        saveStreams()
        publishChoices()
        val info = session?.info ?: return
        val burn = info.hls && PlaybackPlan.isImage(chosen) && !sideload
        if (burn != info.burnSubtitles || (burn && info.subtitleIndex != chosen?.index)) restart(livePosition()) else applyTracks()
    }

    override fun setSubtitleOffset(offsetMs: Long) {
        if (closed) return
        val moved = SubtitleSync.clamp(offsetMs)
        if (moved == offset.value) return
        val wasZero = offset.value == 0L
        offset.value = moved
        ui.update { it.copy(subtitleOffsetMs = moved) }
        file?.id?.let { id ->
            val key = offsetKey(id)
            scope.launch {
                try {
                    // One at a time, in the order chosen: a held key's steps don't land out of order.
                    offsetWrites.withLock { env.offsets.remember(key, moved) }
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    // Not remembered: this viewing keeps it all the same.
                }
            }
        }
        // A file's own text track moves through the server's WebVTT while an offset is set.
        if (wasZero != (moved == 0L)) applyTracks()
    }

    override fun setSleep(choice: SleepChoice) {
        if (closed || stopped) return
        sleep.set(choice)
    }

    override fun keepWatching() {
        if (closed || stopped) return
        sleep.extend()
        // What plays next was held for the timer: its countdown goes on.
        val known = next
        if (known != null && ui.value.upNextCard != null && countdown?.isActive != true && !ended) showUpNext(known, restartCountdown = true)
    }

    override fun selectQuality(name: String) {
        val wanted = Qualities.byIdOrLabel(name)
        val chosen = Qualities.forFile(file).firstOrNull { it.id == wanted.id } ?: return
        if (chosen.id == quality.id) return
        quality = chosen
        recovery.qualityChanged()
        ui.update { it.copy(suggestLowerQuality = false, note = null, canPlayOriginal = false) }
        scope.launch { env.quality.remember(chosen.id) }
        publishChoices()
        if (item != null) restart(livePosition())
    }

    override fun skip() {
        val marker = Markers.active(markers, livePosition()) ?: return
        val duration = ui.value.durationMs
        seekTo(if (duration > 0) minOf(Markers.skipTarget(marker), duration) else Markers.skipTarget(marker))
    }

    override fun playNext() {
        if (closed || stopped) return
        val known = next
        if (known != null) {
            switchTo(known)
            return
        }
        lookUpNext { found ->
            if (found == null) {
                notice(if (current.queue != null) texts.lastInQueue else texts.lastEpisode)
            } else {
                switchTo(found)
            }
        }
    }

    override fun cancelUpNext() {
        hideUpNext()
        if (ended) phase(PlayerPhase.Ended)
    }

    override fun playOriginal() {
        if (recovery.fallbackMode == null) return
        recovery.playOriginal()
        ui.update { it.copy(canPlayOriginal = false, note = null) }
        restart(livePosition())
    }

    override fun setDetailsOpen(open: Boolean) {
        detailsJob?.cancel()
        detailsJob = null
        if (!open) {
            ui.update { it.copy(details = null) }
            return
        }
        detailsJob = scope.launch {
            while (true) {
                val s = session
                if (s != null && s.info.hls) {
                    live = try {
                        env.api.playbackStatus(s.id).transcode
                    } catch (e: CancellationException) {
                        throw e
                    } catch (_: IOException) {
                        live
                    }
                }
                refreshDetails(force = true)
                delay(DETAILS_MS)
            }
        }
    }

    private fun refreshDetails(force: Boolean = false) {
        if (!force && ui.value.details == null) return
        val info = session?.info ?: return
        val details = PlaybackDetails.build(
            texts = texts,
            info = info,
            file = file,
            audioIndex = audio?.index,
            note = recovery.fallbackReason?.let(texts::fallback),
            positionMs = livePosition(),
            live = live,
            engine = link?.stats().orEmpty(),
            subtitle = subtitle,
        )
        ui.update { it.copy(details = details) }
    }

    override fun dismissNotice() = ui.update { it.copy(notice = null) }

    override suspend fun thumbnail(positionMs: Long): Bitmap? = trickplay?.thumbnail(positionMs)

    override fun saveState(): SavedPlaybackState = SavedPlaybackState(
        itemId = item?.id ?: current.itemId,
        fileId = file?.id ?: current.fileId,
        positionMs = livePosition(),
        audioIndex = audio?.index?.toLong(),
        subtitleIndex = subtitle?.index?.toLong(),
        quality = quality.id,
        queue = current.queue,
    )

    override fun retry() {
        if (closed || stopped) return
        recovery.retry()
        ui.update { it.copy(phase = PlayerPhase.Preparing) }
        if (item == null) {
            startJob?.cancel()
            startJob = scope.launch { prepareFirst() }
        } else {
            restart(lastPositionMs)
        }
    }

    override fun close() {
        if (closed) return
        val position = livePosition()
        closed = true
        endSession(position)
        sleep.cancel()
        hideServerSubtitles()
        dropTrickplay()
        link?.setListener(null)
        link?.release()
        link = null
        scope.cancel()
        onClosed()
    }

    // ---------------------------------------------------------------- Up Next

    private fun onCredits() {
        val detail = item ?: return
        if (!nextUp.hasNext(detail.kind, current.queue) || ui.value.upNextCard != null) return
        lookUpNext { found -> if (found != null && !closed) showUpNext(found, restartCountdown = true) }
    }

    /** Finds what plays next (once at a time), then calls [then] with it. */
    private fun lookUpNext(then: (ItemSummary?) -> Unit) {
        val detail = item ?: return then(null)
        nextLookup?.cancel()
        nextLookup = scope.launch {
            val found = try {
                nextUp.after(detail.id, detail.kind, current.queue)
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
                null
            }
            if (found != null) next = found
            then(found)
        }
    }

    private fun showUpNext(found: ItemSummary, restartCountdown: Boolean) {
        next = found
        // The next item's details and still, ahead of time.
        if (nextDetail?.first != found.id) {
            scope.launch {
                try {
                    nextDetail = found.id to env.details.detail(found.id)
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    // Asked for again when it plays.
                }
            }
        }
        val card = UpNextUi(
            itemId = found.id,
            title = found.title,
            heading = texts.upNext(NextUp.labelOf(found)),
            imageUrl = (found.thumbUrl ?: found.backdropUrl)?.let { env.api.url(it, UP_NEXT_IMAGE_WIDTH) },
            secondsLeft = null,
        )
        ui.update { it.copy(upNextCard = card, upNext = card.title, upNextSecondsLeft = null) }
        if (!restartCountdown) return
        countdown?.cancel()
        countdown = scope.launch {
            if (!env.autoplayNext() || sleep.endsWith(item?.id)) return@launch
            for (left in COUNTDOWN_SECONDS downTo 1) {
                setSecondsLeft(left)
                delay(1_000)
            }
            setSecondsLeft(0)
            switchTo(found)
        }
    }

    private fun setSecondsLeft(left: Int) = ui.update {
        it.copy(upNextSecondsLeft = left, upNextCard = it.upNextCard?.copy(secondsLeft = left))
    }

    private fun hideUpNext() {
        countdown?.cancel()
        countdown = null
        ui.update { it.copy(upNextCard = null, upNext = null, upNextSecondsLeft = null) }
    }

    /** Plays [target] on the same engine: the session before ends, the next one starts where [target] resumes. */
    private fun switchTo(target: ItemSummary) {
        hideUpNext()
        nextLookup?.cancel()
        val position = livePosition()
        val endedBefore = endSession(position)
        next = null
        recovery.newItem()
        credits.reset()
        guardMode = null
        hideServerSubtitles()
        dropTrickplay()
        ended = false
        mediaShown = false
        val req = current.copy(
            itemId = target.id,
            fileId = null,
            startMs = NextUp.startOf(target),
            fromStart = false,
            audioIndex = null,
            subtitleIndex = null,
        )
        current = req
        ui.update {
            PlayerUiState(
                phase = PlayerPhase.Preparing,
                itemId = target.id,
                title = it.title,
                positionMs = req.startMs ?: 0,
                sleep = it.sleep,
            )
        }
        sessionJob?.cancel()
        startJob?.cancel()
        sessionJob = scope.launch {
            try {
                if (endedBefore) withTimeoutOrNull(END_WAIT_MS) { env.reporter.flush() }
                val prefetched = nextDetail?.takeIf { it.first == target.id }?.second
                val detail = prefetched ?: env.details.detail(target.id)
                load(detail, req)
                begin(req.startMs ?: 0)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failWith(e)
            }
        }
    }

    // ---------------------------------------------------------------- state

    /** The menus: tracks and qualities, as chosen now. */
    private fun publishChoices() {
        val chosenFile = file
        val streaming = session?.info?.hls ?: false
        val info = session?.info
        val audioChoices = PlaybackPlan.audioStreams(chosenFile).map { TrackChoice(it.index.toLong(), streamLabel(it), it.index == audio?.index) }
        val subs = PlaybackPlan.subtitleStreams(chosenFile)
        val subtitleChoices = if (subs.isEmpty()) {
            emptyList()
        } else {
            listOf(TrackChoice(null, texts.subtitlesOff, subtitle == null)) +
                subs.map { TrackChoice(it.index.toLong(), streamLabel(it), it.index == subtitle?.index, burnIn = streaming && PlaybackPlan.isImage(it) && !sideload) }
        }
        val sub = subtitle
        // Text moves always; pictures only when drawn here from the server's .mks.
        val syncable = sub != null && (!PlaybackPlan.isImage(sub) || (streaming && sideload && info?.burnSubtitles != true))
        val auto = info?.autoQuality?.takeIf { quality.original }?.let { Qualities.byId(it) }?.takeIf { !it.original }
        val fileHint = chosenFile?.let { f ->
            listOfNotNull(resolutionLabel(f.width, f.height), formatBitrate(f.bitrate).ifEmpty { null }).joinToString(" · ").ifEmpty { null }
        }
        val qualities = Qualities.forFile(chosenFile).map { preset ->
            QualityChoice(
                id = preset.id,
                label = texts.quality(preset),
                hint = when {
                    !preset.original -> texts.converted
                    auto != null -> texts.playsAs(auto, fileHint)
                    else -> fileHint
                },
                selected = preset.id == quality.id,
            )
        }
        ui.update {
            it.copy(
                audio = audioChoices,
                subtitles = subtitleChoices,
                qualities = qualities,
                quality = if (auto != null) texts.autoQuality(auto) else texts.quality(quality),
                subtitleSync = syncable,
                subtitleOffsetMs = offset.value,
            )
        }
    }

    /** PUT /api/items/{id}/streams: the next episode, or the next viewing, starts with the same tracks. */
    private fun saveStreams() {
        val itemId = item?.id ?: return
        val choice = StreamChoice(fileId = file?.id, audioIndex = audio?.index, subtitleIndex = subtitle?.index)
        env.reporter.send { env.api.setStreams(itemId, choice) }
    }

    private fun dropTrickplay() {
        trickplayWatch?.cancel()
        trickplayWatch = null
        trickplay?.close()
        trickplay = null
    }

    private fun phase(phase: PlayerPhase) {
        ui.update { if (it.phase is PlayerPhase.Error) it else it.copy(phase = phase) }
    }

    private fun notice(text: String) = ui.update { it.copy(notice = text) }

    private fun fail(message: String, canRetry: Boolean) {
        ui.update { it.copy(phase = PlayerPhase.Error(message, canRetry), skipLabel = null) }
    }

    private fun failWith(error: Exception) {
        val message = describe(error)
        val canRetry = !(error is ApiException && (error.status == 403 || error.status == 404 || error.signedOut))
        fail(message, canRetry)
    }

    companion object {
        const val TICK_MS = 500L
        const val PROGRESS_MS = 10_000L

        /** While paused: the server forgets players it hasn't heard from for 90 s. */
        const val PAUSED_HEARTBEAT_MS = 30_000L
        const val SEEK_REPORT_MS = 1_000L

        /** How long playback may stand still, wanting to play, before it counts as stalled. */
        const val STALL_MS = 12_000L

        /** An end at least this far before the server's duration… */
        const val SHORT_END_MS = 60_000L

        /** …that comes less than this after where playback started is media that falls short, not the end. */
        const val SHORT_PLAY_MS = 5_000L

        /** Less than this buffered ahead while waiting: starving for data. */
        const val STARVING_MS = 1_000L
        const val STRUGGLE_SAMPLE_MS = 2_000L
        const val DETAILS_MS = 2_000L
        const val COUNTDOWN_SECONDS = 10

        /** The longest a new session waits for the one before to be ended. */
        const val END_WAIT_MS = 2_000L

        /** Seeks stop this short of the end, so the end isn't reached by a seek. */
        const val SEEK_END_MARGIN_MS = 250L
        const val UP_NEXT_IMAGE_WIDTH = 480
        private const val CUE_MIN_MS = 20L
        private const val CUE_MAX_MS = 250L
        private const val CUE_IDLE_MS = 500L
    }
}
