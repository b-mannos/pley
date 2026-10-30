package app.pley.android.screens.item

import app.pley.android.data.RequestChange
import app.pley.android.model.ItemKind
import app.pley.android.model.RequestAccess
import app.pley.android.model.RequestSummary
import app.pley.android.model.TitleKind
import app.pley.android.model.TitleStatus
import app.pley.android.screens.PleyKey
import app.pley.android.testing.Fixtures
import app.pley.android.testing.data.FakeRequests
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** What requests say of a library's movie or show: asked once the page has drawn, never before. */
@OptIn(ExperimentalCoroutinesApi::class)
class ItemRequestTest {
    private val dispatcher = UnconfinedTestDispatcher()
    private val data = FakeData()
    private val summary: RequestSummary = Fixtures.decode("requests/summary.json")

    @Before
    fun setUp() = Dispatchers.setMain(dispatcher)

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun TestScope.model(requests: FakeRequests): ItemViewModel {
        data.requests = requests
        val model = ItemViewModel(data, RecordingLauncher(), 1, compute = dispatcher)
        backgroundScope.launch(dispatcher) { model.state.collect { } }
        backgroundScope.launch(dispatcher) { model.request.collect { } }
        return model
    }

    private val show = movie().copy(kind = ItemKind.Show, title = "Severance", files = emptyList())

    @Test
    fun `a show's request state is asked for once the page has drawn`() = runTest(dispatcher) {
        val requests = FakeRequests(summary)
        val model = model(requests)
        data.set(show)
        assertNull(model.request.value)
        assertTrue(requests.api.calls("itemRequest").isEmpty())
        model.onShown()
        assertEquals(listOf<Any?>(1L), requests.api.calls("itemRequest").map { it.args.single() })
        val state = model.request.value!!
        assertEquals(TitleStatus.Partial, state.status)
        // Season 2 may still be asked for: Request more seasons opens its picker.
        assertTrue(state.more)
        assertEquals(PleyKey.DiscoverTitle(TitleKind.Show, 95396, pick = true), state.picker)

        // Its requests changed twice in a moment: read again, once.
        requests.change(RequestChange(70, TitleKind.Show, 95396))
        requests.change(RequestChange(70, TitleKind.Show, 95396))
        advanceTimeBy(600)
        assertEquals(2, requests.api.calls("itemRequest").size)
        // Another title's change isn't read for.
        requests.change(RequestChange(71, TitleKind.Movie, 1184918))
        advanceTimeBy(600)
        assertEquals(2, requests.api.calls("itemRequest").size)
    }

    @Test
    fun `nothing is asked for someone who may not ask and has no requests`() = runTest(dispatcher) {
        val requests = FakeRequests(summary.copy(access = RequestAccess.Off, own = 0, unseen = 0))
        val model = model(requests)
        data.set(show)
        model.onShown()
        assertNull(model.request.value)
        assertTrue(requests.api.calls("itemRequest").isEmpty())
    }

    @Test
    fun `someone who may no longer ask still sees their request, without Request more seasons`() = runTest(dispatcher) {
        val requests = FakeRequests(summary.copy(access = RequestAccess.Off))
        val model = model(requests)
        data.set(show)
        model.onShown()
        val state = model.request.value!!
        assertEquals(TitleStatus.Partial, state.status)
        assertTrue(!state.more)
    }

    @Test
    fun `music isn't asked about`() = runTest(dispatcher) {
        val requests = FakeRequests(summary)
        val model = model(requests)
        data.set(movie().copy(kind = ItemKind.Album, title = "Blue"))
        model.onShown()
        assertNull(model.request.value)
        assertTrue(requests.api.calls("itemRequest").isEmpty())
    }
}
