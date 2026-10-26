package app.pley.android.fixtures

import app.pley.android.model.HubKeys
import app.pley.android.model.Hubs
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemList
import app.pley.android.model.ServerEvent
import okhttp3.Request
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** My List and leaving Continue Watching, as the app's contract has them. */
class ListRoutesTest {
    private fun Client.list(): List<Long> = read("/api/watchlist", ItemList.serializer()).items.map { it.id }

    private fun Client.home(): Hubs = read("/api/hubs/home", Hubs.serializer())

    private fun Hubs.ids(key: String): List<Long>? = hubs.firstOrNull { it.key == key }?.items?.map { it.id }

    private fun Client.inList(id: Long): Boolean = read("/api/items/$id", ItemDetail.serializer()).inWatchlist

    @Test
    fun alexsListIsOnHomeAfterContinueWatching() = withServer { server ->
        val app = Client(server, server.accessToken())
        val list = app.list()
        assertEquals(5, list.size)
        // Newest added first.
        assertEquals(17L, list.first())
        assertEquals(15L, list.last())
        val home = app.home()
        assertEquals(listOf(HubKeys.CONTINUE_WATCHING, HubKeys.MY_LIST), home.hubs.map { it.key }.take(2))
        assertEquals(list, home.ids(HubKeys.MY_LIST))
        assertTrue(app.inList(17))
        assertFalse(app.inList(FixtureIds.SAMPLE_1080P))
        // Sam has none: no row.
        val sam = Client(server, server.accessToken(FixtureIds.SAM))
        assertNull(sam.home().ids(HubKeys.MY_LIST))
    }

    @Test
    fun addingAndRemovingMoviesAndShows() = withServer { server ->
        val app = Client(server, server.accessToken())
        val movie = FixtureIds.SAMPLE_1080P
        assertEquals(204, app.send("PUT", "/api/watchlist/$movie").code)
        assertEquals(204, app.send("PUT", "/api/watchlist/$movie").code)
        assertEquals(listOf(movie), app.list().filter { it == movie })
        assertEquals(movie, app.list().first())
        assertTrue(app.inList(movie))

        assertEquals(204, app.send("DELETE", "/api/watchlist/$movie").code)
        assertFalse(movie in app.list())
        assertFalse(app.inList(movie))
        assertEquals(204, app.send("DELETE", "/api/watchlist/$movie").code)

        assertEquals(400, app.send("PUT", "/api/watchlist/${FixtureIds.SHOW_SEASON_1}").code)
        assertEquals(400, app.send("PUT", "/api/watchlist/${FixtureIds.SHOW_S1E1}").code)
        assertEquals(404, app.send("PUT", "/api/watchlist/999999999").code)
        assertEquals(2, server.journal.writes("PUT", "/api/watchlist/$movie").size)
    }

    @Test
    fun aMovieLeavesOnceWatchedButAShowStays() = withServer { server ->
        val app = Client(server, server.accessToken())
        val movie = FixtureIds.SAMPLE_1080P
        app.send("PUT", "/api/watchlist/$movie")
        // Played to the end.
        app.send("POST", "/api/items/$movie/progress", """{"position_ms":20000,"state":"stopped"}""")
        assertFalse(movie in app.list())

        // Marked watched by hand.
        app.send("POST", "/api/items/15/watched")
        assertFalse(15L in app.list())

        app.send("POST", "/api/items/${FixtureIds.SHOW}/watched")
        assertTrue(FixtureIds.SHOW in app.list())
    }

    @Test
    fun anEpisodeLeavesContinueWatchingWithItsShowUntilItIsPlayedAgain() {
        val clock = FakeClock()
        withServer(FixtureServer.Options(scanEveryMs = 0, clock = clock)) { server ->
            val app = Client(server, server.accessToken())
            val episode = FixtureIds.SHOW_S1E2
            assertTrue(episode in app.home().ids(HubKeys.CONTINUE_WATCHING)!!)

            assertEquals(204, app.send("DELETE", "/api/continue-watching/$episode").code)
            assertFalse(episode in app.home().ids(HubKeys.CONTINUE_WATCHING)!!)
            // The library's row goes too; the progress stays.
            assertNull(app.read("/api/libraries/${FixtureIds.SHOWS}/hubs", Hubs.serializer()).ids("continue_watching:${FixtureIds.SHOWS}"))
            assertEquals(9_000L, app.read("/api/items/$episode", ItemDetail.serializer()).userState.positionMs)

            // Another episode of the show played: the show is back, as it would have been.
            clock.now += 5_000
            app.send("POST", "/api/items/${FixtureIds.SHOW_S1E3}/progress", """{"position_ms":3000,"state":"playing"}""")
            val back = app.home().ids(HubKeys.CONTINUE_WATCHING)!!
            assertTrue(episode in back)
            assertTrue(FixtureIds.SHOW_S1E3 in back)
        }
    }

    @Test
    fun aMovieLeavesContinueWatchingUntilItIsPlayedAgain() {
        val clock = FakeClock()
        withServer(FixtureServer.Options(scanEveryMs = 0, clock = clock)) { server ->
            val app = Client(server, server.accessToken())
            val movie = app.home().ids(HubKeys.CONTINUE_WATCHING)!!.first { it < 100 }
            val position = app.read("/api/items/$movie", ItemDetail.serializer()).userState.positionMs

            assertEquals(204, app.send("DELETE", "/api/continue-watching/$movie").code)
            assertFalse(movie in app.home().ids(HubKeys.CONTINUE_WATCHING)!!)
            assertEquals(position, app.read("/api/items/$movie", ItemDetail.serializer()).userState.positionMs)
            assertEquals(404, app.send("DELETE", "/api/continue-watching/999999999").code)

            clock.now += 5_000
            app.send("POST", "/api/items/$movie/progress", """{"position_ms":${position + 1_000},"state":"playing"}""")
            assertEquals(movie, app.home().ids(HubKeys.CONTINUE_WATCHING)!!.first())
        }
    }

    @Test
    fun myListChangesTellTheProfilesStreams() = withServer { server ->
        val app = Client(server, server.accessToken())
        val request = Request.Builder().url("${server.url}/api/events").header("Authorization", "Bearer ${server.accessToken()}").build()
        app.http.newCall(request).execute().use { response ->
            val source = response.body.source()
            assertEquals(": connected", source.readUtf8Line())
            app.send("PUT", "/api/watchlist/${FixtureIds.SAMPLE_1080P}")
            var line = source.readUtf8Line()
            while (line != null && !line.startsWith("data:")) line = source.readUtf8Line()
            assertEquals(ServerEvent.WatchlistChanged, ServerEvent.parse(line!!.removePrefix("data: ")))
        }
    }

    @Test
    fun watchStateChangesTellTheProfilesStreams() = withServer { server ->
        val app = Client(server, server.accessToken())
        val request = Request.Builder().url("${server.url}/api/events").header("Authorization", "Bearer ${server.accessToken()}").build()
        app.http.newCall(request).execute().use { response ->
            val source = response.body.source()
            assertEquals(": connected", source.readUtf8Line())
            // The next user_state_changed, past the libraries' changes that come with marks.
            fun next(): ServerEvent = generateSequence {
                var line = source.readUtf8Line()
                while (line != null && !line.startsWith("data:")) line = source.readUtf8Line()
                ServerEvent.parse(line!!.removePrefix("data: "))
            }.first { it is ServerEvent.UserStateChanged }
            // A film of an hour or two Alex hasn't played (the samples are short).
            val movie = 100L

            app.send("POST", "/api/items/$movie/watched")
            assertEquals(ServerEvent.UserStateChanged(listOf(movie)), next())
            app.send("POST", "/api/items/${FixtureIds.SHOW_SEASON_1}/unwatched")
            assertEquals(ServerEvent.UserStateChanged(listOf(FixtureIds.SHOW_SEASON_1, FixtureIds.SHOW), children = true), next())
            // Played again: a resume point starts.
            app.send("POST", "/api/items/$movie/progress", """{"position_ms":30000,"state":"playing"}""")
            assertEquals(ServerEvent.UserStateChanged(listOf(movie)), next())
            app.send("DELETE", "/api/continue-watching/$movie")
            assertEquals(ServerEvent.UserStateChanged(listOf(movie)), next())
        }
    }
}
