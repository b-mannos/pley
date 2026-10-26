package app.pley.android.player.session

import app.pley.android.data.ItemRepository
import app.pley.android.data.Loadable
import app.pley.android.model.CollectionSummary
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemSummary
import app.pley.android.network.OfflineException
import app.pley.android.testing.network.FakePleyApi
import java.io.IOException
import kotlinx.coroutines.flow.Flow
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
        val away = OfflineException(IOException("no answer in time"))
        val details = ItemDetails.snapshotFirst(items(Loadable.Failed(away)), api)

        val error = runCatching { details.detail(7) }.exceptionOrNull()

        assertTrue(error is OfflineException)
        assertEquals("the repository's try was the one wait", 0, api.calls("item").size)
    }

    @Test
    fun anotherFailureAsksTheServer() = runBlocking {
        val details = ItemDetails.snapshotFirst(items(Loadable.Failed(IllegalStateException("not signed in yet"))), api)

        runCatching { details.detail(7) }

        assertEquals(1, api.calls("item").size)
    }

    private fun items(first: Loadable<ItemDetail>) = object : ItemRepository {
        override fun detail(itemId: Long): Flow<Loadable<ItemDetail>> = flowOf(first)

        override fun children(itemId: Long): Flow<Loadable<List<ItemSummary>>> = flowOf(Loadable.Loading)

        override fun onDeck(itemId: Long): Flow<Loadable<ItemSummary?>> = flowOf(Loadable.Loading)

        override fun related(itemId: Long): Flow<Loadable<List<ItemSummary>>> = flowOf(Loadable.Loading)

        override fun collectionsOf(itemId: Long): Flow<Loadable<List<CollectionSummary>>> = flowOf(Loadable.Loading)

        override suspend fun next(itemId: Long): ItemSummary? = null

        override fun prefetch(itemId: Long) = Unit
    }
}
