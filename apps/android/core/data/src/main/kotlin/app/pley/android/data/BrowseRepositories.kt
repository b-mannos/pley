package app.pley.android.data

import app.pley.android.model.CollectionDetail
import app.pley.android.model.CollectionSummary
import app.pley.android.model.HistoryPage
import app.pley.android.model.Hubs
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemSummary
import app.pley.android.model.Library
import app.pley.android.model.LibraryFilters
import app.pley.android.model.LibrarySort
import app.pley.android.model.PersonDetail
import app.pley.android.model.SearchResults
import app.pley.android.model.SortOrder
import kotlinx.coroutines.flow.Flow

/** The libraries this profile may see, in the server's order. */
interface LibrariesRepository {
    fun libraries(): Flow<Loadable<List<Library>>>

    suspend fun refresh()
}

/** The rows of Home, and of each library's Recommended tab. */
interface HubsRepository {
    fun home(): Flow<Loadable<Hubs>>

    fun library(libraryId: Long): Flow<Loadable<Hubs>>

    suspend fun refresh()
}

/** How a library is listed: the web's sorts and filters. */
data class LibraryQuery(
    val sort: LibrarySort = LibrarySort.Title,
    val order: SortOrder = SortOrder.Ascending,
    /** `true` unwatched only, `false` watched only, `null` both. */
    val unwatched: Boolean? = null,
    val genre: String? = null,
    val year: Int? = null,
)

/**
 * A library's items as one list, however many there are: the first ones
 * for the first frame, the rest in pages after.
 */
data class LibraryItems(
    val total: Int,
    /** What is loaded so far, in order. */
    val items: List<ItemSummary>,
)

interface LibraryItemsRepository {
    fun items(libraryId: Long, query: LibraryQuery): Flow<Loadable<LibraryItems>>

    fun filters(libraryId: Long): Flow<Loadable<LibraryFilters>>
}

/** An item's page: its details, children, next episode, related items and collections. */
interface ItemRepository {
    fun detail(itemId: Long): Flow<Loadable<ItemDetail>>

    /** A show's seasons, a season's episodes, an album's tracks. */
    fun children(itemId: Long): Flow<Loadable<List<ItemSummary>>>

    /** The episode to play next for a show or season. */
    fun onDeck(itemId: Long): Flow<Loadable<ItemSummary?>>

    fun related(itemId: Long): Flow<Loadable<List<ItemSummary>>>

    fun collectionsOf(itemId: Long): Flow<Loadable<List<CollectionSummary>>>

    /** The episode after [itemId], asked fresh (Up Next). */
    suspend fun next(itemId: Long): ItemSummary?

    /** Loads [itemId]'s details ahead (a likely next tap), at low priority. */
    fun prefetch(itemId: Long)
}

interface SearchRepository {
    /** Results for [query]: only the latest query's answer is kept. */
    fun search(query: String): Flow<Loadable<SearchResults>>

    /** This profile's recent searches, newest first. */
    val recent: Flow<List<String>>
}

interface PeopleRepository {
    fun person(personId: Long): Flow<Loadable<PersonDetail>>
}

interface CollectionsRepository {
    fun collections(libraryId: Long): Flow<Loadable<List<CollectionSummary>>>

    fun collection(collectionId: Long): Flow<Loadable<CollectionDetail>>
}

/** What this profile watched, newest first; entries can be removed. */
interface HistoryRepository {
    fun history(): Flow<Loadable<HistoryPage>>

    suspend fun loadMore()

    suspend fun remove(entryId: Long)
}
