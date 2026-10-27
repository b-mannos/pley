package app.pley.android.player.session

import app.pley.android.data.ItemRepository
import app.pley.android.data.Loadable
import app.pley.android.model.CollectionSummary
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.network.OfflineException
import app.pley.android.testing.network.FakePleyApi
import java.io.IOException
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Where playing gets an item's details: what was saved first, else the server. */
class ItemDetailsTest {
    private val api = FakePleyApi()

    @Test
    fun aServerOutOfReachIsntAskedAgain() = runBlocking {
        // What the cache held from before, then its new try, out of reach as well.
        val details = ItemDetails.snapshotFirst(items(Loadable.Failed(away()), Loadable.Failed(away())), api)

        val error = runCatching { details.detail(7) }.exceptionOrNull()

        assertTrue(error is OfflineException)
        assertEquals("the repository's try was the one wait", 0, api.calls("item").size)
    }

    @Test
    fun aFailureTheCacheHeldFromBeforeDoesntDecide() = runBlocking {
        val alien = ItemDetail(id = 7, libraryId = 1, kind = ItemKind.Movie, title = "Alien")
        // The server is back: the cache's new answer comes after what it held.
        val details = ItemDetails.snapshotFirst(items(Loadable.Failed(away()), Loadable.Ready(alien, fresh = true)), api)

        assertEquals(alien, details.detail(7))
        assertEquals(0, api.calls("item").size)
    }

    @Test
    fun anotherFailureAsksTheServer() = runBlocking {
        val details = ItemDetails.snapshotFirst(items(Loadable.Failed(IllegalStateException("not signed in yet"))), api)

        runCatching { details.detail(7) }

        assertEquals(1, api.calls("item").size)
    }

    @Test
    fun aCopyBeingAskedAgainWaitsForTheServersTracks() = runBlocking {
        // Subtitles were turned on for the show since this episode was held.
        val held = ItemDetail(id = 7, libraryId = 1, kind = ItemKind.Episode, title = "Pilot")
        val now = held.copy(title = "Pilot, with the tracks picked since")
        val details = ItemDetails.snapshotFirst(items(Loadable.Ready(held, fresh = false), Loadable.Ready(now, fresh = true)), api)

        assertEquals(now, details.detail(7))
        assertEquals(0, api.calls("item").size)
    }

    @Test
    fun aCopyBeingAskedAgainPlaysWhenTheServerCantAnswer() = runBlocking {
        val held = ItemDetail(id = 7, libraryId = 1, kind = ItemKind.Episode, title = "Pilot")
        val failed = ItemDetails.snapshotFirst(items(Loadable.Ready(held, fresh = false), Loadable.Failed(away(), held)), api)
        assertEquals(held, failed.detail(7))
        // Nothing within the wait: what is held plays.
        val silent = object : ItemRepository by items() {
            override fun detail(itemId: Long): Flow<Loadable<ItemDetail>> = flow {
                emit(Loadable.Ready(held, fresh = false))
                awaitCancellation()
            }
        }
        val started = System.nanoTime()
        assertEquals(held, ItemDetails.snapshotFirst(silent, api).detail(7))
        assertTrue((System.nanoTime() - started) / 1_000_000 < ItemDetails.FRESH_WAIT_MS + 1_000)
    }

    private fun away() = OfflineException(IOException("no answer in time"))

    private fun items(vararg answers: Loadable<ItemDetail>) = object : ItemRepository {
        override fun detail(itemId: Long): Flow<Loadable<ItemDetail>> = flowOf(*answers)

        override fun children(itemId: Long): Flow<Loadable<List<ItemSummary>>> = flowOf(Loadable.Loading)

        override fun onDeck(itemId: Long): Flow<Loadable<ItemSummary?>> = flowOf(Loadable.Loading)

        override fun related(itemId: Long): Flow<Loadable<List<ItemSummary>>> = flowOf(Loadable.Loading)

        override fun collectionsOf(itemId: Long): Flow<Loadable<List<CollectionSummary>>> = flowOf(Loadable.Loading)

        override suspend fun next(itemId: Long): ItemSummary? = null

        override fun prefetch(itemId: Long) = Unit
    }
}
