package app.pley.android.data

import android.content.Context
import app.pley.android.auth.AuthGraph
import app.pley.android.common.DeviceProfile
import app.pley.android.common.NotBuiltYet
import app.pley.android.common.SavedServer
import app.pley.android.model.CollectionDetail
import app.pley.android.model.CollectionSummary
import app.pley.android.model.HistoryPage
import app.pley.android.model.Hubs
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemSummary
import app.pley.android.model.Library
import app.pley.android.model.LibraryFilters
import app.pley.android.model.PersonDetail
import app.pley.android.model.PlaylistDetail
import app.pley.android.model.PlaylistSummary
import app.pley.android.model.SearchResults
import app.pley.android.model.TrackDefaults
import app.pley.android.network.NetworkGraph
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flowOf

/** Everything the screens read and change, made lazily. */
interface DataGraph {
    val servers: ServersRepository
    val session: SessionRepository
    val snapshots: SnapshotStore
    val libraries: LibrariesRepository
    val hubs: HubsRepository
    val libraryItems: LibraryItemsRepository
    val items: ItemRepository
    val search: SearchRepository
    val people: PeopleRepository
    val collections: CollectionsRepository
    val playlists: PlaylistsRepository
    val history: HistoryRepository
    val watchState: WatchStateRepository
    val trackDefaults: TrackDefaultsRepository
    val playItem: PlayItem
    val liveUpdates: LiveUpdates
    val settings: AppSettings

    companion object {
        /** Not built yet: every repository answers that it isn't. */
        fun create(context: Context, network: NetworkGraph, auth: AuthGraph, profile: DeviceProfile): DataGraph = NotBuiltData
    }
}

private fun <T> notBuilt(what: String): Flow<Loadable<T>> = flowOf(Loadable.Failed(NotBuiltYet(what)))

private fun fail(what: String): Nothing = throw NotBuiltYet(what)

private object NotBuiltData : DataGraph {
    override val servers = object : ServersRepository {
        override val saved: Flow<List<SavedServer>> = flowOf(emptyList())
        override val active: StateFlow<String?> = MutableStateFlow(null)
        override suspend fun use(server: SavedServer) = fail("Choosing a server")
        override suspend fun forget(origin: String) = fail("Forgetting a server")
    }

    override val session = object : SessionRepository {
        override val session: StateFlow<Session?> = MutableStateFlow(null)
        override suspend fun signOut() = fail("Signing out")
    }

    override val snapshots = object : SnapshotStore {
        override suspend fun read(key: String): ByteArray? = null
        override suspend fun write(key: String, bytes: ByteArray) = Unit
        override suspend fun clear(issuer: String) = Unit
    }

    override val libraries = object : LibrariesRepository {
        override fun libraries(): Flow<Loadable<List<Library>>> = notBuilt("Libraries")
        override suspend fun refresh() = Unit
    }

    override val hubs = object : HubsRepository {
        override fun home(): Flow<Loadable<Hubs>> = notBuilt("Home")
        override fun library(libraryId: Long): Flow<Loadable<Hubs>> = notBuilt("Recommended")
        override suspend fun refresh() = Unit
    }

    override val libraryItems = object : LibraryItemsRepository {
        override fun items(libraryId: Long, query: LibraryQuery): Flow<Loadable<LibraryItems>> = notBuilt("Library pages")
        override fun filters(libraryId: Long): Flow<Loadable<LibraryFilters>> = notBuilt("Filters")
    }

    override val items = object : ItemRepository {
        override fun detail(itemId: Long): Flow<Loadable<ItemDetail>> = notBuilt("Item pages")
        override fun children(itemId: Long): Flow<Loadable<List<ItemSummary>>> = notBuilt("Seasons and episodes")
        override fun onDeck(itemId: Long): Flow<Loadable<ItemSummary?>> = notBuilt("On deck")
        override fun related(itemId: Long): Flow<Loadable<List<ItemSummary>>> = notBuilt("More like this")
        override fun collectionsOf(itemId: Long): Flow<Loadable<List<CollectionSummary>>> = notBuilt("Part of")
        override suspend fun next(itemId: Long): ItemSummary? = fail("Up next")
        override fun prefetch(itemId: Long) = Unit
    }

    override val search = object : SearchRepository {
        override fun search(query: String): Flow<Loadable<SearchResults>> = notBuilt("Search")
        override val recent: Flow<List<String>> = flowOf(emptyList())
    }

    override val people = object : PeopleRepository {
        override fun person(personId: Long): Flow<Loadable<PersonDetail>> = notBuilt("People")
    }

    override val collections = object : CollectionsRepository {
        override fun collections(libraryId: Long): Flow<Loadable<List<CollectionSummary>>> = notBuilt("Collections")
        override fun collection(collectionId: Long): Flow<Loadable<CollectionDetail>> = notBuilt("Collections")
    }

    override val playlists = object : PlaylistsRepository {
        override fun playlists(): Flow<Loadable<List<PlaylistSummary>>> = notBuilt("Playlists")
        override fun playlist(playlistId: Long): Flow<Loadable<PlaylistDetail>> = notBuilt("Playlists")
        override suspend fun create(title: String, itemIds: List<Long>): Long = fail("Playlists")
        override suspend fun createSmart(title: String, libraryId: Long, query: String): Long = fail("Smart playlists")
        override suspend fun rename(playlistId: Long, title: String) = fail("Playlists")
        override suspend fun delete(playlistId: Long) = fail("Playlists")
        override suspend fun add(playlistId: Long, itemIds: List<Long>) = fail("Playlists")
        override suspend fun remove(playlistId: Long, entryIds: List<Long>) = fail("Playlists")
        override suspend fun reorder(playlistId: Long, entryIds: List<Long>) = fail("Playlists")
    }

    override val history = object : HistoryRepository {
        override fun history(): Flow<Loadable<HistoryPage>> = notBuilt("History")
        override suspend fun loadMore() = Unit
        override suspend fun remove(entryId: Long) = fail("History")
    }

    override val watchState = object : WatchStateRepository {
        override suspend fun setWatched(itemId: Long, watched: Boolean) = fail("Watched marks")
        override suspend fun rate(itemId: Long, rating: Double?) = fail("Ratings")
        override suspend fun chooseStreams(itemId: Long, fileId: Long?, audioIndex: Int?, subtitleIndex: Int?) = fail("Track choices")
    }

    override val trackDefaults = object : TrackDefaultsRepository {
        override fun defaults(): Flow<Loadable<TrackDefaults>> = notBuilt("Track defaults")
        override suspend fun save(defaults: TrackDefaults) = fail("Track defaults")
    }

    override val playItem = PlayItem { _, _ -> fail("Playing") }

    override val liveUpdates = object : LiveUpdates {
        override val scans: StateFlow<List<ScanState>> = MutableStateFlow(emptyList())
    }

    override val settings = object : AppSettings {
        override val settings: Flow<Settings> = flowOf(Settings())
        override suspend fun update(change: (Settings) -> Settings) = fail("Settings")
    }
}
