package app.pley.android.screens.requests

import app.pley.android.data.RequestChange
import app.pley.android.model.DiscoverList
import app.pley.android.model.DiscoverPage
import app.pley.android.model.MediaRequest
import app.pley.android.model.NewRequest
import app.pley.android.model.RequestList
import app.pley.android.model.RequestScope
import app.pley.android.model.RequestSummary
import app.pley.android.model.TitleKind
import app.pley.android.model.TitleStatus
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
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Discover: its hero, its rows read page by page, badges following events, its search. */
@OptIn(ExperimentalCoroutinesApi::class)
class DiscoverViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val trending: DiscoverPage = Fixtures.decode("discover/trending.json")
    private val summary: RequestSummary = Fixtures.decode("requests/summary.json")

    @Before
    fun main() = Dispatchers.setMain(dispatcher)

    @After
    fun reset() = Dispatchers.resetMain()

    private fun TestScope.viewModel(requests: FakeRequests, query: String = ""): DiscoverViewModel {
        val vm = DiscoverViewModel(query, RequestsDeps(requests, MutableStateFlow("http://pley"), MutableStateFlow(true), dispatcher))
        backgroundScope.launch { vm.state.collect {} }
        return vm
    }

    private fun DiscoverViewModel.row(list: DiscoverList) = state.value.rows.first { it.list == list }.pages

    @Test
    fun theHeroIsTheFirstTrendingTitleWithItsMainAction() = runTest(dispatcher) {
        val vm = viewModel(FakeRequests())
        advanceUntilIdle()
        val hero = vm.state.value.hero!!
        assertEquals("The Wild Robot", hero.title.title)
        // Its page was read for the exact action.
        assertEquals(MainAction.Request(Ask.Yes), hero.action)
        assertEquals(null, vm.state.value.problem)
    }

    @Test
    fun aShortPageIsNotTheEndAndEachTitleShowsOnce() = runTest(dispatcher) {
        val requests = FakeRequests()
        val asked = mutableListOf<Int>()
        requests.pages = { _, page ->
            asked += page
            when (page) {
                // Titles above an age limit were left out: none on page 1, yet more follow.
                1 -> DiscoverPage(1, 3, emptyList())
                2 -> DiscoverPage(2, 3, trending.results.take(2))
                else -> DiscoverPage(page, 3, trending.results.drop(1))
            }
        }
        val vm = viewModel(requests)
        advanceUntilIdle()
        val row = vm.row(DiscoverList.Trending)
        assertEquals(listOf(1, 2, 3), asked)
        assertEquals(trending.results.map { it.tmdbId }, row.titles.map { it.tmdbId })
        assertFalse(row.more)
    }

    @Test
    fun rowsAreReadAsTheyComeIntoView() = runTest(dispatcher) {
        val requests = FakeRequests()
        val lists = mutableListOf<DiscoverList>()
        requests.pages = { list, page ->
            lists += list
            DiscoverPage(page, 1, trending.results)
        }
        val vm = viewModel(requests)
        advanceUntilIdle()
        assertEquals(listOf(DiscoverList.Trending), lists.distinct())
        vm.showRow(DiscoverList.Anime)
        advanceUntilIdle()
        assertEquals(listOf(DiscoverList.Trending, DiscoverList.Anime), lists.distinct())
        assertEquals(6, vm.row(DiscoverList.Anime).titles.size)
    }

    @Test
    fun badgesFollowARequestInRowsAlreadyShown() = runTest(dispatcher) {
        val requests = FakeRequests()
        val vm = viewModel(requests)
        advanceUntilIdle()
        val pending = Fixtures.decode<RequestList>("requests/list.json").requests.first { it.id == 44L }
        requests.api.answer("request", pending.copy(id = 61, kind = TitleKind.Movie, tmdbId = 1184918))
        val pagesBefore = requests.api.calls.size
        requests.change(RequestChange(61, TitleKind.Movie, 1184918))
        advanceUntilIdle()
        val robot = vm.row(DiscoverList.Trending).titles.first { it.tmdbId == 1184918L }
        assertEquals(TitleStatus.Requested, robot.status)
        assertEquals(TitleBadge.Requested, badgeOf(robot))
        // One request read, the rows not read again.
        assertEquals(listOf("request"), requests.api.calls.drop(pagesBefore).map { it.method }.filter { it != "requests" })
    }

    @Test
    fun signedInBeforeRequestsShowsOnlySigningInAgain() = runTest(dispatcher) {
        val requests = FakeRequests(summary.copy(signInAgain = RequestScope.Requests, own = 0, unseen = 0))
        val read = mutableListOf<Int>()
        requests.pages = { _, page -> read += page; trending }
        val vm = viewModel(requests)
        advanceUntilIdle()
        assertEquals(RequestProblem.SignInAgain(RequestScope.Requests), vm.state.value.problem)
        assertEquals(emptyList<Int>(), read)
    }

    @Test
    fun tmdbAwayShowsItsStateAndRetryReadsAgain() = runTest(dispatcher) {
        val requests = FakeRequests()
        var down = true
        requests.pages = { _, page -> if (down) throw ApiException(503, "tmdb_down", "TMDB isn't answering") else trending.copy(page = page) }
        val vm = viewModel(requests)
        advanceUntilIdle()
        assertEquals(RequestProblem.TmdbDown, vm.state.value.problem)
        down = false
        vm.retry()
        advanceUntilIdle()
        assertEquals(null, vm.state.value.problem)
        assertEquals("The Wild Robot", vm.state.value.hero?.title?.title)
    }

    @Test
    fun theHeroAsksForAMovieAndOpensAShowsPicker() = runTest(dispatcher) {
        val requests = FakeRequests()
        val vm = viewModel(requests)
        val events = mutableListOf<RequestEvent>()
        // Unconfined: an event is taken as it's sent, before advanceUntilIdle returns.
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { vm.events.collect { events += it } }
        advanceUntilIdle()
        vm.heroAction()
        advanceUntilIdle()
        assertEquals(NewRequest(TitleKind.Movie, 1184918), requests.api.calls("ask").single().args.single())
        assertTrue(events.single() is RequestEvent.Asked)

        // A show first: its picker, on its page.
        val severance = trending.results.first { it.tmdbId == 95396L }.copy(status = TitleStatus.None, itemId = null)
        requests.pages = { _, page -> DiscoverPage(page, 1, listOf(severance)) }
        val other = viewModel(requests)
        val opened = mutableListOf<RequestEvent>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { other.events.collect { opened += it } }
        advanceUntilIdle()
        other.heroAction()
        advanceUntilIdle()
        assertEquals(RequestEvent.Open(PleyKey.DiscoverTitle(TitleKind.Show, 95396, pick = true)), opened.single())

        // Partly in a library: Open goes to what the library has.
        val partly = trending.results.first { it.tmdbId == 95396L }
        requests.pages = { _, page -> DiscoverPage(page, 1, listOf(partly)) }
        val third = viewModel(requests)
        val went = mutableListOf<RequestEvent>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { third.events.collect { went += it } }
        advanceUntilIdle()
        third.heroAction()
        advanceUntilIdle()
        assertEquals(RequestEvent.Open(PleyKey.Item(201)), went.single())
    }

    @Test
    fun searchWaitsForTypingToPauseAndReadsOnUntilAScreenful() = runTest(dispatcher) {
        val requests = FakeRequests()
        val asked = mutableListOf<Pair<String, Int>>()
        requests.searches = { q, page, _ ->
            asked += q to page
            DiscoverPage(page, 2, if (page == 1) trending.results.take(3) else trending.results.drop(3))
        }
        val vm = viewModel(requests)
        backgroundScope.launch { vm.search.collect {} }
        advanceUntilIdle()
        vm.onQueryChange("ro")
        advanceTimeBy(100)
        vm.onQueryChange("robot")
        advanceTimeBy(249)
        assertEquals(emptyList<Pair<String, Int>>(), asked)
        advanceUntilIdle()
        // Three titles aren't a screenful: page 2 too.
        assertEquals(listOf("robot" to 1, "robot" to 2), asked)
        val ready = vm.search.value as DiscoverSearch.Ready
        assertEquals(6, ready.pages.titles.size)
        assertFalse(ready.pages.more)
    }

    @Test
    fun yourRequestsShowForThoseWhoHaveAny() = runTest(dispatcher) {
        val requests = FakeRequests()
        val vm = viewModel(requests)
        advanceUntilIdle()
        assertTrue(vm.state.value.yours.isNotEmpty())
        val none = FakeRequests(summary.copy(own = 0, unseen = 0))
        val other = viewModel(none)
        advanceUntilIdle()
        assertEquals(emptyList<MediaRequest>(), other.state.value.yours)
        assertTrue(none.api.calls("requests").isEmpty())
    }
}
