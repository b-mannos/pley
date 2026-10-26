package app.pley.android.player.engine

import android.annotation.SuppressLint
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Window
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionParameters.AudioOffloadPreferences
import androidx.media3.common.Tracks
import androidx.media3.common.util.Clock
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.SeekParameters
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.trackselection.DefaultTrackSelector
import app.pley.android.caps.CapsDetails
import app.pley.android.caps.CapsGraph
import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.DecoderPolicy
import app.pley.android.caps.DeviceCapabilities
import app.pley.android.common.DeviceProfile
import app.pley.android.model.DynamicRange
import app.pley.android.network.NetworkGraph
import app.pley.android.player.EngineSource
import app.pley.android.player.PlayerEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

/** The playback settings the engine follows (Settings → Playback and Advanced). */
data class EngineSettings(
    val refreshRate: RefreshRateMatching = RefreshRateMatching.Auto,
    /** Advanced: tunneled playback on a model not on the allowlist. */
    val tunneling: Boolean = false,
)

/**
 * Engines on Media3 for this device: its caps decide the decoders, the
 * TV's display modes and Dolby Vision; [mediaClient] gives each server's
 * media client (Bearer to that server only). Engines are made and used on
 * the main thread.
 */
class Media3EngineFactory(
    private val capabilities: DeviceCapabilities,
    private val mediaClient: (origin: String) -> OkHttpClient,
    private val settings: () -> EngineSettings = { EngineSettings() },
) : PlayerEngine.Factory {
    override fun create(context: Context, profile: DeviceProfile, music: Boolean): PlayerEngine = Media3Engine(
        context = context,
        profile = profile,
        music = music,
        caps = { capabilities.snapshot.value },
        mediaClient = mediaClient,
        settings = settings,
        frameRateMemory = PreferencesFrameRateMemory(context),
        focusPort = if (music) null else AndroidFocusPort(context),
    )

    companion object {
        fun of(caps: CapsGraph, network: NetworkGraph, settings: () -> EngineSettings = { EngineSettings() }): Media3EngineFactory =
            Media3EngineFactory(caps.capabilities, network::media, settings)
    }
}

/**
 * [PlayerEngine] on ExoPlayer, as pley plays (P3-ENGINE):
 * - decoders by the caps' policy (TVs hardware only), falling back to the
 *   next decoder when one fails, asynchronous queueing on Fire TV;
 * - tracks never beyond what the device plays (the session asks the server
 *   instead), selections redone when the audio output changes, Dolby Vision
 *   preferred on Dolby Vision screens, tunneling only where allowed;
 * - buffers by memory tier, starting sooner from a home server;
 * - on TVs, the display mode matched to the video's frame rate before it
 *   plays; on phones, Media3's seamless-only rate hints;
 * - audio focus that pauses rather than ducks passthrough, pauses when
 *   headphones or HDMI go, and an audio output started again after it
 *   fails;
 * - stats, a struggling decoder, and tracks the device can't play, in
 *   [state].
 * Music engines have no video renderer, let Media3 handle focus and
 * offload audio with the screen off.
 */
@OptIn(markerClass = [UnstableApi::class])
class Media3Engine(
    context: Context,
    private val profile: DeviceProfile,
    private val music: Boolean,
    private val caps: () -> CapsSnapshot?,
    mediaClient: (origin: String) -> OkHttpClient,
    private val settings: () -> EngineSettings,
    private val frameRateMemory: FrameRateMemory,
    focusPort: FocusPort?,
    private val sdk: Int = Build.VERSION.SDK_INT,
    private val now: () -> Long = SystemClock::elapsedRealtime,
    private val hostFor: (Window) -> DisplayModeHost = ::WindowModeHost,
    /** Media3's clock; tests pass one that moves on its own. */
    playerClock: Clock = Clock.DEFAULT,
) : PlayerEngine {
    private val app = context.applicationContext
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val main = Handler(Looper.getMainLooper())
    private val mutableState = MutableStateFlow(EngineState())
    override val state: StateFlow<EngineState> = mutableState.asStateFlow()

    private val details: CapsDetails? get() = caps()?.details

    private val rules = DecoderRules(
        policy = { details?.policy ?: DecoderPolicy(tv = profile.isTv) },
        dolbyVisionScreen = { details?.takeIf { it.input != null }?.let { DynamicRange.DolbyVision in it.screenHdrTypes } },
    )

    private val loadControl = PleyLoadControl(BufferPlan.of(profile))
    private val trackSelector = DefaultTrackSelector(app)
    private val loading = MediaLoading(app, mediaClient)

    override val player: ExoPlayer = ExoPlayer.Builder(app)
        .setRenderersFactory(PleyRenderersFactory(app, rules, asyncQueueing = details?.forceAsyncQueueing ?: profile.fireTv, music = music))
        .setTrackSelector(trackSelector)
        .setLoadControl(loadControl)
        .setMediaSourceFactory(loading.sources)
        .setAudioAttributes(audioAttributes(music), music)
        .setHandleAudioBecomingNoisy(true)
        .setVideoChangeFrameRateStrategy(frameRateStrategy())
        .setSeekBackIncrementMs(SEEK_STEP_MS)
        .setSeekForwardIncrementMs(SEEK_STEP_MS)
        .withClock(playerClock)
        .apply { if (music) setWakeMode(C.WAKE_MODE_NETWORK) }
        .build()

    private val focus: AudioFocusRules? = focusPort?.let { port ->
        AudioFocusRules(
            port,
            object : FocusTarget {
                override fun pause() = pauseOurselves()

                override fun play() = player.play()

                override fun setVolume(volume: Float) {
                    player.volume = volume
                }
            },
        )
    }

    private val hdmi: HdmiUnplugWatcher? = if (music) null else HdmiUnplugWatcher(app) { player.pause() }
    private val restarts = AudioRestarts(now)
    private val struggle = StruggleDetector()
    private val analytics = PlaybackAnalytics(now, ::isHardware) { stats -> mutableState.update { it.copy(stats = stats) } }

    private var window: Window? = null
    private var frameRate: FrameRateController? = null
    private var matchJob: Job? = null
    private var matchPending = false
    private var request: MatchRequest? = null
    private var wantPlay = false
    private var ownPauses = 0
    private var released = false

    /**
     * A file played as it is that starts part way (a resume) starts at the
     * key frame before its place (Media3's PREVIOUS_SYNC, once the file's
     * index is read), so no frame is decoded only to be thrown away; seeks
     * are exact again once its first frame shows. On the Google TV Streamer
     * a 1080p HEVC resume went from 3.75 s to 2.3 s from the press to the
     * first frame, a 4K one from 4.4 s to 3.6 s. Not for HLS: the server's
     * segments start where the playback does, and moving into the one
     * being fetched made a 4K remux start later.
     */
    private var snapToKeyFrame = false

    private val tick = object : Runnable {
        override fun run() {
            sample()
            main.postDelayed(this, TICK_MS)
        }
    }

    init {
        player.addListener(Listener())
        player.addAnalyticsListener(analytics)
        player.addAnalyticsListener(AudioOutput())
        applyTrackParameters()
        hdmi?.start()
    }

    override fun prepare(source: EngineSource, startMs: Long, playWhenReady: Boolean) {
        check(!released) { "The engine was released" }
        matchJob?.cancel()
        wantPlay = playWhenReady
        loadControl.home = BufferPlan.isHome(source.url)
        applyTrackParameters()
        player.setVideoChangeFrameRateStrategy(frameRateStrategy())
        struggle.reset()
        analytics.onPrepare(frameRate?.requestedModeId?.takeIf { it != 0 })
        mutableState.update { EngineState(stats = it.stats) }
        val asked = matchRequest(source)
        request = asked
        matchPending = asked != null
        snapToKeyFrame = startMs > 0 && !music && !source.hls
        player.setSeekParameters(SeekParameters.EXACT)
        player.setMediaSource(loading.mediaSource(source), startMs.coerceAtLeast(0))
        setPlayWhenReady(playWhenReady && !matchPending)
        player.prepare()
        if (matchPending && window != null) startMatch()
    }

    override fun attachWindow(window: Window) {
        if (released || this.window === window) return
        this.window = window
        if (profile.isTv && !music) frameRate = FrameRateController(hostFor(window), frameRateMemory, sdk)
        if (matchPending) startMatch()
    }

    override fun detachWindow() {
        matchJob?.cancel()
        matchJob = null
        frameRate?.clear()
        frameRate = null
        window = null
        mutableState.update { it.copy(switchingDisplayMode = false, switchingToHz = null) }
    }

    override fun release() {
        if (released) return
        released = true
        detachWindow()
        scope.cancel()
        main.removeCallbacks(tick)
        hdmi?.stop()
        focus?.release()
        player.release()
    }

    /**
     * Media3's own refresh-rate switching: TVs match the display mode
     * themselves (FrameRateController); phones let Media3 switch when it
     * is seamless, unless the setting is Off. Read again for each video.
     */
    private fun frameRateStrategy(): Int = when {
        profile.isTv || settings().refreshRate == RefreshRateMatching.Off -> C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_OFF
        else -> C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS
    }

    /** The display match a video on a TV needs; `null` when nothing is to match. */
    private fun matchRequest(source: EngineSource): MatchRequest? {
        if (!profile.isTv || music) return null
        val setting = settings().refreshRate
        if (setting == RefreshRateMatching.Off) return null
        val video = source.video ?: return null
        if ((video.frameRate ?: 0.0) <= 1.0) return null
        if ((source.durationMs ?: 0) < ModeChoice.MIN_DURATION_MS) return null
        return ModeChoice.requestFor(video, source.durationMs, details, setting)
    }

    private fun startMatch() {
        val controller = frameRate
        val asked = request
        if (controller == null || asked == null) {
            // Not a TV window: nothing to wait for.
            matchPending = false
            setPlayWhenReady(wantPlay)
            return
        }
        matchJob?.cancel()
        matchJob = scope.launch {
            val outcome = controller.match(asked) { hz -> mutableState.update { it.copy(switchingDisplayMode = true, switchingToHz = hz) } }
            matchPending = false
            mutableState.update {
                it.copy(switchingDisplayMode = false, switchingToHz = null, frameRateHint = it.frameRateHint || outcome.showHint)
            }
            analytics.onDisplayMode(controller.requestedModeId.takeIf { it != 0 })
            // The screen's HDR types and the receiver's formats may have changed with the mode.
            if (outcome.switched) applyTrackParameters()
            setPlayWhenReady(wantPlay)
        }
    }

    private fun applyTrackParameters() {
        val caps = details
        val dolbyVisionScreen = caps?.input != null && DynamicRange.DolbyVision in caps.screenHdrTypes && caps.dolbyVisionProfiles.isNotEmpty()
        val builder = trackSelector.buildUponParameters()
            .setExceedRendererCapabilitiesIfNecessary(false)
            .setAllowInvalidateSelectionsOnRendererCapabilitiesChange(true)
            .setTunnelingEnabled(!music && (caps?.tunnelingAllowed == true || settings().tunneling))
            .setPreferredVideoMimeTypes(*if (dolbyVisionScreen) arrayOf(MimeTypes.VIDEO_DOLBY_VISION) else emptyArray())
        if (music) {
            builder.setTrackTypeDisabled(C.TRACK_TYPE_VIDEO, true)
                .setAudioOffloadPreferences(
                    AudioOffloadPreferences.Builder()
                        .setAudioOffloadMode(AudioOffloadPreferences.AUDIO_OFFLOAD_MODE_ENABLED)
                        .setIsGaplessSupportRequired(true)
                        .build(),
                )
        }
        trackSelector.setParameters(builder)
    }

    private fun setPlayWhenReady(play: Boolean) {
        if (play) {
            player.playWhenReady = true
        } else {
            pauseOurselves()
        }
    }

    /**
     * Pauses for the engine's own reasons (a display mode change, audio
     * focus): counted, so the listener tells them from the person's pauses
     * even though Media3 reports a change made inside a listener after it
     * returns.
     */
    private fun pauseOurselves() {
        if (!player.playWhenReady) return
        ownPauses++
        player.playWhenReady = false
    }

    /** Once a second while playing: the decoder's counts, and whether it struggles. */
    private fun sample() {
        val counters = player.videoDecoderCounters ?: return
        analytics.onCounters(counters)
        val stats = analytics.stats
        val struggling = struggle.add(now(), stats.renderedFrames, stats.droppedFrames, stats.maxConsecutiveDropped)
        if (struggling != state.value.struggling) mutableState.update { it.copy(struggling = struggling) }
    }

    private fun isHardware(name: String): Boolean? = details?.input?.decoders?.firstOrNull { it.name == name }?.hardware

    private inner class Listener : Player.Listener {
        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            if (!playWhenReady && ownPauses > 0) {
                ownPauses--
                return
            }
            if (playWhenReady) {
                when {
                    // Asked to play while the display changes: it plays once the mode settles.
                    matchPending -> {
                        wantPlay = true
                        pauseOurselves()
                    }
                    focus != null && !focus.beforePlay() -> pauseOurselves()
                }
            } else {
                wantPlay = false
                focus?.onPausedByPerson()
            }
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            main.removeCallbacks(tick)
            if (isPlaying) main.postDelayed(tick, TICK_MS)
        }

        override fun onPositionDiscontinuity(oldPosition: Player.PositionInfo, newPosition: Player.PositionInfo, reason: Int) {
            if (reason == Player.DISCONTINUITY_REASON_SEEK) struggle.reset()
        }

        override fun onRenderedFirstFrame() {
            player.setSeekParameters(SeekParameters.EXACT)
        }

        override fun onTracksChanged(tracks: Tracks) {
            if (tracks.isEmpty) return
            if (snapToKeyFrame) {
                snapToKeyFrame = false
                player.setSeekParameters(SeekParameters.PREVIOUS_SYNC)
                player.seekTo(player.currentPosition)
            }
            // Music plays no video: an album's cover track isn't missing anything.
            val groups = TrackMapper.groupsOf(tracks).filter { !music || it.type != C.TRACK_TYPE_VIDEO }
            val help = UnsupportedTrackWatcher.check(groups)
            if (help != state.value.needsServerHelp) mutableState.update { it.copy(needsServerHelp = help) }
        }

        override fun onPlayerError(error: PlaybackException) {
            if (restarts.shouldRestart(error)) {
                mutableState.update { it.copy(audioRestarts = it.audioRestarts + 1) }
                // After an error the player keeps its position: preparing again resumes there.
                player.prepare()
            }
        }
    }

    /** Tells the focus rules when encoded audio passes through. */
    private inner class AudioOutput : AnalyticsListener {
        override fun onAudioTrackInitialized(eventTime: AnalyticsListener.EventTime, audioTrackConfig: AudioSink.AudioTrackConfig) {
            focus?.passthrough = PlaybackAnalytics.isPassthrough(audioTrackConfig.encoding)
        }
    }

    companion object {
        const val SEEK_STEP_MS = 10_000L
        const val TICK_MS = 1_000L

        /** Media3 marks setClock for tests; the app always passes Clock.DEFAULT, tests a clock that moves by itself. */
        @SuppressLint("VisibleForTests")
        private fun ExoPlayer.Builder.withClock(clock: Clock): ExoPlayer.Builder =
            if (clock === Clock.DEFAULT) this else setClock(clock)

        fun audioAttributes(music: Boolean): AudioAttributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(if (music) C.AUDIO_CONTENT_TYPE_MUSIC else C.AUDIO_CONTENT_TYPE_MOVIE)
            .build()
    }
}
