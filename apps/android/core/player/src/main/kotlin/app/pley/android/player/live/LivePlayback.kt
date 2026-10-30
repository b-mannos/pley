package app.pley.android.player.live

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Timeline
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.HttpDataSource
import app.pley.android.common.DeviceProfile
import app.pley.android.common.asSentence
import app.pley.android.common.sentenceOf
import app.pley.android.data.LiveTvRepository
import app.pley.android.model.ApiErrorBody
import app.pley.android.model.PleyJson
import app.pley.android.network.ApiException
import app.pley.android.network.OfflineException
import app.pley.android.player.EngineSource
import app.pley.android.player.PlayerEngine
import app.pley.android.player.session.clientIdOf
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Why a channel doesn't play, by what the server answered (docs/API.md,
 * "Watching"), with its own sentence: a `409` names the channels holding
 * every connection of the source.
 */
data class LiveTrouble(val kind: Kind, val message: String) {
    enum class Kind {
        /** `409`: every connection of the channel's source is in use. */
        Busy,

        /** `410`: an administrator stopped the channel, or the provider ended it. */
        Stopped,

        /** `404`: the channel was hidden or its source removed. */
        Missing,

        /** `403`: Live TV isn't open to this profile. */
        NotAllowed,

        /** `503` or `504`: the provider sends nothing, or not in time. */
        Unavailable,

        /** The server is out of reach. */
        Offline,
        Other,
    }

    /**
     * Whether trying again may help: not for a channel that is gone. A
     * stopped one (an administrator, a server restarting) tunes again.
     */
    val canRetry: Boolean get() = kind != Kind.Missing

    companion object {
        /** The trouble [error] is: a refusal of the server's, a network failure, or else. */
        fun of(error: Throwable): LiveTrouble = when (error) {
            is ApiException -> LiveTrouble(kindOf(error.status), sentenceOf(error))
            is OfflineException -> LiveTrouble(Kind.Offline, sentenceOf(error))
            else -> LiveTrouble(Kind.Other, sentenceOf(error))
        }

        /** The trouble of a stream that stopped: the server's answer to a playlist or segment, when it gave one. */
        @OptIn(markerClass = [UnstableApi::class])
        fun of(error: PlaybackException): LiveTrouble {
            var cause: Throwable? = error
            while (cause != null) {
                if (cause is HttpDataSource.InvalidResponseCodeException) {
                    val said = runCatching { PleyJson.decodeFromString(ApiErrorBody.serializer(), cause.responseBody.decodeToString()).message }
                        .getOrNull()?.takeIf { it.isNotBlank() }
                    return LiveTrouble(kindOf(cause.responseCode), said?.let(::sentence) ?: "The server answered ${cause.responseCode}.")
                }
                if (cause is HttpDataSource.HttpDataSourceException) return LiveTrouble(Kind.Offline, sentenceOf(OfflineException(cause)))
                cause = cause.cause
            }
            return LiveTrouble(Kind.Other, sentence(error.errorCodeName))
        }

        private fun kindOf(status: Int): Kind = when (status) {
            409 -> Kind.Busy
            410 -> Kind.Stopped
            404 -> Kind.Missing
            403 -> Kind.NotAllowed
            502, 503, 504 -> Kind.Unavailable
            else -> Kind.Other
        }

        private fun sentence(text: String): String = asSentence(text)
    }
}

/** Where watching a channel is. */
sealed interface LivePhase {
    /** Nothing is tuned: before the first channel, or let go in the background. */
    data object Idle : LivePhase

    /** Asking the server for the channel, then for its first segments. */
    data object Tuning : LivePhase

    /** The channel's picture (or sound) is out. */
    data object Playing : LivePhase

    data class Failed(val trouble: LiveTrouble) : LivePhase
}

/**
 * What a live player shows. Positions are in the window the server keeps
 * of the channel (its timeshift): [positionMs] from its start, [windowMs]
 * long, [behindMs] behind the live edge.
 */
data class LiveState(
    val channelId: Long? = null,
    val phase: LivePhase = LivePhase.Idle,
    /** Playing, or about to: not paused. */
    val playWhenReady: Boolean = true,
    /** Waiting for data while playing. */
    val buffering: Boolean = false,
    val positionMs: Long = 0,
    val windowMs: Long = 0,
    val behindMs: Long = 0,
    /** The channel has sound only (radio): no picture comes. */
    val audioOnly: Boolean = false,
) {
    /** Close enough to the live edge to say Live. */
    val atLive: Boolean get() = behindMs < LIVE_WITHIN_MS

    /** The window is long enough to pause and go back in. */
    val canRewind: Boolean get() = windowMs >= MIN_WINDOW_MS

    companion object {
        /** Behind the live edge by less than this is live (the web's 10 s). */
        const val LIVE_WITHIN_MS = 10_000L

        /** A window shorter than this is no timeshift: a channel just tuned keeps a few segments. */
        const val MIN_WINDOW_MS = 30_000L
    }
}

/**
 * Watching Live TV on this device (docs/API.md, "Live TV"), shared by the
 * phone's and the TV's live players:
 *
 * - [tune] asks the server for the channel with this device's
 *   [clientId], which ends what it watched before (zapping never holds
 *   two of the provider's connections), lets the previous session go, and
 *   plays the answer's HLS from the live edge on the app's Media3 engine;
 * - paused, the server is told every [KEEP_ALIVE_MS] that it is still
 *   watched (a paused player asks for nothing, and 45 s of silence is
 *   leaving); played again, it goes on from where it paused while the
 *   window holds it, else from the live edge;
 * - [stop] lets the session and the engine go (the screen went to the
 *   background), [resume] tunes the channel again, [close] ends it all.
 *
 * Made and used on the main thread: [scope] is the screen's.
 */
@OptIn(markerClass = [UnstableApi::class])
class LivePlayback(
    private val live: LiveTvRepository,
    private val engines: () -> PlayerEngine,
    private val clientId: () -> String,
    private val scope: CoroutineScope,
) {
    private val mutableState = MutableStateFlow(LiveState())
    val state: StateFlow<LiveState> = mutableState.asStateFlow()

    private val mutableEngine = MutableStateFlow<PlayerEngine?>(null)

    /** The engine playing now, for the screen's video surface and media session; `null` before the first tune. */
    val engine: StateFlow<PlayerEngine?> = mutableEngine.asStateFlow()

    private var session: String? = null
    private var tuning: Job? = null
    private var keepAlive: Job? = null
    private var ticker: Job? = null
    private var closed = false
    private val listener = Listener()
    private val window = Timeline.Window()

    /** Starts watching [channelId] in place of what played. */
    fun tune(channelId: Long) {
        if (closed) return
        tuning?.cancel()
        keepAlive?.cancel()
        letSessionGo()
        mutableEngine.value?.player?.let {
            it.stop()
            it.clearMediaItems()
        }
        mutableState.value = LiveState(channelId = channelId, phase = LivePhase.Tuning)
        tuning = scope.launch {
            val watching = try {
                live.watch(channelId, clientId())
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                fail(LiveTrouble.of(e))
                return@launch
            }
            session = watching.sessionId
            val engine = mutableEngine.value ?: engines().also {
                it.player.addListener(listener)
                mutableEngine.value = it
            }
            engine.prepare(EngineSource(url = watching.url, hls = true, live = true), startMs = 0, playWhenReady = true)
            startTicker()
        }
    }

    /** Tunes the channel again: after a failure, or back from the background. */
    fun retry() {
        state.value.channelId?.let(::tune)
    }

    /** Back in front: the channel left in [stop] plays again, from the live edge. */
    fun resume() {
        if (state.value.phase == LivePhase.Idle) retry()
    }

    fun play() {
        mutableEngine.value?.player?.play()
    }

    fun pause() {
        mutableEngine.value?.player?.pause()
    }

    fun togglePlay() = if (state.value.playWhenReady) pause() else play()

    /** Moves [deltaMs] (back when negative) in the window, never past the live edge. */
    fun seekBy(deltaMs: Long) {
        val s = state.value
        if (!s.canRewind) return
        seekTo(s.positionMs + deltaMs)
    }

    /** Moves to [positionMs] of the window, never past the live edge. */
    fun seekTo(positionMs: Long) {
        val player = mutableEngine.value?.player ?: return
        val edge = liveEdgeMs(player) ?: return
        player.seekTo(positionMs.coerceIn(0, edge))
        sample()
    }

    /** Back to the live edge, playing. */
    fun goLive() {
        val player = mutableEngine.value?.player ?: return
        player.seekToDefaultPosition()
        player.play()
        sample()
    }

    /**
     * Lets the session and the engine go, keeping which channel it was:
     * the screen is in the background. Nothing plays and no connection is
     * held until [resume].
     */
    fun stop() {
        tuning?.cancel()
        keepAlive?.cancel()
        ticker?.cancel()
        letSessionGo()
        mutableEngine.value?.let {
            it.player.removeListener(listener)
            it.release()
        }
        mutableEngine.value = null
        mutableState.update { LiveState(channelId = it.channelId) }
    }

    /** Ends watching for good. */
    fun close() {
        stop()
        closed = true
    }

    private fun letSessionGo() {
        session?.let(live::leave)
        session = null
    }

    private fun fail(trouble: LiveTrouble) {
        keepAlive?.cancel()
        letSessionGo()
        mutableEngine.value?.player?.stop()
        mutableState.update { it.copy(phase = LivePhase.Failed(trouble), buffering = false) }
    }

    private fun startTicker() {
        if (ticker?.isActive == true) return
        ticker = scope.launch {
            while (isActive) {
                sample()
                delay(TICK_MS)
            }
        }
    }

    /** Where the player is in the window, now. */
    private fun sample() {
        val player = mutableEngine.value?.player ?: return
        val edge = liveEdgeMs(player) ?: return
        val position = player.currentPosition.coerceAtLeast(0)
        val length = window.durationMs.takeIf { it != C.TIME_UNSET } ?: edge
        mutableState.update {
            it.copy(positionMs = position, windowMs = length.coerceAtLeast(edge), behindMs = (edge - position).coerceAtLeast(0))
        }
    }

    /** The live edge, as a position in the window (the player's default position); `null` before the playlist is read. */
    private fun liveEdgeMs(player: Player): Long? {
        val timeline = player.currentTimeline
        if (timeline.isEmpty) return null
        timeline.getWindow(player.currentMediaItemIndex, window)
        return window.defaultPositionMs
    }

    private fun keepTelling() {
        keepAlive?.cancel()
        val id = session ?: return
        keepAlive = scope.launch {
            while (isActive) {
                delay(KEEP_ALIVE_MS)
                try {
                    live.keepWatching(id)
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    // Out of reach for now: playing again asks for the stream itself.
                }
            }
        }
    }

    private inner class Listener : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            when (playbackState) {
                Player.STATE_READY -> mutableState.update {
                    if (it.phase is LivePhase.Tuning) it.copy(phase = LivePhase.Playing, buffering = false) else it.copy(buffering = false)
                }
                Player.STATE_BUFFERING -> mutableState.update { if (it.phase is LivePhase.Playing) it.copy(buffering = true) else it }
                else -> Unit
            }
            sample()
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            mutableState.update { it.copy(playWhenReady = playWhenReady) }
            if (playWhenReady) keepAlive?.cancel() else keepTelling()
        }

        override fun onTracksChanged(tracks: Tracks) {
            if (tracks.isEmpty) return
            mutableState.update { it.copy(audioOnly = !tracks.containsType(C.TRACK_TYPE_VIDEO)) }
        }

        override fun onPlayerError(error: PlaybackException) {
            val player = mutableEngine.value?.player ?: return
            if (error.errorCode == PlaybackException.ERROR_CODE_BEHIND_LIVE_WINDOW) {
                // Paused past what the server keeps: on from the live edge.
                player.seekToDefaultPosition()
                player.prepare()
                return
            }
            fail(LiveTrouble.of(error))
        }
    }

    companion object {
        /** A paused player tells the server it still watches this often (it lets a silent viewer go after 45 s). */
        const val KEEP_ALIVE_MS = 20_000L

        /** How often the position in the window is read while it plays. */
        const val TICK_MS = 500L

        /**
         * Watching on this device: [live]'s server, engines of [engines],
         * the device's own client id (the one its playbacks send).
         */
        fun create(
            context: Context,
            live: LiveTvRepository,
            engines: PlayerEngine.Factory,
            profile: DeviceProfile,
            scope: CoroutineScope,
        ): LivePlayback {
            val app = context.applicationContext
            return LivePlayback(live, { engines.create(app, profile, music = false) }, clientIdOf(app), scope)
        }
    }
}
