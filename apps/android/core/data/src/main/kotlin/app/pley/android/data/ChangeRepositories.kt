package app.pley.android.data

import app.pley.android.model.PlaylistDetail
import app.pley.android.model.PlaylistSummary
import app.pley.android.model.TrackDefaults
import kotlinx.coroutines.flow.Flow

/**
 * This profile's playlists, and changes to them: shown at once, undone
 * if the server refuses. Smart playlists can't be edited here.
 */
interface PlaylistsRepository {
    fun playlists(): Flow<Loadable<List<PlaylistSummary>>>

    fun playlist(playlistId: Long): Flow<Loadable<PlaylistDetail>>

    /** A new playlist with [itemIds]; its id. */
    suspend fun create(title: String, itemIds: List<Long> = emptyList()): Long

    /** A smart playlist of a library's [query] (the web's filter string). */
    suspend fun createSmart(title: String, libraryId: Long, query: String): Long

    suspend fun rename(playlistId: Long, title: String)

    suspend fun delete(playlistId: Long)

    suspend fun add(playlistId: Long, itemIds: List<Long>)

    suspend fun remove(playlistId: Long, entryIds: List<Long>)

    /** The entries in their new order. */
    suspend fun reorder(playlistId: Long, entryIds: List<Long>)
}

/**
 * Watched marks, ratings and track choices: every list and page showing the
 * item changes at once, then everything is fetched again from the server.
 */
interface WatchStateRepository {
    suspend fun setWatched(itemId: Long, watched: Boolean)

    /** Stars out of 10 (halves allowed), or none when [rating] is `null`. */
    suspend fun rate(itemId: Long, rating: Double?)

    /** The version and tracks to play [itemId] with next time. */
    suspend fun chooseStreams(itemId: Long, fileId: Long?, audioIndex: Int?, subtitleIndex: Int?)
}

/** The profile's audio and subtitle defaults; older servers don't offer them to apps. */
interface TrackDefaultsRepository {
    fun defaults(): Flow<Loadable<TrackDefaults>>

    suspend fun save(defaults: TrackDefaults)
}
