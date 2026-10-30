package app.pley.android.screens.livetv

import android.app.Application
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.LiveTvRepository
import app.pley.android.data.valueOrNull
import app.pley.android.model.LiveChannel
import app.pley.android.player.PlayerEngine
import app.pley.android.player.live.LivePhase
import app.pley.android.player.live.LivePlayback
import app.pley.android.player.live.LiveState
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** What the live player reads and drives. */
class LivePlayerDeps(
    val live: LiveTvRepository,
    /** The playback, made in the ViewModel's scope (the main thread's). */
    val playback: (CoroutineScope) -> LivePlayback,
    val origin: Flow<String?>,
    /** Unix ms. */
    val clock: () -> Long = System::currentTimeMillis,
    val work: CoroutineScope = LiveWork,
)

/** A channel number being typed on a remote or keypad, or one that matched nothing. */
data class ChannelDigits(val typed: String = "", val notFound: String? = null)

/**
 * The live player (apps/web/src/player/live/LivePlayerPage.tsx), shared
 * by the phone's and the TV's: the channel playing, with what's on now and
 * next, channel up and down through the lineup (round its ends), a channel
 * number typed, pause and rewind in what the server keeps, a favourite.
 * The channel is kept with the screen's saved state, so the player comes
 * back on it. The playback ends with the ViewModel.
 */
class LivePlayerViewModel(
    private val deps: LivePlayerDeps,
    private val saved: SavedStateHandle,
    channelId: Long,
) : ViewModel() {
    private val playback = deps.playback(viewModelScope)
    private val mutableDigits = MutableStateFlow(ChannelDigits())
    private var digitsJob: Job? = null

    val state: StateFlow<LiveState> = playback.state

    /** The engine, for the video surface and the media session. */
    val engine: StateFlow<PlayerEngine?> = playback.engine

    /** Every channel, by number (the lineup channel up and down go through). */
    val channels: StateFlow<List<LiveChannel>> = deps.live.channels()
        .map { it.valueOrNull.orEmpty() }
        .distinctUntilChanged()
        .stateIn(viewModelScope, SharingStarted.Eagerly, emptyList())

    /** The channel playing, as the lineup has it (a favourite set shows at once). */
    val channel: StateFlow<LiveChannel?> = combine(channels, state.map { it.channelId }.distinctUntilChanged()) { list, id ->
        list.firstOrNull { it.id == id }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, null)

    /** The server's origin, for logos. */
    val origin: StateFlow<String> = deps.origin.map { it.orEmpty() }.stateIn(viewModelScope, SharingStarted.Eagerly, "")

    /** Unix seconds, every 5 s: what's on and how far it is. */
    val now: StateFlow<Long> = flow {
        while (true) {
            emit(deps.clock() / 1000)
            delay(TICK_MS)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(LiveTvViewModel.STOP_AFTER_MS), deps.clock() / 1000)

    /** A channel number being typed. */
    val digits: StateFlow<ChannelDigits> = mutableDigits.asStateFlow()

    init {
        playback.tune(saved[CHANNEL] ?: channelId)
        // What's on moves on while watching: the lineup is asked again when the programme playing ends.
        viewModelScope.launch {
            combine(now, channel) { t, c -> c?.now?.let { it.stop <= t } == true }.distinctUntilChanged().collect { ended ->
                if (ended) deps.live.refresh()
            }
        }
    }

    /** Watches [channelId] in place of what plays. */
    fun tune(channelId: Long) {
        if (channelId == state.value.channelId && state.value.phase !is LivePhase.Failed) return
        saved[CHANNEL] = channelId
        playback.tune(channelId)
    }

    /** Channel up ([by] 1, the next number) or down (-1), round the ends of the lineup. */
    fun step(by: Int) {
        LiveTv.step(channels.value, state.value.channelId, by)?.let { tune(it.id) }
    }

    /**
     * A digit of a channel number: the channel is tuned once no digit
     * came for [DIGITS_WAIT_MS], or "No channel 123" shows for a while.
     */
    fun digit(d: Char) {
        if (!d.isDigit()) return
        val typed = (mutableDigits.value.typed + d).take(MAX_DIGITS)
        mutableDigits.value = ChannelDigits(typed)
        digitsJob?.cancel()
        digitsJob = viewModelScope.launch {
            delay(DIGITS_WAIT_MS)
            val match = LiveTv.numbered(channels.value, typed)
            if (match != null) {
                mutableDigits.value = ChannelDigits()
                tune(match.id)
            } else {
                mutableDigits.value = ChannelDigits(notFound = typed)
                delay(NOT_FOUND_MS)
                mutableDigits.value = ChannelDigits()
            }
        }
    }

    fun play() = playback.play()

    fun pause() = playback.pause()

    fun togglePlay() = playback.togglePlay()

    fun seekBy(deltaMs: Long) = playback.seekBy(deltaMs)

    fun seekTo(positionMs: Long) = playback.seekTo(positionMs)

    fun goLive() = playback.goLive()

    fun retry() = playback.retry()

    /** The screen went to the background: the connection and the decoders are let go. */
    fun stop() = playback.stop()

    /** Back in front: the channel plays again. */
    fun resume() = playback.resume()

    fun toggleFavorite() {
        val c = channel.value ?: return
        deps.work.launch {
            try {
                deps.live.setFavorite(c.id, !c.favorite)
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
                // Put back by the repository: the star shows it.
            }
        }
    }

    override fun onCleared() {
        playback.close()
    }

    companion object {
        const val TICK_MS = 5_000L

        /** A typed number waits this long for another digit (the web's 1.4 s). */
        const val DIGITS_WAIT_MS = 1_400L
        const val NOT_FOUND_MS = 2_000L
        const val MAX_DIGITS = 5

        private const val CHANNEL = "live.channel"

        /** The player on [channelId] (the one opened; the saved state's after a restart). */
        fun factory(channelId: Long): ViewModelProvider.Factory = viewModelFactory {
            initializer {
                val graph = screensGraph()
                val app = this[ViewModelProvider.AndroidViewModelFactory.APPLICATION_KEY] as Application
                val deps = LivePlayerDeps(
                    live = graph.data.liveTv,
                    playback = { scope -> LivePlayback.create(app, graph.data.liveTv, graph.player.engines, graph.device, scope) },
                    origin = graph.data.servers.active,
                )
                LivePlayerViewModel(deps, createSavedStateHandle(), channelId)
            }
        }
    }
}
