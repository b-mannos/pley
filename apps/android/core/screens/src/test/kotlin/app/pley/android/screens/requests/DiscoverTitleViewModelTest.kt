package app.pley.android.screens.requests

import app.pley.android.model.ApiErrorBody
import app.pley.android.model.NewRequest
import app.pley.android.model.RequestScope
import app.pley.android.model.RequestSummary
import app.pley.android.model.TitleKind
import app.pley.android.network.ApiException
import app.pley.android.screens.PleyKey
import app.pley.android.testing.Fixtures
import app.pley.android.testing.data.FakeRequests
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** A title page: its main action, the picker, and what asking tells. */
@OptIn(ExperimentalCoroutinesApi::class)
class DiscoverTitleViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val summary: RequestSummary = Fixtures.decode("requests/summary.json")

    @Before
    fun main() = Dispatchers.setMain(dispatcher)

    @After
    fun reset() = Dispatchers.resetMain()

    private class Page(val vm: DiscoverTitleViewModel, val events: MutableList<RequestEvent>)

    private fun TestScope.page(requests: FakeRequests, key: PleyKey.DiscoverTitle): Page {
        val vm = DiscoverTitleViewModel(key, RequestsDeps(requests, MutableStateFlow("http://pley"), MutableStateFlow(true), dispatcher))
        val events = mutableListOf<RequestEvent>()
        backgroundScope.launch { vm.state.collect {} }
        backgroundScope.launch { vm.picker.collect {} }
        // Unconfined: an event is taken as it's sent, before advanceUntilIdle returns.
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { vm.events.collect { events += it } }
        return Page(vm, events)
    }

    private fun Page.ready() = vm.state.value as TitleUiState.Ready

    @Test
    fun aMovieIsAskedForFromItsPage() = runTest(dispatcher) {
        val requests = FakeRequests()
        val page = page(requests, PleyKey.DiscoverTitle(TitleKind.Movie, 1184918))
        advanceUntilIdle()
        assertEquals(MainAction.Request(Ask.Yes), page.ready().action)
        page.vm.request()
        advanceUntilIdle()
        assertEquals(NewRequest(TitleKind.Movie, 1184918), requests.api.calls("ask").single().args.single())
        val asked = page.events.single() as RequestEvent.Asked
        assertEquals("The Wild Robot", asked.title)
        assertTrue(asked.asked.created)
    }

    @Test
    fun requestMoreSeasonsOpensThePickerAndAsksForWhatIsTicked() = runTest(dispatcher) {
        val requests = FakeRequests()
        val page = page(requests, PleyKey.DiscoverTitle(TitleKind.Show, 1396, pick = true))
        advanceUntilIdle()
        assertEquals(MainAction.Partial(null, Ask.Yes), page.ready().action)
        // Opened with the page, as asked: 3 and 5 ticked.
        val picker = page.vm.picker.value!!
        assertEquals(setOf(3, 5), picker.picked)
        page.vm.toggle(5)
        page.vm.askPicked()
        advanceUntilIdle()
        assertEquals(NewRequest(TitleKind.Show, 1396, listOf(3)), requests.api.calls("ask").single().args.single())
        assertNull(page.vm.picker.value)
    }

    @Test
    fun aLimitSaysWhenMoreCanBeAskedFor() = runTest(dispatcher) {
        val requests = FakeRequests()
        requests.api.fail("ask", ApiException(429, "too_many_requests", "limit", details = ApiErrorBody("too_many_requests", "limit", retryAt = 1762000000)))
        val page = page(requests, PleyKey.DiscoverTitle(TitleKind.Movie, 1184918))
        advanceUntilIdle()
        page.vm.request()
        advanceUntilIdle()
        assertEquals(RequestEvent.Failed(RequestProblem.Limit(1762000000)), page.events.single())
        assertTrue(!page.ready().busy)
    }

    @Test
    fun signedInBeforeRequestsHasNoTitlePages() = runTest(dispatcher) {
        val requests = FakeRequests(summary.copy(signInAgain = RequestScope.Requests))
        val page = page(requests, PleyKey.DiscoverTitle(TitleKind.Movie, 1184918))
        advanceUntilIdle()
        assertEquals(RequestProblem.SignInAgain(RequestScope.Requests), (page.vm.state.value as TitleUiState.Problem).problem)
        assertTrue(requests.api.calls("discoverTitle").isEmpty())
    }

    @Test
    fun aTitleTmdbDoesNotHaveIsNotFound() = runTest(dispatcher) {
        val requests = FakeRequests()
        requests.api.fail("discoverTitle", ApiException(404, "not_found", "no such title"))
        val page = page(requests, PleyKey.DiscoverTitle(TitleKind.Movie, 1))
        advanceUntilIdle()
        assertEquals(RequestProblem.NotFound, (page.vm.state.value as TitleUiState.Problem).problem)
    }
}
