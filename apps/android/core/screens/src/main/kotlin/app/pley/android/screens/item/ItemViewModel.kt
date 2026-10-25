package app.pley.android.screens.item

import android.os.Build
import android.os.Trace
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.common.TraceNames
import app.pley.android.data.DataGraph
import app.pley.android.data.Loadable
import app.pley.android.data.PlayTarget
import app.pley.android.data.map
import app.pley.android.data.valueOrNull
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.network.ApiException
import app.pley.android.player.PlayRequest
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.PleyKey
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.Locale

/**
 * Something the page tells the person once: nothing to play, or an action
 * that failed (the shell's toast says why).
 */
sealed interface ItemEvent {
    data object NothingToPlay : ItemEvent

    data class Failed(val error: Throwable) : ItemEvent
}

/**
 * An item page (movie, show, season, episode), for the phone and the TV
 * alike (apps/web/src/pages/item): the item from what was saved first, the
 * server's answer after; its seasons and episodes, the next episode,
 * More Like This and its collections beside it; and what can be done there.
 *
 * Playing goes through [PlayItem][app.pley.android.data.PlayItem] (movies and
 * episodes resume, shows play their next episode) and the [PlayerLauncher],
 * with the version and tracks chosen here. Choosing an audio or subtitle
 * track is saved for the item (PUT /api/items/{id}/streams), as on the web.
 * Watched marks and ratings show at once and are undone if the server
 * refuses.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ItemViewModel(
    private val data: DataGraph,
    private val launcher: PlayerLauncher,
    val itemId: Long,
    compute: CoroutineDispatcher = Dispatchers.Default,
    private val locale: Locale = Locale.getDefault(),
) : ViewModel() {
    private val pick = MutableStateFlow(StreamPick())
    private val chosenSeason = MutableStateFlow<Long?>(null)
    private val retries = MutableStateFlow(0)
    private val eventChannel = Channel<ItemEvent>(Channel.BUFFERED)

    /** What the page says once: shown by the interface as a toast. */
    val events: Flow<ItemEvent> = eventChannel.receiveAsFlow()

    /** The detail last shown, for actions. */
    @Volatile
    private var shown: ItemDetail? = null

    private var traced = false

    init {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            Trace.beginAsyncSection(TraceNames.NAV_ITEM, itemId.toInt())
            traced = true
        }
    }

    val state: StateFlow<ItemUiState> = retries
        .flatMapLatest { pageStates() }
        .flowOn(compute)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_MS), ItemUiState.Loading)

    private fun pageStates(): Flow<ItemUiState> {
        val detail = data.items.detail(itemId)
        val shape = detail.map { load -> load.valueOrNull?.let { Shape(it.kind, it.parentId) } }.distinctUntilChanged()
        return shape.flatMapLatest { s ->
            when {
                s == null -> detail.map(::withoutDetail)
                s.kind == ItemKind.Artist -> flowOf(ItemUiState.Elsewhere(PleyKey.Artist(itemId)))
                s.kind == ItemKind.Album -> flowOf(ItemUiState.Elsewhere(PleyKey.Album(itemId)))
                s.kind == ItemKind.Track -> flowOf(if (s.parentId != null) ItemUiState.Elsewhere(PleyKey.Album(s.parentId)) else ItemUiState.NotFound)
                s.kind == ItemKind.Extra -> flowOf(if (s.parentId != null) ItemUiState.Elsewhere(PleyKey.Item(s.parentId)) else ItemUiState.NotFound)
                s.kind == ItemKind.Unknown -> flowOf(ItemUiState.NotFound)
                else -> pages(detail, s)
            }
        }
    }

    /** The page's state while no detail is known: loading, not found, or failed. */
    private fun withoutDetail(load: Loadable<ItemDetail>): ItemUiState = when (load) {
        is Loadable.Failed -> if ((load.error as? ApiException)?.status == 404) ItemUiState.NotFound else ItemUiState.Failed(load.error)
        else -> ItemUiState.Loading
    }

    private fun pages(detail: Flow<Loadable<ItemDetail>>, shape: Shape): Flow<ItemUiState> {
        val origin = data.servers.active.map { it.orEmpty() }.distinctUntilChanged()
        val related: Flow<List<ItemSummary>> = if (shape.kind == ItemKind.Movie || shape.kind == ItemKind.Show) {
            data.items.related(itemId).map { it.valueOrNull.orEmpty() }
        } else {
            flowOf(emptyList())
        }
        val partOf = if (shape.kind == ItemKind.Movie || shape.kind == ItemKind.Show) {
            data.items.collectionsOf(itemId).map { it.valueOrNull.orEmpty() }
        } else {
            flowOf(emptyList())
        }
        val onDeck: Flow<Loadable<ItemSummary?>?> = when (shape.kind) {
            ItemKind.Show, ItemKind.Season -> data.items.onDeck(itemId)
            else -> flowOf(null)
        }
        // The tabs: a show's seasons, or a season's and its siblings.
        val seasons: Flow<List<ItemSummary>?> = when (shape.kind) {
            ItemKind.Show -> data.items.children(itemId).map { it.valueOrNull }
            ItemKind.Season -> shape.parentId?.let { show -> data.items.children(show).map { it.valueOrNull } } ?: flowOf(null)
            else -> flowOf(null)
        }
        val selected: Flow<Long?> = when (shape.kind) {
            ItemKind.Show -> combine(chosenSeason, seasons, onDeck) { chosen, list, next ->
                chosen ?: when {
                    list == null -> null
                    // Wait for the next episode before picking a season, so the page doesn't jump to it.
                    next == null || next is Loadable.Loading -> null
                    else -> defaultSeason(list, next.valueOrNull)
                }
            }
            ItemKind.Season -> chosenSeason.map { it ?: itemId }
            ItemKind.Episode -> flowOf(shape.parentId)
            else -> flowOf(null)
        }.distinctUntilChanged()
        val episodes: Flow<Pair<Long?, List<ItemSummary>?>> = selected.flatMapLatest { season ->
            if (season == null) flowOf(null to null) else data.items.children(season).map { season to it.valueOrNull }
        }
        val beside = combine(onDeck, seasons, episodes, related, partOf) { next, list, (season, eps), rel, part ->
            Beside(next, list, season, eps, rel, part)
        }
        return combine(detail, origin, pick, beside) { load, server, picked, b ->
            val d = load.valueOrNull ?: return@combine withoutDetail(load)
            shown = d
            val page = itemPage(
                PageInput(
                    detail = d,
                    origin = server,
                    pick = picked,
                    onDeck = b.onDeck,
                    seasons = b.seasons,
                    selectedSeason = b.season,
                    episodes = b.episodes,
                    related = b.related,
                    partOf = b.partOf,
                    locale = locale,
                ),
            )
            ItemUiState.Ready(page, load.map { })
        }
    }

    /** The page first drew its content: the end of [TraceNames.NAV_ITEM]. */
    fun onShown() {
        if (traced) {
            traced = false
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) Trace.endAsyncSection(TraceNames.NAV_ITEM, itemId.toInt())
        }
    }

    /** Loads the page again after it failed. */
    fun retry() {
        retries.update { it + 1 }
    }

    /**
     * Plays the item: a movie or episode from where it was left (or from
     * the start), with the version and tracks chosen here; a show or season
     * its next episode.
     */
    fun play(fromStart: Boolean = false) {
        val d = shown ?: return
        viewModelScope.launch {
            val target = attempt { data.playItem.resolve(d.toSummary(), fromStart) } ?: return@launch
            when (target) {
                is PlayTarget.Video -> launcher.play(request(d, target, fromStart))
                else -> eventChannel.send(ItemEvent.NothingToPlay)
            }
        }
    }

    /** The request for [target]: this item's own choices when it is the item played. */
    private fun request(d: ItemDetail, target: PlayTarget.Video, fromStart: Boolean): PlayRequest {
        if (target.itemId != d.id) return PlayRequest(itemId = target.itemId, startMs = target.startMs, fromStart = fromStart)
        val playing = playing(d, pick.value)
        val audio = playing.audioIndex?.takeIf { it != playing.defaultAudio?.index }
        return PlayRequest(
            itemId = d.id,
            fileId = playing.file?.id?.takeIf { d.files.size > 1 },
            startMs = target.startMs,
            fromStart = fromStart,
            audioIndex = audio?.toLong(),
            subtitleIndex = playing.subtitleIndex?.toLong(),
        )
    }

    /** Plays [item] (an episode of the list, an extra), from where it was left unless [fromStart]. */
    fun play(item: ItemSummary, fromStart: Boolean = false) {
        if (item.id == shown?.id) return play(fromStart)
        viewModelScope.launch { playItem(data, launcher, item, fromStart)?.let { eventChannel.send(it) } }
    }

    /** Marks the item watched, or unwatched when it is. */
    fun toggleWatched() {
        val page = (state.value as? ItemUiState.Ready)?.page ?: return
        setWatched(page.id, !page.watched)
    }

    /** Marks [id] (this item, or one of its episodes) watched or not. */
    fun setWatched(id: Long, watched: Boolean) {
        viewModelScope.launch { attempt { data.watchState.setWatched(id, watched) } }
    }

    /** The profile's rating, 1 to 10 (a half star each), or none. */
    fun rate(rating: Int?) {
        viewModelScope.launch { attempt { data.watchState.rate(itemId, rating?.takeIf { it > 0 }?.toDouble()) } }
    }

    /** Plays another version: its tracks start as the server would pick them. Not saved, as on the web. */
    fun chooseVersion(fileId: Long) {
        pick.update { it.copy(fileId = fileId) }
    }

    /** Plays audio track [index], and keeps it for the item (and the rest of a show). */
    fun chooseAudio(index: Int) {
        val d = shown ?: return
        val file = playing(d, pick.value).file ?: return
        pick.update { it.copy(audioFile = file.id, audio = index) }
        remember(d, file.id, index, playing(d, pick.value).subtitleIndex)
    }

    /** Shows subtitle track [index] (`null`: off), and keeps it for the item. */
    fun chooseSubtitle(index: Int?) {
        val d = shown ?: return
        val file = playing(d, pick.value).file ?: return
        val audio = playing(d, pick.value).audioIndex
        pick.update { it.copy(subtitleFile = file.id, subtitle = index) }
        remember(d, file.id, audio, index)
    }

    private fun remember(d: ItemDetail, fileId: Long, audio: Int?, subtitle: Int?) {
        viewModelScope.launch { attempt { data.watchState.chooseStreams(d.id, fileId, audio, subtitle) } }
    }

    /** [block]'s value; `null` once a failure was told as an event. */
    private suspend fun <T> attempt(block: suspend () -> T): T? = try {
        block()
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        eventChannel.send(ItemEvent.Failed(e))
        null
    }

    /** Shows [seasonId]'s episodes. */
    fun selectSeason(seasonId: Long) {
        chosenSeason.value = seasonId
    }

    override fun onCleared() {
        onShown()
    }

    private data class Shape(val kind: ItemKind, val parentId: Long?)

    private data class Beside(
        val onDeck: Loadable<ItemSummary?>?,
        val seasons: List<ItemSummary>?,
        val season: Long?,
        val episodes: List<ItemSummary>?,
        val related: List<ItemSummary>,
        val partOf: List<app.pley.android.model.CollectionSummary>,
    )

    companion object {
        /** Keeps loading this long after the page stops showing (rotation, a quick look elsewhere). */
        const val STOP_MS = 5_000L

        /** The page of [itemId], made with the app's graph. */
        fun factory(itemId: Long): ViewModelProvider.Factory = viewModelFactory {
            initializer {
                val graph = screensGraph()
                ItemViewModel(graph.data, graph.playerLauncher, itemId)
            }
        }
    }
}

/**
 * Plays [item] as pressing play does: a video from where it was left (or
 * from the start), a show or season its next episode. What to tell the
 * person when it can't play, or `null`.
 */
internal suspend fun playItem(data: DataGraph, launcher: PlayerLauncher, item: ItemSummary, fromStart: Boolean): ItemEvent? {
    val target = try {
        data.playItem.resolve(item, fromStart)
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        return ItemEvent.Failed(e)
    }
    return when (target) {
        is PlayTarget.Video -> {
            launcher.play(PlayRequest(itemId = target.itemId, startMs = target.startMs, fromStart = fromStart))
            null
        }
        else -> ItemEvent.NothingToPlay
    }
}
