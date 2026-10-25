package app.pley.android.screens.item

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.DataGraph
import app.pley.android.data.Loadable
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.format.cardSubtitle
import app.pley.android.model.format.isInProgress
import app.pley.android.network.ApiException
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.PleyKey
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/**
 * What can be done with an item from its action sheet, its TV panel or a
 * long press on its tile (ItemActions.tsx without the administrator's
 * part): playing, watched marks, playlists, and going to its show or
 * season. Editing, refreshing and analysing are the web's, for
 * administrators: never offered here, whatever the server says of the
 * profile.
 */
enum class ItemAction {
    Play,
    Resume,
    FromStart,
    MarkWatched,
    MarkUnwatched,
    AddToPlaylist,
    GoToShow,
    GoToSeason,

    /** Artists, albums and tracks: their music page, which plays them. */
    Open,
}

/** The actions for [item], in the order they are listed. */
fun itemActions(item: ItemSummary): List<ItemAction> = buildList {
    val kind = item.kind
    val music = kind == ItemKind.Artist || kind == ItemKind.Album || kind == ItemKind.Track
    if (music) {
        add(ItemAction.Open)
    } else if (kind != ItemKind.Unknown) {
        if (isInProgress(item)) {
            add(ItemAction.Resume)
            add(ItemAction.FromStart)
        } else {
            add(ItemAction.Play)
        }
    }
    if (kind == ItemKind.Movie || kind == ItemKind.Episode || kind == ItemKind.Show || kind == ItemKind.Season) {
        add(if (watched(item)) ItemAction.MarkUnwatched else ItemAction.MarkWatched)
    }
    if (kind != ItemKind.Extra && kind != ItemKind.Unknown) add(ItemAction.AddToPlaylist)
    if (kind == ItemKind.Episode && item.grandparentId != null) add(ItemAction.GoToShow)
    if (kind == ItemKind.Episode && item.parentId != null) add(ItemAction.GoToSeason)
    if (kind == ItemKind.Season && item.parentId != null) add(ItemAction.GoToShow)
}

/** Watched: a video marked so, or every episode of a show or season. */
fun watched(item: ItemSummary): Boolean = when (item.kind) {
    ItemKind.Show, ItemKind.Season -> {
        val leaves = item.leafCount ?: 0
        leaves > 0 && (item.viewedLeafCount ?: 0) >= leaves
    }
    else -> item.userState.watched
}

/** Where [action] on [item] goes, for the actions that open a page or a sheet; `null` for the others. */
fun destination(action: ItemAction, item: ItemSummary): PleyKey? = when (action) {
    ItemAction.AddToPlaylist -> PleyKey.AddToPlaylist(listOf(item.id))
    ItemAction.GoToShow -> (if (item.kind == ItemKind.Season) item.parentId else item.grandparentId)?.let { PleyKey.Item(it) }
    ItemAction.GoToSeason -> item.parentId?.let { PleyKey.Item(it) }
    ItemAction.Open -> when (item.kind) {
        ItemKind.Artist -> PleyKey.Artist(item.id)
        ItemKind.Album -> PleyKey.Album(item.id)
        else -> item.parentId?.let { PleyKey.Album(it) }
    }
    else -> null
}

/** The sheet's state: the item's name and its actions. */
sealed interface ItemActionsUiState {
    data object Loading : ItemActionsUiState

    data class Failed(val error: Throwable) : ItemActionsUiState

    data class Ready(
        val item: ItemSummary,
        /** The server's origin, for the item's pictures. */
        val origin: String,
        /** "The Office", or an episode's show. */
        val title: String,
        /** "S2 · E5 · Office Olympics", "2019", "3 seasons". */
        val subtitle: String,
        val actions: List<ItemAction>,
    ) : ItemActionsUiState
}

/**
 * The action sheet of an item (PleyKey.ItemActions), opened by a long press
 * on its card or the page's More. Play and the watched marks are done
 * here; the rest open a page ([destination]).
 */
class ItemActionsViewModel(
    private val data: DataGraph,
    private val launcher: PlayerLauncher,
    val itemId: Long,
    compute: CoroutineDispatcher = Dispatchers.Default,
    /** Where actions run: they outlive the sheet, which closes as soon as one is chosen. */
    private val work: CoroutineScope = OutlivingWork,
) : ViewModel() {
    private val eventChannel = Channel<ItemEvent>(Channel.BUFFERED)

    /** Failures and "nothing to play", told once. */
    val events: Flow<ItemEvent> = eventChannel.receiveAsFlow()

    val state: StateFlow<ItemActionsUiState> = combine(data.items.detail(itemId), data.servers.active) { load, origin ->
        when (load) {
            Loadable.Loading -> ItemActionsUiState.Loading
            is Loadable.Failed -> load.stale?.let { ready(it.toSummary(), origin.orEmpty()) } ?: ItemActionsUiState.Failed(load.error)
            is Loadable.Ready -> ready(load.value.toSummary(), origin.orEmpty())
        }
    }
        .flowOn(compute)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(ItemViewModel.STOP_MS), ItemActionsUiState.Loading)

    private fun ready(item: ItemSummary, origin: String): ItemActionsUiState.Ready {
        val episode = item.kind == ItemKind.Episode
        return ItemActionsUiState.Ready(
            item = item,
            origin = origin,
            title = if (episode) item.grandparentTitle?.takeIf { it.isNotEmpty() } ?: item.title else item.title,
            subtitle = if (episode) {
                listOf(cardSubtitle(item), item.title.takeIf { item.grandparentTitle?.isNotEmpty() == true }.orEmpty())
                    .filter { it.isNotEmpty() }.joinToString(" · ")
            } else {
                cardSubtitle(item)
            },
            actions = itemActions(item),
        )
    }

    /**
     * Does [action] if it is done here (play, from start, the watched
     * marks): `true` once it is under way, so the sheet can close.
     */
    fun perform(action: ItemAction): Boolean {
        val item = (state.value as? ItemActionsUiState.Ready)?.item ?: return false
        when (action) {
            ItemAction.Play, ItemAction.Resume, ItemAction.FromStart -> work.launch {
                playItem(data, launcher, item, fromStart = action == ItemAction.FromStart)?.let { eventChannel.trySend(it) }
            }
            ItemAction.MarkWatched, ItemAction.MarkUnwatched -> work.launch {
                try {
                    data.watchState.setWatched(item.id, action == ItemAction.MarkWatched)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    // The marks are undone by then; the sheet, if still open, says why.
                    eventChannel.trySend(ItemEvent.Failed(e))
                }
            }
            else -> return false
        }
        return true
    }

    companion object {
        fun factory(itemId: Long): ViewModelProvider.Factory = viewModelFactory {
            initializer {
                val graph = screensGraph()
                ItemActionsViewModel(graph.data, graph.playerLauncher, itemId)
            }
        }
    }
}

/**
 * Work started from a sheet that closes at once: a play to launch, a mark
 * to send. It runs to its end on the main thread's loop, whatever happens
 * to the sheet.
 */
internal val OutlivingWork: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

/** A 404: the item or person isn't there for this profile. */
internal fun Throwable.isNotFound(): Boolean = (this as? ApiException)?.status == 404
