package app.pley.android.testing.network

import app.pley.android.model.Added
import app.pley.android.model.CollectionDetail
import app.pley.android.model.CollectionList
import app.pley.android.model.EntryIds
import app.pley.android.model.Health
import app.pley.android.model.HistoryPage
import app.pley.android.model.Hubs
import app.pley.android.model.ImageKind
import app.pley.android.model.ItemCollections
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemIds
import app.pley.android.model.ItemList
import app.pley.android.model.ItemsPage
import app.pley.android.model.Library
import app.pley.android.model.LibraryFilters
import app.pley.android.model.LibrarySort
import app.pley.android.model.MaybeItem
import app.pley.android.model.Me
import app.pley.android.model.NewPlaylist
import app.pley.android.model.PersonDetail
import app.pley.android.model.PlaybackInfo
import app.pley.android.model.PlaybackMode
import app.pley.android.model.PlaybackRequest
import app.pley.android.model.PlaylistDetail
import app.pley.android.model.PlaylistList
import app.pley.android.model.PlaylistPatch
import app.pley.android.model.ProgressReport
import app.pley.android.model.RatingRequest
import app.pley.android.model.SearchResults
import app.pley.android.model.ServerInfo
import app.pley.android.model.SessionStatus
import app.pley.android.model.SortOrder
import app.pley.android.model.StreamChoice
import app.pley.android.model.TrickplayInfo
import app.pley.android.network.HttpPleyApi
import app.pley.android.network.PleyApi
import app.pley.android.network.SubtitleText
import app.pley.android.testing.Fixtures
import kotlinx.coroutines.CompletableDeferred

/**
 * [PleyApi] without a network, answering from the JSON fixtures (the
 * household of [Fixtures]: Arrival 101, Severance 201 with season 202 and
 * episodes 203 to 205, the track 303, collections 801 and 803, playlists
 * 901 and 902). Every call is recorded in [calls]; [answer] and [fail]
 * change what a method gives, [hold] keeps calls waiting until released.
 * Addresses are built as [HttpPleyApi] builds them, on [issuer].
 */
class FakePleyApi(val issuer: String = "http://127.0.0.1:7539") : PleyApi {
    /** A call: the method's name (`"libraryItems"`) and its arguments in order. */
    data class Call(val method: String, val args: List<Any?>)

    private val recorded = mutableListOf<Call>()
    private val answers = mutableMapOf<String, Any?>()
    private val failures = mutableMapOf<String, Throwable>()
    private var gate: CompletableDeferred<Unit>? = null

    /** The calls so far, oldest first. */
    val calls: List<Call> get() = synchronized(recorded) { recorded.toList() }

    /** The calls of [method]. */
    fun calls(method: String): List<Call> = calls.filter { it.method == method }

    /** [method] answers [value] from now on (its type must be the method's). */
    fun answer(method: String, value: Any?) = synchronized(recorded) { answers[method] = value }

    /** [method] fails with [error] from now on; `null` for every method. */
    fun fail(method: String?, error: Throwable) = synchronized(recorded) { failures[method ?: ANY] = error }

    /** Answers from the fixtures again, and forgets the calls. */
    fun reset() = synchronized(recorded) {
        recorded.clear()
        answers.clear()
        failures.clear()
        gate?.complete(Unit)
        gate = null
    }

    /** Calls from now on wait until [release]: for what a screen shows while loading. */
    fun hold() = synchronized(recorded) { if (gate == null) gate = CompletableDeferred() }

    fun release() = synchronized(recorded) {
        gate?.complete(Unit)
        gate = null
    }

    private val urls = HttpPleyApi(issuer, { error("FakePleyApi has no network") })

    @Suppress("UNCHECKED_CAST")
    private suspend fun <T> respond(method: String, vararg args: Any?, fixture: () -> T): T {
        val (waitFor, failure, answer) = synchronized(recorded) {
            recorded += Call(method, args.toList())
            Triple(gate, failures[method] ?: failures[ANY], if (method in answers) Answer(answers[method]) else null)
        }
        waitFor?.await()
        if (failure != null) throw failure
        return if (answer != null) answer.value as T else fixture()
    }

    private class Answer(val value: Any?)

    private inline fun <reified T> fixture(path: String): T = Fixtures.decode(path)

    // ---------------------------------------------------------------- server

    override suspend fun health(): Health = respond("health") { fixture("server/health.json") }

    override suspend fun server(): ServerInfo = respond("server") { fixture("server/info.json") }

    override suspend fun me(): Me = respond("me") { fixture("me/app_token.json") }

    override fun avatarUrl(userId: Long, version: String?): String = urls.avatarUrl(userId, version)

    override fun url(path: String, width: Int?): String = urls.url(path, width)

    // ---------------------------------------------------------------- libraries

    override suspend fun libraries(): List<Library> = respond("libraries") { fixture("libraries/list.json") }

    override suspend fun library(id: Long): Library = respond("library", id) {
        fixture<List<Library>>("libraries/list.json").firstOrNull { it.id == id } ?: fixture<Library>("libraries/library.json").copy(id = id)
    }

    override suspend fun libraryItems(
        libraryId: Long,
        sort: LibrarySort,
        order: SortOrder?,
        watched: Boolean?,
        genre: String?,
        year: Int?,
        offset: Int,
        limit: Int?,
    ): ItemsPage = respond("libraryItems", libraryId, sort, order, watched, genre, year, offset, limit) {
        val page = fixture<ItemsPage>(if (sort == LibrarySort.Bitrate) "libraries/items_by_bitrate.json" else "libraries/items.json")
        val items = page.items.drop(offset)
        page.copy(items = if (limit != null) items.take(limit) else items)
    }

    override suspend fun libraryFilters(libraryId: Long): LibraryFilters =
        respond("libraryFilters", libraryId) { fixture("libraries/filters.json") }

    override suspend fun libraryHubs(libraryId: Long): Hubs = respond("libraryHubs", libraryId) { fixture("libraries/hubs.json") }

    override suspend fun libraryCollections(libraryId: Long): CollectionList =
        respond("libraryCollections", libraryId) { fixture("libraries/collections.json") }

    override suspend fun collection(id: Long): CollectionDetail = respond("collection", id) {
        fixture(if (id == 803L) "collections/smart_detail.json" else "collections/detail.json")
    }

    override fun collectionImageUrl(id: Long, kind: ImageKind, width: Int?, version: String?): String =
        urls.collectionImageUrl(id, kind, width, version)

    // ---------------------------------------------------------------- playlists (reading)

    override suspend fun playlists(): PlaylistList = respond("playlists") { fixture("playlists/list.json") }

    override suspend fun playlist(id: Long): PlaylistDetail = respond("playlist", id) {
        fixture(if (id == 902L) "playlists/smart_detail.json" else "playlists/detail.json")
    }

    // ---------------------------------------------------------------- items

    override suspend fun item(id: Long): ItemDetail = respond("item", id) {
        when (id) {
            101L -> fixture("items/movie_detail.json")
            102L -> fixture("items/extra.json")
            201L -> fixture("items/show_detail.json")
            203L -> fixture("items/episode_detail.json")
            303L -> fixture("items/track_detail.json")
            else -> fixture<ItemDetail>("items/minimal_detail.json").copy(id = id)
        }
    }

    override suspend fun children(id: Long): ItemList = respond("children", id) {
        when (id) {
            201L -> fixture("items/children_seasons.json")
            202L -> fixture("items/children_episodes.json")
            else -> ItemList()
        }
    }

    override suspend fun onDeck(id: Long): MaybeItem = respond("onDeck", id) { fixture("items/on_deck.json") }

    override suspend fun next(id: Long): MaybeItem = respond("next", id) { fixture("items/next_none.json") }

    override suspend fun related(id: Long, limit: Int): ItemList = respond("related", id, limit) {
        val list = fixture<ItemList>("items/related.json")
        list.copy(items = list.items.take(limit))
    }

    override fun itemImageUrl(id: Long, kind: ImageKind, width: Int?, version: String?): String =
        urls.itemImageUrl(id, kind, width, version)

    override suspend fun itemCollections(id: Long): ItemCollections =
        respond("itemCollections", id) { fixture("items/collections.json") }

    override suspend fun person(id: Long): PersonDetail = respond("person", id) { fixture("people/detail.json") }

    override fun personPhotoUrl(id: Long, width: Int?, version: String?): String = urls.personPhotoUrl(id, width, version)

    override suspend fun history(offset: Int, limit: Int, itemId: Long?, libraryId: Long?, since: Long?): HistoryPage =
        respond("history", offset, limit, itemId, libraryId, since) { fixture("history/page.json") }

    override suspend fun homeHubs(): Hubs = respond("homeHubs") { fixture("hubs/home.json") }

    override suspend fun search(query: String, limit: Int): SearchResults =
        respond("search", query, limit) { fixture("search/results.json") }

    // ---------------------------------------------------------------- playback

    override suspend fun startPlayback(request: PlaybackRequest): PlaybackInfo = respond("startPlayback", request) {
        val hls = request.mode == PlaybackMode.Remux || request.mode == PlaybackMode.Transcode
        fixture(if (hls) "playback/info_transcode.json" else "playback/info_direct.json")
    }

    override suspend fun playbackStatus(sessionId: String): SessionStatus = respond("playbackStatus", sessionId) {
        val transcode = fixture<SessionStatus>("playback/status_transcode.json")
        if (sessionId == transcode.sessionId) transcode else fixture<SessionStatus>("playback/status_direct.json").copy(sessionId = sessionId)
    }

    override suspend fun stopPlayback(sessionId: String) = respond("stopPlayback", sessionId) { }

    override fun playbackFileUrl(sessionId: String, file: String): String = urls.playbackFileUrl(sessionId, file)

    override fun fileStreamUrl(fileId: Long): String = urls.fileStreamUrl(fileId)

    override suspend fun subtitles(fileId: Long, index: Int): SubtitleText =
        respond("subtitles", fileId, index) { SubtitleText(SUBTITLES, complete = true) }

    override suspend fun trickplay(fileId: Long): TrickplayInfo? = respond("trickplay", fileId) { fixture<TrickplayInfo>("trickplay/info.json") }

    override fun trickplaySheetUrl(fileId: Long, sheet: Int, version: String): String = urls.trickplaySheetUrl(fileId, sheet, version)

    override fun chapterImageUrl(fileId: Long, chapter: Int, version: String?): String = urls.chapterImageUrl(fileId, chapter, version)

    override suspend fun reportProgress(itemId: Long, report: ProgressReport) = respond("reportProgress", itemId, report) { }

    override suspend fun markWatched(itemId: Long) = respond("markWatched", itemId) { }

    override suspend fun markUnwatched(itemId: Long) = respond("markUnwatched", itemId) { }

    override suspend fun setStreams(itemId: Long, choice: StreamChoice) = respond("setStreams", itemId, choice) { }

    override suspend fun setRating(itemId: Long, rating: RatingRequest) = respond("setRating", itemId, rating) { }

    override suspend fun deleteHistoryEntry(id: Long) = respond("deleteHistoryEntry", id) { }

    // ---------------------------------------------------------------- playlists (changing)

    override suspend fun createPlaylist(playlist: NewPlaylist): PlaylistDetail = respond("createPlaylist", playlist) {
        val detail = fixture<PlaylistDetail>(if (playlist.smartFilter != null) "playlists/smart_detail.json" else "playlists/detail.json")
        detail.copy(title = playlist.title)
    }

    override suspend fun updatePlaylist(id: Long, patch: PlaylistPatch): PlaylistDetail = respond("updatePlaylist", id, patch) {
        val detail = fixture<PlaylistDetail>(if (id == 902L) "playlists/smart_detail.json" else "playlists/detail.json")
        detail.copy(title = patch.title ?: detail.title, smartFilter = patch.smartFilter ?: detail.smartFilter)
    }

    override suspend fun deletePlaylist(id: Long) = respond("deletePlaylist", id) { }

    override suspend fun addToPlaylist(id: Long, items: ItemIds): Added = respond("addToPlaylist", id, items) { fixture("playlists/added.json") }

    override suspend fun removeFromPlaylist(id: Long, entryId: Long) = respond("removeFromPlaylist", id, entryId) { }

    override suspend fun reorderPlaylist(id: Long, entries: EntryIds): PlaylistDetail = respond("reorderPlaylist", id, entries) {
        val detail = fixture<PlaylistDetail>("playlists/detail.json")
        val first = entries.entryIds.mapNotNull { e -> detail.entries.firstOrNull { it.entryId == e } }
        detail.copy(entries = first + detail.entries.filter { it !in first })
    }

    companion object {
        private const val ANY = "*"

        /** What [subtitles] gives: a short WebVTT track. */
        const val SUBTITLES = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nThe universe is not made of atoms.\n"
    }
}
