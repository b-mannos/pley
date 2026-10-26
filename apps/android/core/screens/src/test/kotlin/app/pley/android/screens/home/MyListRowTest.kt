package app.pley.android.screens.home

import app.pley.android.model.Hub
import app.pley.android.model.HubKeys
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.UserState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

/** Home's My List: a row of posters after Continue Watching, as the server sends it. */
class MyListRowTest {
    private val episode = ItemSummary(
        id = 204, libraryId = 2, kind = ItemKind.Episode, title = "Half Loop", grandparentId = 201,
        durationMs = 3_209_000, userState = UserState(positionMs = 1_185_000),
    )
    private val show = ItemSummary(id = 201, libraryId = 2, kind = ItemKind.Show, title = "Severance")
    private val movie = ItemSummary(id = 104, libraryId = 1, kind = ItemKind.Movie, title = "Dune: Part Two", addedAt = 1_761_300_000)
    private val continueWatching = Hub(HubKeys.CONTINUE_WATCHING, "Continue Watching", null, listOf(episode))
    private val myList = Hub(HubKeys.MY_LIST, "My List", null, listOf(show))
    private val recent = Hub("recently_added:1", "Recently Added in Movies", 1, listOf(movie))

    @Test
    fun `My List is a row of posters in the server's place, without See all`() {
        val rows = homeRows(listOf(continueWatching, myList, recent), null)
        assertEquals(listOf(HubKeys.CONTINUE_WATCHING, HubKeys.MY_LIST, "recently_added:1"), rows.map { it.key })
        val row = rows[1]
        assertEquals("My List", row.title)
        assertEquals(RowShape.Poster, row.shape)
        assertNull(row.seeAll)
        assertFalse(row.continueWatching)
        assertEquals(listOf(201L), row.items.map { it.id })
    }

    @Test
    fun `an empty My List isn't drawn, and never makes the hero`() {
        val rows = homeRows(listOf(continueWatching, myList.copy(items = emptyList()), recent), null)
        assertFalse(rows.any { it.key == HubKeys.MY_LIST })
        // The hero stays Continue Watching's first.
        assertEquals(204L, heroOf(homeRows(listOf(myList, continueWatching, recent), null))?.id)
    }
}
