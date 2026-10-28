package app.pley.android.watchnext

import app.pley.android.model.Hub
import app.pley.android.model.HubKeys
import app.pley.android.model.Hubs
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.UserState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** What the Watch Next row shows, and how its programs change. */
class WatchNextPlanTest {
    private val alex = WatchNextOwner("http://nas:7539", 2)
    private val sam = WatchNextOwner("http://nas:7539", 3)

    private fun movie(id: Long, positionMs: Long = 0) = ItemSummary(
        id = id,
        libraryId = 1,
        kind = ItemKind.Movie,
        title = "Arrival",
        durationMs = 6_960_000,
        backdropUrl = "/api/items/$id/backdrop?v=1",
        userState = UserState(positionMs = positionMs, lastPlayedAt = 1_761_000_000),
    )

    private fun episode(id: Long, positionMs: Long = 0) = ItemSummary(
        id = id,
        libraryId = 2,
        kind = ItemKind.Episode,
        title = "Good News About Hell",
        index = 1,
        parentIndex = 1,
        grandparentTitle = "Severance",
        thumbUrl = "/api/items/$id/thumb?v=2",
        backdropUrl = "/api/items/201/backdrop?v=3",
        userState = UserState(positionMs = positionMs),
    )

    private fun home(vararg items: ItemSummary) = Hubs(
        listOf(
            Hub(HubKeys.CONTINUE_WATCHING, "Continue Watching", items = items.toList()),
            Hub("recently_added:1", "Recently Added", libraryId = 1, items = listOf(movie(999))),
        ),
    )

    @Test
    fun `continue watching becomes the row, part watched as Continue and episodes not started as Next`() {
        val row = WatchNextPlan.of(home(movie(101, positionMs = 600_000), episode(203)), alex, now = 1_800_000_000_000)
        assertEquals(listOf(101L, 203L), row.map { it.itemId })
        val (film, next) = row
        assertEquals(WatchNextKind.Continue, film.kind)
        assertEquals(600_000L, film.positionMs)
        assertEquals("Arrival", film.title)
        assertEquals(1_761_000_000_000L, film.engagedAtMs)
        assertEquals("/api/items/101/backdrop?v=1", film.imagePath)
        assertEquals(WatchNextKind.Next, next.kind)
        assertEquals("Severance", next.title)
        assertEquals("Good News About Hell", next.episodeTitle)
        assertEquals(1, next.season)
        assertEquals(1, next.number)
        // An episode's own still, not its show's backdrop.
        assertEquals("/api/items/203/thumb?v=2", next.imagePath)
        assertEquals("http://nas:7539#2|203", next.internalId)
    }

    @Test
    fun `only movies and episodes, ten at most, and nothing without Continue Watching`() {
        val show = movie(201).copy(kind = ItemKind.Show)
        val many = (1..15L).map { movie(it, positionMs = 1_000) }.toTypedArray()
        assertEquals(10, WatchNextPlan.of(home(*many), alex, 0).size)
        assertTrue(WatchNextPlan.of(home(show), alex, 0).isEmpty())
        assertTrue(WatchNextPlan.of(Hubs(), alex, 0).isEmpty())
    }

    @Test
    fun `programs follow the list, another profile's go, and one taken off the row isn't put back`() {
        val wanted = WatchNextPlan.of(home(movie(101, 1_000), episode(203), movie(104, 2_000)), alex, 0)
        val published = listOf(
            PublishedProgram(1, "http://nas:7539#2|101", browsable = true),
            // Taken off the row by the person: left alone.
            PublishedProgram(2, "http://nas:7539#2|203", browsable = false),
            // Finished, or taken off Continue Watching.
            PublishedProgram(3, "http://nas:7539#2|105", browsable = true),
            // Another profile's.
            PublishedProgram(4, "http://nas:7539#3|101", browsable = true),
            // Twice the same: one goes.
            PublishedProgram(5, "http://nas:7539#2|101", browsable = true),
        )
        val changes = WatchNextDiff.of(published, wanted)
        assertEquals(listOf(1L), changes.update.map { it.first })
        assertEquals(listOf(3L, 4L, 5L), changes.delete)
        assertEquals(listOf(104L), changes.insert.map { it.itemId })
    }

    @Test
    fun `signed out or another profile, everything of the one before goes`() {
        val published = listOf(PublishedProgram(1, "http://nas:7539#2|101", true), PublishedProgram(2, null, true))
        assertEquals(listOf(1L, 2L), WatchNextDiff.of(published, emptyList()).delete)
        val samRow = WatchNextPlan.of(home(movie(101, 1_000)), sam, 0)
        val changes = WatchNextDiff.of(published, samRow)
        assertEquals(listOf(1L, 2L), changes.delete)
        assertEquals(listOf("http://nas:7539#3|101"), changes.insert.map { it.internalId })
    }
}
