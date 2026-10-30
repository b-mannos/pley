package app.pley.android.player.live

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
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

/** What a live player shows. Live TV is watched as it airs: no pausing, no going back. */
data class LiveState(
    val channelId: Long? = null,
    val phase: LivePhase = LivePhase.Idle,
    /** Waiting for data while playing. */
    val buffering: Boolean = false,
    /** The channel has sound only (radio): no picture comes. */
    val audioOnly: Boolean = false,
)

/**
 * Watching Live TV on this device (docs/API.md, "Live TV"), shared by the
 * phone's and the TV's live players:
 *
 * - [tune] asks the server for the channel with this device's
 *   [clientId], which ends what it watched before (zapping never holds
 *   two of the provider's connections), lets the previous session go, and
 *   plays the answer's HLS from the live edge on the app's Media3 engine;
 * - it is watched as it airs, as the web's: no pausing, no going back;
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
    private var closed = false
    private val listener = Listener()

    /** Starts watching [channelId] in place of what played. */
    fun tune(channelId: Long) {
        if (closed) return
        tuning?.cancel()
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

    /**
     * Lets the session and the engine go, keeping which channel it was:
     * the screen is in the background. Nothing plays and no connection is
     * held until [resume].
     */
    fun stop() {
        tuning?.cancel()
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
        letSessionGo()
        mutableEngine.value?.player?.stop()
        mutableState.update { it.copy(phase = LivePhase.Failed(trouble), buffering = false) }
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
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            // Live has no pause (headphones pulled out, a stray media key): it plays on, as a browser's does.
            if (!playWhenReady) mutableEngine.value?.player?.play()
        }

        override fun onTracksChanged(tracks: Tracks) {
            if (tracks.isEmpty) return
            mutableState.update { it.copy(audioOnly = !tracks.containsType(C.TRACK_TYPE_VIDEO)) }
        }

        override fun onPlayerError(error: PlaybackException) {
            val player = mutableEngine.value?.player ?: return
            if (error.errorCode == PlaybackException.ERROR_CODE_BEHIND_LIVE_WINDOW) {
                // Fell behind what the server keeps (a long stall): on from the live edge.
                player.seekToDefaultPosition()
                player.prepare()
                return
            }
            fail(LiveTrouble.of(error))
        }
    }

    companion object {
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
