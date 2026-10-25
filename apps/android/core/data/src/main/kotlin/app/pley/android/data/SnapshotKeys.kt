package app.pley.android.data

/**
 * The names answers are kept under, per server and profile, in memory and
 * as snapshots. One place, so live updates and changes find what they
 * refresh.
 */
object SnapshotKeys {
    /** GET /api/hubs/home. */
    const val HOME = "home"

    /** GET /api/libraries. */
    const val LIBRARIES = "libraries"

    /** GET /api/playlists. */
    const val PLAYLISTS = "playlists"

    /** The first page of GET /api/history. */
    const val HISTORY = "history:head"

    /** GET /api/auth/me. */
    const val ME = "me"

    /** This profile's recent searches. */
    const val SEARCHES = "searches"

    fun libraryHubs(libraryId: Long) = "libhubs:$libraryId"

    /** The first items of a library's listing, for the first frame. */
    fun libraryHead(libraryId: Long, query: LibraryQuery) = "library:$libraryId:${queryKey(query)}:head"

    fun filters(libraryId: Long) = "filters:$libraryId"

    fun collections(libraryId: Long) = "collections:$libraryId"

    fun collection(collectionId: Long) = "collection:$collectionId"

    fun item(itemId: Long) = "item:$itemId"

    fun children(itemId: Long) = "children:$itemId"

    fun onDeck(itemId: Long) = "ondeck:$itemId"

    fun related(itemId: Long) = "related:$itemId"

    fun partOf(itemId: Long) = "partof:$itemId"

    fun person(personId: Long) = "person:$personId"

    fun playlist(playlistId: Long) = "playlist:$playlistId"

    fun search(query: String) = "search:$query"

    /** The library a key is about, from its name alone; `null` for the others. */
    fun libraryOf(key: String): Long? {
        val prefix = key.substringBefore(':')
        if (prefix !in LIBRARY_KEYS) return null
        return key.substringAfter(':').substringBefore(':').toLongOrNull()
    }

    /** [query] in a key: sort, order, watched, genre and year. */
    fun queryKey(query: LibraryQuery): String = buildString {
        append(query.sort.wire).append('.').append(query.order.wire)
        when (query.unwatched) {
            true -> append(".unwatched")
            false -> append(".watched")
            null -> Unit
        }
        query.genre?.let { append(".g=").append(it) }
        query.year?.let { append(".y=").append(it) }
    }

    private val LIBRARY_KEYS = setOf("libhubs", "library", "filters", "collections")
}
