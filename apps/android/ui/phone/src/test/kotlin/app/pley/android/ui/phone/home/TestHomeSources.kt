package app.pley.android.ui.phone.home


import app.pley.android.data.HubsRepository
import app.pley.android.data.ItemRepository
import app.pley.android.data.LibrariesRepository
import app.pley.android.data.Loadable
import app.pley.android.data.PlayItem
import app.pley.android.data.PlayTarget
import app.pley.android.data.ScanState
import app.pley.android.data.Session
import app.pley.android.data.WatchStateRepository
import app.pley.android.model.CollectionSummary
import app.pley.android.model.Hubs
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemSummary
import app.pley.android.model.Library
import app.pley.android.player.PlayRequest
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.home.HomeSources
import java.util.Collections
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** Home's sources, every one a fake the test drives and reads. */
class TestHomeSources : HomeSources {
    val home = MutableStateFlow<Loadable<Hubs>>(Loadable.Loading)
    val libraryList = MutableStateFlow<Loadable<List<Library>>>(Loadable.Loading)
    val details = HashMap<Long, MutableStateFlow<Loadable<ItemDetail>>>()
    val prefetched: MutableList<Long> = Collections.synchronizedList(mutableListOf())
    val sessionFlow = MutableStateFlow<Session?>(Session(ORIGIN, null, emptySet()))
    val scanList = MutableStateFlow<List<ScanState>>(emptyList())
    val played: MutableList<PlayRequest> = Collections.synchronizedList(mutableListOf())
    val watched: MutableList<Pair<Long, Boolean>> = Collections.synchronizedList(mutableListOf())
    var hubRefreshes = 0
    var libraryRefreshes = 0

    /** What pressing play on an item resolves to: by default a video from its resume point. */
    var resolve: (ItemSummary, Boolean) -> PlayTarget = { item, fromStart ->
        PlayTarget.Video(item.id, if (fromStart) 0 else item.userState.positionMs)
    }

    fun detail(id: Long): MutableStateFlow<Loadable<ItemDetail>> = details.getOrPut(id) { MutableStateFlow(Loadable.Loading) }

    override val hubs = object : HubsRepository {
        override fun home(): Flow<Loadable<Hubs>> = this@TestHomeSources.home
        override fun library(libraryId: Long): Flow<Loadable<Hubs>> = error("not on Home")
        override suspend fun refresh() {
            hubRefreshes++
        }
    }

    override val libraries = object : LibrariesRepository {
        override fun libraries(): Flow<Loadable<List<Library>>> = libraryList
        override suspend fun refresh() {
            libraryRefreshes++
        }
    }

    override val items = object : ItemRepository {
        override fun detail(itemId: Long): Flow<Loadable<ItemDetail>> = this@TestHomeSources.detail(itemId)
        override fun children(itemId: Long): Flow<Loadable<List<ItemSummary>>> = error("not on Home")
        override fun onDeck(itemId: Long): Flow<Loadable<ItemSummary?>> = error("not on Home")
        override fun related(itemId: Long): Flow<Loadable<List<ItemSummary>>> = error("not on Home")
        override fun collectionsOf(itemId: Long): Flow<Loadable<List<CollectionSummary>>> = error("not on Home")
        override suspend fun next(itemId: Long): ItemSummary? = error("not on Home")
        override fun prefetch(itemId: Long) {
            prefetched += itemId
        }
    }

    override val session: Flow<Session?> = sessionFlow
    override val scans: StateFlow<List<ScanState>> = scanList
    override val playItem = PlayItem { item, fromStart -> resolve(item, fromStart) }

    override val watchState = object : WatchStateRepository {
        override suspend fun setWatched(itemId: Long, watched: Boolean) {
            this@TestHomeSources.watched += itemId to watched
        }

        override suspend fun rate(itemId: Long, rating: Double?) = Unit
        override suspend fun chooseStreams(itemId: Long, fileId: Long?, audioIndex: Int?, subtitleIndex: Int?) = Unit
    }

    override val launcher = object : PlayerLauncher {
        override fun play(request: PlayRequest) {
            played += request
        }
    }

    companion object {
        /** Nothing listens there: pictures fail at once and placeholders show. */
        const val ORIGIN = "http://127.0.0.1:9"
    }
}
