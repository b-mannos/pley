package app.pley.android.screens.livetv

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.LiveTvRepository
import app.pley.android.data.Loadable
import app.pley.android.data.valueOrNull
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveProgram
import app.pley.android.model.LiveSummary
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.ScreensGraph
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** What Live TV's pages and its player read and change. */
class LiveTvDeps(
    val live: LiveTvRepository,
    val launcher: PlayerLauncher,
    val origin: Flow<String?>,
    /** Unix ms. */
    val clock: () -> Long = System::currentTimeMillis,
    val compute: CoroutineDispatcher = Dispatchers.Default,
    /** Where changes run: they outlive the page that asked. */
    val work: CoroutineScope = LiveWork,
) {
    companion object {
        fun of(graph: ScreensGraph): LiveTvDeps = LiveTvDeps(graph.data.liveTv, graph.playerLauncher, graph.data.servers.active)
    }
}

/** The Live TV page, as its screens draw it. */
data class LivePage(
    /** Live TV is open to this profile (true until the server says). */
    val allowed: Boolean = true,
    /** Every channel, by number; `null` while nothing is known. */
    val channels: List<LiveChannel>? = null,
    /** A load that failed with nothing to show. */
    val error: Throwable? = null,
    /** The channels the filter and the search let through. */
    val shown: List<LiveChannel> = emptyList(),
    val groups: List<ChannelGroup> = emptyList(),
    val favorites: Int = 0,
    /** The filter in force: the one chosen, unless what it names is gone. */
    val filter: ChannelFilter = ChannelFilter.All,
    val query: String = "",
    val view: LiveView = LiveView.Guide,
    /** Recently watched, newest first. */
    val recent: List<LiveChannel> = emptyList(),
    /** The server's origin: logos and programme pictures are on it. */
    val origin: String = "",
)

/** The guide's programmes by channel, and whether the chunks in view are in. */
data class GuidePrograms(val byChannel: Map<Long, List<LiveProgram>> = emptyMap(), val loaded: Boolean = false)

/** Something to tell: a favourite the server refused. */
sealed interface LiveTvEvent {
    data class Failed(val error: Throwable) : LiveTvEvent
}

/**
 * Live TV (apps/web/src/pages/livetv/LiveTvPage.tsx): the channels with
 * what's on now and next, as a guide that scrolls through time or as
 * cards under Recently watched; filtered by All, Favourites or a group,
 * and searched by name, number or what's on. A channel opens the live
 * player. What's on is asked for again when a programme shown ends.
 *
 * The view and the filter are kept with the page (and its saved state).
 */
@OptIn(ExperimentalCoroutinesApi::class)
class LiveTvViewModel(private val deps: LiveTvDeps, private val saved: SavedStateHandle = SavedStateHandle()) : ViewModel() {
    private val retries = MutableStateFlow(0)
    private val chosenFilter = MutableStateFlow(ChannelFilter.parse(saved[FILTER]))
    private val query = MutableStateFlow("")
    private val view = MutableStateFlow(saved.get<String>(VIEW)?.let { runCatching { LiveView.valueOf(it) }.getOrNull() } ?: LiveView.Guide)
    private val eventChannel = Channel<LiveTvEvent>(Channel.BUFFERED)

    /** A favourite refused, to tell. */
    val events: Flow<LiveTvEvent> = eventChannel.receiveAsFlow()

    /** Unix seconds, every [TICK_MS]: progress bars, the guide's now line. */
    val now: StateFlow<Long> = flow {
        while (true) {
            emit(deps.clock() / 1000)
            delay(TICK_MS)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_AFTER_MS), deps.clock() / 1000)

    /** Where the guide starts: the half hour before the one it opened in, fixed while the page is open. */
    val guideStart: Long = LiveTv.guideStart(deps.clock() / 1000)

    private val summary: Flow<Loadable<LiveSummary>> = retries.flatMapLatest { deps.live.summary() }
    private val channels: Flow<Loadable<List<LiveChannel>>> = retries.flatMapLatest { deps.live.channels() }

    val state: StateFlow<LivePage> = combine(
        combine(summary, channels, deps.origin.map { it.orEmpty() }.distinctUntilChanged(), ::Triple),
        chosenFilter,
        query,
        view,
    ) { (summary, channels, origin), filter, q, v -> page(summary, channels, origin, filter, q, v) }
        .flowOn(deps.compute)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_AFTER_MS), LivePage(view = view.value, filter = chosenFilter.value))

    private val window = MutableStateFlow<Pair<Long, Long>?>(null)

    /** The guide's programmes for the time in view and either side of it. */
    val guide: StateFlow<GuidePrograms> = window
        .map { span -> span?.let { (from, to) -> LiveTv.chunks(guideStart, from, to) }.orEmpty() }
        .distinctUntilChanged()
        .flatMapLatest { chunks ->
            if (chunks.isEmpty()) {
                flowOf(GuidePrograms())
            } else {
                combine(chunks.map { deps.live.guide(it).onStart { emit(Loadable.Loading) } }) { loads ->
                    GuidePrograms(LiveTv.merge(loads.mapNotNull { it.valueOrNull }), loaded = loads.none { it is Loadable.Loading })
                }
            }
        }
        .flowOn(deps.compute)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_AFTER_MS), GuidePrograms())

    init {
        // What's on moves on: a programme listed as now that ended asks for the channels again.
        viewModelScope.launch {
            var asked = 0L
            combine(now, channels) { t, load -> t to load.valueOrNull.orEmpty() }.collect { (t, list) ->
                val ended = list.any { c -> c.now?.let { it.stop <= t } == true }
                if (ended && deps.clock() - asked > REFRESH_FLOOR_MS) {
                    asked = deps.clock()
                    deps.live.refresh()
                }
            }
        }
    }

    fun retry() = retries.update { it + 1 }

    fun setView(value: LiveView) {
        view.value = value
        saved[VIEW] = value.name
    }

    fun setFilter(value: ChannelFilter) {
        chosenFilter.value = value
        saved[FILTER] = value.key
    }

    fun setQuery(value: String) {
        query.value = value
    }

    /** Clears the search and the filter: every channel shows. */
    fun showAll() {
        setQuery("")
        setFilter(ChannelFilter.All)
    }

    /** The guide shows [from] to [to] (unix seconds): their chunks are loaded. Cheap: call it as it scrolls. */
    fun showing(from: Long, to: Long) {
        val chunk = Math.floorDiv(from, CHUNK) to Math.floorDiv(to, CHUNK)
        val now = window.value?.let { (f, t) -> Math.floorDiv(f, CHUNK) to Math.floorDiv(t, CHUNK) }
        if (chunk != now) window.value = from to to
    }

    /** Opens the live player on [channel]. */
    fun watch(channel: LiveChannel) = deps.launcher.watchLive(channel.id)

    fun toggleFavorite(channel: LiveChannel) {
        deps.work.launch {
            try {
                deps.live.setFavorite(channel.id, !channel.favorite)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                eventChannel.trySend(LiveTvEvent.Failed(e))
            }
        }
    }

    private fun page(
        summary: Loadable<LiveSummary>,
        channels: Loadable<List<LiveChannel>>,
        origin: String,
        filter: ChannelFilter,
        q: String,
        v: LiveView,
    ): LivePage {
        val all = channels.valueOrNull
        val list = all.orEmpty()
        val groups = LiveTv.groups(list)
        val favorites = list.count { it.favorite }
        val effective = LiveTv.effective(filter, groups, favorites)
        return LivePage(
            allowed = summary.valueOrNull?.allowed ?: true,
            channels = all,
            error = (channels as? Loadable.Failed)?.takeIf { all == null }?.error,
            shown = LiveTv.filtered(list, effective, q),
            groups = groups,
            favorites = favorites,
            filter = effective,
            query = q,
            view = v,
            recent = LiveTv.recent(list, summary.valueOrNull?.recent.orEmpty()),
            origin = origin,
        )
    }

    companion object {
        const val STOP_AFTER_MS = 5_000L

        /** The progress bars and the now line move this often. */
        const val TICK_MS = 15_000L

        /** What's on is asked for again at most this often. */
        const val REFRESH_FLOOR_MS = 60_000L

        private const val CHUNK = LiveTvRepository.GUIDE_CHUNK_S / 2
        private const val VIEW = "live.view"
        private const val FILTER = "live.filter"

        val Factory: ViewModelProvider.Factory = viewModelFactory {
            initializer { LiveTvViewModel(LiveTvDeps.of(screensGraph()), createSavedStateHandle()) }
        }
    }
}

/**
 * Whether Live TV has a way in (the phone's Libraries tab, the TV's
 * rail): open to this profile and with channels, as the web's sidebar
 * shows it. `null` while unknown, so nothing pops in and out.
 */
class LiveTvEntryViewModel(live: LiveTvRepository) : ViewModel() {
    /** The channels there are when Live TV shows; 0 when it doesn't. */
    val channels: StateFlow<Int?> = live.summary()
        .map { load ->
            when (load) {
                is Loadable.Ready -> load.value.channels.takeIf { load.value.available } ?: 0
                is Loadable.Failed -> load.stale?.let { if (it.available) it.channels else 0 } ?: 0
                Loadable.Loading -> null
            }
        }
        .distinctUntilChanged()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(LiveTvViewModel.STOP_AFTER_MS), null)

    companion object {
        val Factory: ViewModelProvider.Factory = viewModelFactory {
            initializer { LiveTvEntryViewModel(screensGraph().data.liveTv) }
        }
    }
}
