package app.pley.android.network

import app.pley.android.model.EntryIds
import app.pley.android.model.ImageKind
import app.pley.android.model.ItemIds
import app.pley.android.model.LibrarySort
import app.pley.android.model.NewPlaylist
import app.pley.android.model.PlaybackRequest
import app.pley.android.model.PlaylistPatch
import app.pley.android.model.PleyJson
import app.pley.android.model.ProgressReport
import app.pley.android.model.RatingRequest
import app.pley.android.model.SortOrder
import app.pley.android.model.StreamChoice
import app.pley.android.testing.Fixtures
import app.pley.android.testing.network.MockPley
import app.pley.android.testing.network.bodyText
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * A golden request per PleyApi method: the request line (path and query,
 * in their fixed order), the body, and the headers every request carries.
 */
class HttpPleyApiTest {
    @get:Rule val folder = TemporaryFolder()

    private lateinit var server: MockWebServer
    private lateinit var issuer: String
    private lateinit var api: PleyApi

    @Before
    fun setUp() {
        server = MockPley.server()
        issuer = MockPley.issuerOf(server)
        val tokens = testTokens(issuer)
        api = testGraph(folder.root, { tokens }).api(issuer, userId = 2)
    }

    @After
    fun tearDown() {
        server.close()
    }

    /** One method: what the server answers, what the app sends. */
    private class Golden(
        val name: String,
        val answer: MockResponse,
        val line: String,
        val body: String? = null,
        val call: suspend PleyApi.() -> Any?,
    )

    private fun json(path: String) = MockPley.fixture(path)

    private inline fun <reified T> request(path: String): T = Fixtures.decode(path)

    private fun goldens(): List<Golden> {
        val playback = request<PlaybackRequest>("playback/request.json")
        val progress = request<ProgressReport>("playback/progress_request.json")
        val streams = request<StreamChoice>("playback/streams_request.json")
        val rating = request<RatingRequest>("playback/rating_request.json")
        val create = request<NewPlaylist>("playlists/create_request.json")
        val patch = request<PlaylistPatch>("playlists/patch_request.json")
        val add = request<ItemIds>("playlists/add_items_request.json")
        val order = request<EntryIds>("playlists/order_request.json")
        return listOf(
            Golden("health", json("server/health.json"), "GET /api/health") { health() },
            Golden("server", json("server/info.json"), "GET /api/server") { server() },
            Golden("me", json("me/app_token.json"), "GET /api/auth/me") { me() },
            Golden("libraries", json("libraries/list.json"), "GET /api/libraries") { libraries() },
            Golden("library", json("libraries/library.json"), "GET /api/libraries/2") { library(2) },
            Golden("libraryItems (defaults)", json("libraries/items.json"), "GET /api/libraries/1/items?sort=title&offset=0") {
                libraryItems(1)
            },
            Golden(
                "libraryItems (every parameter)",
                json("libraries/items.json"),
                "GET /api/libraries/1/items?sort=added&order=desc&watched=false&genre=Science%20Fiction&year=2016&offset=200&limit=1000",
            ) {
                libraryItems(1, LibrarySort.Added, SortOrder.Descending, watched = false, genre = "Science Fiction", year = 2016, offset = 200, limit = 1000)
            },
            Golden("libraryFilters", json("libraries/filters.json"), "GET /api/libraries/1/filters") { libraryFilters(1) },
            Golden("libraryHubs", json("libraries/hubs.json"), "GET /api/libraries/1/hubs") { libraryHubs(1) },
            Golden("libraryCollections", json("libraries/collections.json"), "GET /api/libraries/1/collections") { libraryCollections(1) },
            Golden("collection", json("collections/detail.json"), "GET /api/collections/801") { collection(801) },
            Golden("playlists", json("playlists/list.json"), "GET /api/playlists") { playlists() },
            Golden("playlist", json("playlists/detail.json"), "GET /api/playlists/901") { playlist(901) },
            Golden("item", json("items/movie_detail.json"), "GET /api/items/101") { item(101) },
            Golden("children", json("items/children_seasons.json"), "GET /api/items/201/children") { children(201) },
            Golden("onDeck", json("items/on_deck.json"), "GET /api/items/201/on-deck") { onDeck(201) },
            Golden("next", json("items/next_none.json"), "GET /api/items/203/next") { next(203) },
            Golden("related", json("items/related.json"), "GET /api/items/101/related?limit=20") { related(101) },
            Golden("itemCollections", json("items/collections.json"), "GET /api/items/101/collections") { itemCollections(101) },
            Golden("person", json("people/detail.json"), "GET /api/people/706") { person(706) },
            Golden("history (defaults)", json("history/page.json"), "GET /api/history?offset=0&limit=50") { history() },
            Golden(
                "history (every parameter)",
                json("history/page.json"),
                "GET /api/history?offset=50&limit=200&item_id=201&library_id=2&since=1761000000",
            ) { history(offset = 50, limit = 200, itemId = 201, libraryId = 2, since = 1761000000) },
            Golden("homeHubs", json("hubs/home.json"), "GET /api/hubs/home") { homeHubs() },
            Golden("watchlist", json("watchlist/list.json"), "GET /api/watchlist") { watchlist() },
            Golden("search", json("search/results.json"), "GET /api/search?q=blade%20runner%20%26%202049&limit=60") {
                search("blade runner & 2049")
            },
            Golden(
                "startPlayback",
                json("playback/info_transcode.json"),
                "POST /api/playback",
                body = PleyJson.encodeToString(PlaybackRequest.serializer(), playback),
            ) { startPlayback(playback) },
            Golden("playbackStatus", json("playback/status_transcode.json"), "GET /api/playback/6f1c2a9e") { playbackStatus("6f1c2a9e") },
            Golden("stopPlayback", MockPley.noContent(), "DELETE /api/playback/6f1c2a9e") { stopPlayback("6f1c2a9e") },
            Golden(
                "subtitles",
                MockResponse.Builder().setHeader("Content-Type", "text/vtt").body("WEBVTT\n").build(),
                "GET /api/files/601/subtitles/3.vtt",
            ) { subtitles(601, 3) },
            Golden("trickplay", json("trickplay/info.json"), "GET /api/files/601/trickplay") { trickplay(601) },
            Golden(
                "reportProgress",
                MockPley.noContent(),
                "POST /api/items/101/progress",
                body = PleyJson.encodeToString(ProgressReport.serializer(), progress),
            ) { reportProgress(101, progress) },
            Golden("markWatched", MockPley.noContent(), "POST /api/items/101/watched", body = "") { markWatched(101) },
            Golden("markUnwatched", MockPley.noContent(), "POST /api/items/101/unwatched", body = "") { markUnwatched(101) },
            Golden(
                "setStreams",
                MockPley.noContent(),
                "PUT /api/items/203/streams",
                body = PleyJson.encodeToString(StreamChoice.serializer(), streams),
            ) { setStreams(203, streams) },
            Golden(
                "setRating",
                MockPley.noContent(),
                "PUT /api/items/101/rating",
                body = PleyJson.encodeToString(RatingRequest.serializer(), rating),
            ) { setRating(101, rating) },
            Golden("deleteHistoryEntry", MockPley.noContent(), "DELETE /api/history/5") { deleteHistoryEntry(5) },
            Golden("hideFromContinueWatching", MockPley.noContent(), "DELETE /api/continue-watching/204") { hideFromContinueWatching(204) },
            Golden("addToWatchlist", MockPley.noContent(), "PUT /api/watchlist/201", body = "") { addToWatchlist(201) },
            Golden("removeFromWatchlist", MockPley.noContent(), "DELETE /api/watchlist/201") { removeFromWatchlist(201) },
            Golden(
                "createPlaylist",
                json("playlists/detail.json"),
                "POST /api/playlists",
                body = PleyJson.encodeToString(NewPlaylist.serializer(), create),
            ) { createPlaylist(create) },
            Golden(
                "updatePlaylist",
                json("playlists/detail.json"),
                "PATCH /api/playlists/901",
                body = PleyJson.encodeToString(PlaylistPatch.serializer(), patch),
            ) { updatePlaylist(901, patch) },
            Golden("deletePlaylist", MockPley.noContent(), "DELETE /api/playlists/901") { deletePlaylist(901) },
            Golden(
                "addToPlaylist",
                json("playlists/added.json"),
                "POST /api/playlists/901/items",
                body = PleyJson.encodeToString(ItemIds.serializer(), add),
            ) { addToPlaylist(901, add) },
            Golden("removeFromPlaylist", MockPley.noContent(), "DELETE /api/playlists/901/items/7") { removeFromPlaylist(901, 7) },
            Golden(
                "reorderPlaylist",
                json("playlists/detail.json"),
                "PUT /api/playlists/901/order",
                body = PleyJson.encodeToString(EntryIds.serializer(), order),
            ) { reorderPlaylist(901, order) },
        )
    }

    @Test
    fun everyMethodSendsItsGoldenRequest() = runTest {
        for (golden in goldens()) {
            server.enqueue(golden.answer)
            golden.call(api)
            val request = server.takeRequest(5, TimeUnit.SECONDS) ?: error("${golden.name}: no request")
            assertEquals(golden.name, "${golden.line} HTTP/1.1", request.requestLine)
            assertEquals("${golden.name}: user agent", TEST_AGENT, request.headers["User-Agent"])
            assertEquals("${golden.name}: token", "Bearer pley_at_one", request.headers["Authorization"])
            if (golden.body != null) assertEquals("${golden.name}: body", golden.body, request.bodyText)
            if (golden.body != null && golden.body.isNotEmpty()) {
                assertEquals("${golden.name}: content type", "application/json; charset=utf-8", request.headers["Content-Type"])
            }
        }
    }

    @Test
    fun requestBodiesAreTheFixturesTheyCameFrom() {
        // Decoded from docs/API.md's shapes and written back: nothing lost, nothing renamed.
        for ((path, serializer) in listOf(
            "playback/progress_request.json" to ProgressReport.serializer(),
            "playback/streams_request.json" to StreamChoice.serializer(),
            "playback/rating_request.json" to RatingRequest.serializer(),
            "playlists/add_items_request.json" to ItemIds.serializer(),
            "playlists/order_request.json" to EntryIds.serializer(),
        )) {
            @Suppress("UNCHECKED_CAST")
            val s = serializer as kotlinx.serialization.KSerializer<Any>
            val written = PleyJson.encodeToString(s, PleyJson.decodeFromString(s, Fixtures.text(path)))
            assertEquals(path, Json.parseToJsonElement(Fixtures.text(path)), Json.parseToJsonElement(written))
        }
    }

    @Test
    fun answersAreDecoded() = runTest {
        server.enqueue(MockPley.fixture("items/movie_detail.json"))
        val movie = api.item(101)
        assertEquals("Arrival", movie.title)

        server.enqueue(MockPley.fixture("libraries/list.json"))
        assertEquals(listOf(1L, 2L, 3L), api.libraries().map { it.id })
    }

    @Test
    fun subtitlesSayWhetherMoreAreComing() = runTest {
        server.enqueue(
            MockResponse.Builder().setHeader("Content-Type", "text/vtt").setHeader("X-Pley-Complete", "0").body("WEBVTT\n\npart").build(),
        )
        val partial = api.subtitles(601, 3)
        assertEquals(SubtitleText("WEBVTT\n\npart", complete = false), partial)

        server.enqueue(
            MockResponse.Builder().setHeader("Content-Type", "text/vtt").setHeader("X-Pley-Complete", "1").body("WEBVTT\n\nall").build(),
        )
        assertEquals(true, api.subtitles(601, 3).complete)

        // A sidecar or a cached track: no header, and complete.
        server.enqueue(MockResponse.Builder().setHeader("Content-Type", "text/vtt").body("WEBVTT\n").build())
        assertEquals(true, api.subtitles(601, 3).complete)
    }

    @Test
    fun seekPreviewsNotMadeYetAreNull() = runTest {
        server.enqueue(MockPley.error(404, "not_found", "seek previews are not ready yet"))
        assertNull(api.trickplay(601))
    }

    @Test
    fun addressesAreAbsoluteAndCarryNoToken() {
        assertEquals("$issuer/api/users/2/avatar?v=abc", api.avatarUrl(2, "abc"))
        assertEquals("$issuer/api/users/2/avatar", api.avatarUrl(2))
        assertEquals("$issuer/api/items/101/images/poster?v=17&w=480", api.itemImageUrl(101, ImageKind.Poster, width = 400, version = "17"))
        assertEquals("$issuer/api/items/203/images/thumb?w=1920", api.itemImageUrl(203, ImageKind.Thumb, width = 5000))
        assertEquals("$issuer/api/collections/801/images/backdrop?v=9&w=1280", api.collectionImageUrl(801, ImageKind.Backdrop, 1280, "9"))
        assertEquals("$issuer/api/people/706/photo?w=160", api.personPhotoUrl(706, width = 100))
        assertEquals("$issuer/api/playback/abc/index.m3u8", api.playbackFileUrl("abc"))
        assertEquals("$issuer/api/playback/abc/7.ts", api.playbackFileUrl("abc", "7.ts"))
        assertEquals("$issuer/api/files/601/stream", api.fileStreamUrl(601))
        assertEquals("$issuer/api/files/601/trickplay/2.jpg?v=t1", api.trickplaySheetUrl(601, 2, "t1"))
        assertEquals("$issuer/api/files/601/chapters/0.jpg", api.chapterImageUrl(601, 0))
        // The server's own addresses, made absolute and sized.
        assertEquals("$issuer/api/items/12/images/poster?v=1729600000", api.url("/api/items/12/images/poster?v=1729600000"))
        assertEquals("$issuer/api/items/12/images/poster?v=1729600000&w=320", api.url("/api/items/12/images/poster?v=1729600000", width = 300))
        assertEquals("https://elsewhere.example/x.jpg", api.url("https://elsewhere.example/x.jpg"))
    }

    @Test(expected = IllegalArgumentException::class)
    fun collectionsHaveNoThumb() {
        api.collectionImageUrl(801, ImageKind.Thumb)
    }
}
