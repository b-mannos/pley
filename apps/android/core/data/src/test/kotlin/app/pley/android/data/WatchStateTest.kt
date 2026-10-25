package app.pley.android.data

import app.pley.android.model.Hubs
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemList
import app.pley.android.model.ItemSummary
import app.pley.android.model.RatingRequest
import app.pley.android.model.StreamChoice
import app.pley.android.network.ApiException
import app.pley.android.testing.Fixtures
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class WatchStateTest {
    @get:Rule
    val folder = TemporaryFolder()

    private val refused = ApiException(500, "internal", "the server failed")

    private fun Hubs.item(id: Long): ItemSummary? = hubs.flatMap { it.items }.firstOrNull { it.id == id }

    @Test
    fun `marked watched, every list and page shows it at once`() = runTest {
        val data = signedIn(folder.newFolder())
        val home = collect(data.graph.hubs.home())
        val detail = collect(data.graph.items.detail(101))
        testScheduler.runCurrent()
        assertTrue(home.last().valueOrNull!!.item(101) != null)

        data.api.hold()
        val change = launch { data.graph.watchState.setWatched(101, true) }
        testScheduler.runCurrent()

        // Before the server answers: watched, nothing to resume, out of Continue Watching.
        val shown = detail.last().valueOrNull!!
        assertTrue(shown.userState.watched)
        assertEquals(0, shown.userState.positionMs)
        val continueWatching = home.last().valueOrNull!!.hubs.first { it.key == "continue_watching" }
        assertTrue(continueWatching.items.none { it.id == 101L })

        data.api.release()
        testScheduler.runCurrent()
        change.join()
        assertEquals(listOf<Any?>(101L), data.api.calls("markWatched").single().args)
        // Then asked again: Home and the page.
        assertEquals(2, data.api.calls("homeHubs").size)
        assertEquals(2, data.api.calls("item").size)
    }

    @Test
    fun `a refused mark is put back`() = runTest {
        val data = signedIn(folder.newFolder())
        val home = collect(data.graph.hubs.home())
        val detail = collect(data.graph.items.detail(101))
        testScheduler.runCurrent()
        val homeBefore = home.last().valueOrNull
        val detailBefore = detail.last().valueOrNull
        data.api.fail("markWatched", refused)

        try {
            data.graph.watchState.setWatched(101, true)
            fail("refused")
        } catch (e: ApiException) {
            assertSame(refused, e)
        }
        testScheduler.runCurrent()

        assertEquals(homeBefore, home.last().valueOrNull)
        assertEquals(detailBefore, detail.last().valueOrNull)
        assertFalse(detail.last().valueOrNull!!.userState.watched)
    }

    @Test
    fun `a whole show marked watched marks its seasons and episodes`() = runTest {
        val data = signedIn(folder.newFolder())
        val seasons = collect(data.graph.items.children(201))
        val episodes = collect(data.graph.items.children(202))
        testScheduler.runCurrent()

        data.graph.watchState.setWatched(201, true)

        assertTrue(seasons.last().valueOrNull!!.all { it.userState.watched })
        assertTrue(episodes.last().valueOrNull!!.all { it.userState.watched && it.userState.positionMs == 0L })
    }

    @Test
    fun `an episode marked watched counts in its season and show`() = runTest {
        val episodes: ItemList = Fixtures.decode("items/children_episodes.json")
        val episode = episodes.items.first { !it.userState.watched }
        val season = ItemSummary(id = 202, libraryId = 2, kind = app.pley.android.model.ItemKind.Season, title = "Season 1", leafCount = 3, viewedLeafCount = 1)
        val patch = ItemPatch.watched(episode.id, true, episode)

        val counted = patch.apply(ItemList(listOf(season))) as ItemList
        assertEquals(2, counted.items.single().viewedLeafCount)
        assertFalse(counted.items.single().userState.watched)

        val all = ItemPatch.watched(episode.id, true, episode).apply(ItemList(listOf(season.copy(viewedLeafCount = 2)))) as ItemList
        assertTrue(all.items.single().userState.watched)
    }

    @Test
    fun `values without the item stay the same instance`() {
        val home: Hubs = Fixtures.decode("hubs/home.json")
        assertNull(ItemPatch.watched(999, true, null).apply(home))
        assertNull(ItemPatch.rating(999, 8.0).apply(home))
    }

    @Test
    fun `a rating shows at once, in half stars`() = runTest {
        val data = signedIn(folder.newFolder())
        val detail = collect(data.graph.items.detail(101))
        testScheduler.runCurrent()

        data.graph.watchState.rate(101, 7.3)

        assertEquals(7.5, detail.last().valueOrNull!!.userState.rating)
        assertEquals(listOf<Any?>(101L, RatingRequest(7.5)), data.api.calls("setRating").single().args)
    }

    @Test
    fun `a refused rating is put back`() = runTest {
        val data = signedIn(folder.newFolder())
        val detail = collect(data.graph.items.detail(101))
        testScheduler.runCurrent()
        val before = detail.last().valueOrNull!!.userState.rating
        data.api.fail("setRating", refused)

        runCatching { data.graph.watchState.rate(101, 3.0) }

        assertEquals(before, detail.last().valueOrNull!!.userState.rating)
    }

    @Test
    fun `chosen tracks show on the page and go to the server`() = runTest {
        val data = signedIn(folder.newFolder())
        val detail = collect(data.graph.items.detail(101))
        testScheduler.runCurrent()
        val file = detail.last().valueOrNull!!.files.first()

        data.graph.watchState.chooseStreams(101, file.id, 2, null)

        val chosen: ItemDetail = detail.last().valueOrNull!!
        assertEquals(2, chosen.files.first().selectedAudio)
        assertNull(chosen.files.first().selectedSubtitle)
        assertEquals(listOf<Any?>(101L, StreamChoice(file.id, 2, null)), data.api.calls("setStreams").single().args)
    }
}
