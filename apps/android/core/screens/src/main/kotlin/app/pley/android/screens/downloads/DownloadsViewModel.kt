package app.pley.android.screens.downloads

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.DownloadEntry
import app.pley.android.data.DownloadState
import app.pley.android.data.DownloadStorage
import app.pley.android.data.Downloads
import app.pley.android.model.ItemKind
import app.pley.android.player.PlayRequest
import app.pley.android.screens.ScreensGraph
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * A movie, or a show with its downloaded episodes (by season and number),
 * as the Downloads page lists them, newest first.
 */
data class DownloadGroup(
    /** The show's id; `null` for a movie. */
    val showId: Long?,
    /** The show's title, or the movie's. */
    val title: String,
    val entries: List<DownloadEntry>,
) {
    val key: String get() = showId?.let { "show-$it" } ?: "item-${entries.first().itemId}"

    /** What the group takes on the device, done or not. */
    val sizeBytes: Long get() = entries.sumOf { it.sizeBytes }
}

/** Something to tell about downloading. */
sealed interface DownloadMessage {
    /** The items couldn't be read to download them: [error] says why. */
    data class Failed(val error: Throwable) : DownloadMessage
}

/**
 * Downloads for a screen: the Downloads page's groups and storage, each
 * item's download (item pages, episode rows), and asking, retrying,
 * removing and playing them. On TVs, which download nothing, [available]
 * is false and everything is empty.
 */
class DownloadsViewModel(private val graph: ScreensGraph) : ViewModel() {
    private val downloads: Downloads = graph.downloads

    /** Whether this device downloads at all: phones and tablets. */
    val available: Boolean = downloads !== Downloads.None

    /** The profile's downloads by item. */
    val byItem: StateFlow<Map<Long, DownloadEntry>> = downloads.entries
        .map { list -> list.associateBy { it.itemId } }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_MS), emptyMap())

    /** The Downloads page: movies and shows, newest first. */
    val groups: StateFlow<List<DownloadGroup>> = downloads.entries
        .map(::groupsOf)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_MS), emptyList())

    val storage: StateFlow<DownloadStorage> = downloads.storage

    private val messages = Channel<DownloadMessage>(Channel.BUFFERED)
    val events: Flow<DownloadMessage> = messages.receiveAsFlow()

    /** Downloads [itemIds] (a movie, an episode, a season's episodes). */
    fun download(itemIds: List<Long>) {
        if (itemIds.isEmpty()) return
        viewModelScope.launch {
            try {
                // Going back right after asking still downloads them.
                withContext(NonCancellable) { downloads.download(itemIds) }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                messages.send(DownloadMessage.Failed(e))
            }
        }
    }

    fun remove(itemId: Long) = downloads.remove(itemId)

    fun retry(itemId: Long) = downloads.retry(itemId)

    fun removeAll() = downloads.removeAll()

    /** Plays a download, from where it was left here. */
    fun play(entry: DownloadEntry) {
        graph.playerLauncher.play(
            PlayRequest(
                itemId = entry.itemId,
                fileId = entry.fileId,
                startMs = entry.positionMs.takeIf { it > 0 && !entry.watched },
            ),
        )
    }

    companion object {
        private const val STOP_MS = 5_000L

        val factory = viewModelFactory { initializer { DownloadsViewModel(screensGraph()) } }

        /** Movies alone and shows with their episodes in order, the group with the newest download first. */
        fun groupsOf(entries: List<DownloadEntry>): List<DownloadGroup> {
            val groups = LinkedHashMap<String, MutableList<DownloadEntry>>()
            // Entries come newest first: a group sits where its newest is.
            for (entry in entries) {
                val key = if (entry.kind == ItemKind.Episode && entry.showId != null) "show-${entry.showId}" else "item-${entry.itemId}"
                groups.getOrPut(key) { mutableListOf() } += entry
            }
            return groups.values.map { list ->
                val first = list.first()
                if (first.kind == ItemKind.Episode && first.showId != null) {
                    DownloadGroup(
                        showId = first.showId,
                        title = first.showTitle ?: first.title,
                        entries = list.sortedWith(compareBy({ it.seasonIndex ?: Int.MAX_VALUE }, { it.episodeIndex ?: Int.MAX_VALUE })),
                    )
                } else {
                    DownloadGroup(showId = null, title = first.title, entries = list)
                }
            }
        }

        /** Whether [entry] can play without the server. */
        fun playable(entry: DownloadEntry?): Boolean = entry?.state == DownloadState.Done
    }
}
