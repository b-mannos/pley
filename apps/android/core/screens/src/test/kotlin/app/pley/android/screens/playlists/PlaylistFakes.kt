package app.pley.android.screens.playlists

import app.pley.android.data.Loadable
import app.pley.android.data.PlaylistsRepository
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.PlaylistDetail
import app.pley.android.model.PlaylistEntry
import app.pley.android.model.PlaylistKind
import app.pley.android.model.PlaylistSummary
import app.pley.android.model.UserState
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map

/**
 * Playlists as the data side keeps them for the tests: changes show at
 * once and are put back when the "server" refuses ([fail]). [gate], when
 * set, holds every change until the test completes it, so a test sees
 * what shows while one is on its way.
 */
class FakePlaylists : PlaylistsRepository {
    val details = HashMap<Long, MutableStateFlow<Loadable<PlaylistDetail>>>()
    val list = MutableStateFlow<Loadable<List<PlaylistSummary>>>(Loadable.Loading)

    /** What was sent, in order: "reorder 1 [3, 1, 2]", "remove 1 [7]", "rename 1 Road trip"… */
    val sent = mutableListOf<String>()

    /** The next changes fail with this. */
    var fail: Exception? = null

    var gate: CompletableDeferred<Unit>? = null

    private var nextId = 100L

    fun detail(id: Long): MutableStateFlow<Loadable<PlaylistDetail>> = details.getOrPut(id) { MutableStateFlow(Loadable.Loading) }

    fun set(detail: PlaylistDetail) {
        detail(detail.id).value = Loadable.Ready(detail, fresh = true)
    }

    private fun current(id: Long): PlaylistDetail = (detail(id).value as Loadable.Ready).value

    override fun playlists(): Flow<Loadable<List<PlaylistSummary>>> = list

    override fun playlist(playlistId: Long): Flow<Loadable<PlaylistDetail>> = detail(playlistId)

    override suspend fun create(title: String, kind: PlaylistKind?, itemIds: List<Long>): Long {
        sent += "create $title $kind $itemIds"
        server()
        return nextId++
    }

    override suspend fun createSmart(title: String, libraryId: Long, query: String): Long {
        sent += "smart $title $libraryId $query"
        server()
        return nextId++
    }

    override suspend fun rename(playlistId: Long, title: String) {
        sent += "rename $playlistId $title"
        change(playlistId) { it.copy(title = title) }
    }

    override suspend fun delete(playlistId: Long) {
        sent += "delete $playlistId"
        server()
    }

    override suspend fun add(playlistId: Long, itemIds: List<Long>) {
        sent += "add $playlistId $itemIds"
        server()
    }

    override suspend fun remove(playlistId: Long, entryIds: List<Long>) {
        sent += "remove $playlistId $entryIds"
        change(playlistId) { d -> d.copy(entries = d.entries.filterNot { it.entryId in entryIds }, itemCount = d.itemCount - entryIds.size) }
    }

    override suspend fun reorder(playlistId: Long, entryIds: List<Long>) {
        sent += "reorder $playlistId $entryIds"
        change(playlistId) { d ->
            val byId = d.entries.associateBy { it.entryId }
            val first = entryIds.mapNotNull { byId[it] }
            d.copy(entries = first + d.entries.filterNot { it.entryId in entryIds })
        }
    }

    /** The change shows at once; it is undone if the server refuses. */
    private suspend fun change(id: Long, transform: (PlaylistDetail) -> PlaylistDetail) {
        val before = current(id)
        set(transform(before))
        try {
            server()
        } catch (e: Exception) {
            set(before)
            throw e
        }
    }

    private suspend fun server() {
        gate?.await()
        fail?.let { throw it }
    }
}

fun episode(id: Long, title: String, show: String, season: Int, number: Int, durationMs: Long = 3_000_000, positionMs: Long = 0) = ItemSummary(
    id = id,
    libraryId = 2,
    kind = ItemKind.Episode,
    title = title,
    index = number,
    parentIndex = season,
    parentId = 900,
    grandparentId = 901,
    grandparentTitle = show,
    durationMs = durationMs,
    userState = UserState(positionMs = positionMs),
)

fun film(id: Long, title: String, year: Int = 2024, durationMs: Long = 6_000_000) =
    ItemSummary(id = id, libraryId = 1, kind = ItemKind.Movie, title = title, year = year, durationMs = durationMs)

fun track(id: Long, title: String, album: Long = 50) =
    ItemSummary(id = id, libraryId = 3, kind = ItemKind.Track, title = title, parentId = album, grandparentTitle = "Harbour Lights", durationMs = 200_000)

/** A playlist of [items], entries numbered from 1 in that order. */
fun playlist(id: Long, title: String, vararg items: ItemSummary, kind: PlaylistKind = PlaylistKind.Video, smart: String? = null) = PlaylistDetail(
    id = id,
    title = title,
    kind = kind,
    itemCount = items.size,
    durationMs = items.sumOf { it.durationMs ?: 0 },
    smartFilter = smart,
    libraryId = smart?.let { 1 },
    entries = items.mapIndexed { i, item -> PlaylistEntry(entryId = i + 1L, item = item) },
)
