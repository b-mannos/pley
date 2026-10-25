package app.pley.android.screens.search

import app.pley.android.data.Loadable
import app.pley.android.data.SearchRepository
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.PersonSummary
import app.pley.android.model.SearchResults
import app.pley.android.screens.PleyKey
import app.pley.android.screens.library.FakeTile
import app.pley.android.screens.library.FakeTiles
import app.pley.android.screens.library.TileLook
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Search's 250 ms pause, the latest query only, the groups and the recent searches, on the test's clock. */
@OptIn(ExperimentalCoroutinesApi::class)
class SearchViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val search = FakeSearch()

    @Before
    fun main() = Dispatchers.setMain(dispatcher)

    @After
    fun reset() = Dispatchers.resetMain()

    /** Answers the test gives per query; a query without one waits. */
    private class FakeSearch : SearchRepository {
        val asked = mutableListOf<String>()
        val cancelled = mutableListOf<String>()
        val answers = HashMap<String, CompletableDeferred<SearchResults>>()
        override val recent = MutableStateFlow(listOf("halcyon"))

        fun answer(q: String): CompletableDeferred<SearchResults> = answers.getOrPut(q) { CompletableDeferred() }

        override fun search(query: String): Flow<Loadable<SearchResults>> = flow {
            asked += query
            emit(Loadable.Loading)
            try {
                emit(Loadable.Ready(answer(query).await(), fresh = true))
            } catch (e: kotlinx.coroutines.CancellationException) {
                cancelled += query
                throw e
            } catch (e: IOException) {
                emit(Loadable.Failed(e))
            }
        }

        override suspend fun remember(query: String) {
            recent.value = (listOf(query) + recent.value.filterNot { it == query }).take(10)
        }

        override suspend fun clearRecent() {
            recent.value = emptyList()
        }
    }

    private fun TestScope.viewModel(initial: String = ""): SearchViewModel<FakeTile> {
        val tiles = SearchTiles.of(FakeTiles) { person, origin -> FakeTile(person.id, person.name, TileLook.Poster, "person $origin") }
        val vm = SearchViewModel(initial, SearchDeps(search, MutableStateFlow("http://pley"), dispatcher), tiles)
        backgroundScope.launch { vm.state.collect {} }
        return vm
    }

    private fun SearchViewModel<FakeTile>.results() = state.value.results

    private fun SearchViewModel<FakeTile>.type(text: String) = onQueryChange(text)

    private fun item(id: Long, kind: ItemKind, title: String = "Item $id", parentId: Long? = null) =
        ItemSummary(id = id, libraryId = 1, kind = kind, title = title, parentId = parentId)

    @Test
    fun typingWaitsFor250msOfQuietBeforeAsking() = runTest(dispatcher) {
        val vm = viewModel()
        runCurrent()
        vm.type("n")
        advanceTimeBy(100)
        vm.type("no")
        advanceTimeBy(100)
        vm.type("nor")
        advanceTimeBy(249)
        assertEquals(emptyList<String>(), search.asked)
        advanceTimeBy(2)
        assertEquals(listOf("nor"), search.asked)
        assertEquals(SearchResultsState.Loading("nor"), vm.results())
    }

    @Test
    fun onlyTheLatestQuerysAnswerCounts() = runTest(dispatcher) {
        val vm = viewModel()
        vm.type("north")
        advanceTimeBy(300)
        vm.type("south")
        advanceTimeBy(300)
        assertEquals(listOf("north", "south"), search.asked)
        // The older request was given up.
        assertEquals(listOf("north"), search.cancelled)
        search.answer("north").complete(SearchResults(items = listOf(item(1, ItemKind.Movie, "Northern Lights"))))
        runCurrent()
        assertEquals(SearchResultsState.Loading("south"), vm.results())
        search.answer("south").complete(SearchResults(items = listOf(item(2, ItemKind.Movie, "South Wind"))))
        advanceUntilIdle()
        val ready = vm.results() as SearchResultsState.Ready
        assertEquals("south", ready.query)
        assertEquals(listOf("South Wind"), ready.groups.single().tiles.map { it.title })
    }

    @Test
    fun resultsStayWhileTheNextOnesLoad() = runTest(dispatcher) {
        val vm = viewModel()
        vm.type("north")
        search.answer("north").complete(SearchResults(items = listOf(item(1, ItemKind.Movie))))
        advanceUntilIdle()
        assertEquals(false, (vm.results() as SearchResultsState.Ready).loading)
        vm.type("northe")
        advanceTimeBy(300)
        val kept = vm.results() as SearchResultsState.Ready
        assertEquals("north", kept.query)
        assertTrue(kept.loading)
        search.answer("northe").complete(SearchResults())
        advanceUntilIdle()
        assertEquals(SearchResultsState.Empty("northe"), vm.results())
    }

    @Test
    fun resultsComeInTheWebsGroupsAndOrderWithCounts() = runTest(dispatcher) {
        val vm = viewModel("lights")
        search.answer("lights").complete(
            SearchResults(
                items = listOf(
                    item(1, ItemKind.Track, "Lights", parentId = 40),
                    item(2, ItemKind.Episode),
                    item(3, ItemKind.Movie),
                    item(4, ItemKind.Album),
                    item(5, ItemKind.Show),
                    item(6, ItemKind.Movie),
                    item(7, ItemKind.Extra),
                    item(8, ItemKind.Season),
                    item(9, ItemKind.Artist),
                ),
                people = listOf(PersonSummary(70, "Lena Okafor")),
            ),
        )
        advanceUntilIdle()
        val groups = (vm.results() as SearchResultsState.Ready).groups
        assertEquals(
            listOf(
                SearchGroupKind.People, SearchGroupKind.Movies, SearchGroupKind.Shows, SearchGroupKind.Seasons,
                SearchGroupKind.Episodes, SearchGroupKind.Artists, SearchGroupKind.Albums, SearchGroupKind.Tracks,
            ),
            groups.map { it.kind },
        )
        val movies = groups.first { it.kind == SearchGroupKind.Movies }
        assertEquals(2, movies.count)
        // The server's order of relevance holds inside a group.
        assertEquals(listOf(3L, 6L), movies.ids)
        assertEquals(TileLook.Landscape, groups.first { it.kind == SearchGroupKind.Episodes }.tiles.single().look)
        assertEquals(TileLook.Square, groups.first { it.kind == SearchGroupKind.Albums }.tiles.single().look)
        assertEquals("person http://pley", groups.first().tiles.single().subtitle)
        // Opening: a person's page, a track's album, an album's page, the rest their item page.
        assertEquals(PleyKey.Person(70), vm.open(SearchGroupKind.People, 70))
        assertEquals(PleyKey.Album(40), vm.open(SearchGroupKind.Tracks, 1))
        assertEquals(PleyKey.Album(4), vm.open(SearchGroupKind.Albums, 4))
        assertEquals(PleyKey.Artist(9), vm.open(SearchGroupKind.Artists, 9))
        assertEquals(PleyKey.Item(3), vm.open(SearchGroupKind.Movies, 3))
    }

    @Test
    fun aQueryIsRememberedWhenSubmittedOrOneOfItsResultsIsOpened() = runTest(dispatcher) {
        val vm = viewModel()
        advanceUntilIdle()
        assertEquals(listOf("halcyon"), vm.state.value.recent)
        assertEquals(SearchResultsState.Idle, vm.results())
        vm.type(" north ")
        vm.submit()
        advanceUntilIdle()
        assertEquals(listOf("north", "halcyon"), vm.state.value.recent)
        vm.type("orbit")
        search.answer("orbit").complete(SearchResults(items = listOf(item(1, ItemKind.Movie))))
        advanceUntilIdle()
        vm.open(SearchGroupKind.Movies, 1)
        advanceUntilIdle()
        assertEquals(listOf("orbit", "north", "halcyon"), vm.state.value.recent)
        // A recent search searches again.
        vm.searchAgain("halcyon")
        advanceUntilIdle()
        assertEquals("halcyon", vm.state.value.query)
        assertEquals("halcyon", search.asked.last())
        vm.clearRecent()
        advanceUntilIdle()
        assertEquals(emptyList<String>(), vm.state.value.recent)
    }

    @Test
    fun clearingTheFieldGoesBackToRecentSearchesAtOnce() = runTest(dispatcher) {
        val vm = viewModel("north")
        search.answer("north").complete(SearchResults(items = listOf(item(1, ItemKind.Movie))))
        advanceUntilIdle()
        vm.type("  ")
        runCurrent()
        assertEquals(SearchResultsState.Idle, vm.results())
    }

    @Test
    fun aFailureSaysWhy() = runTest(dispatcher) {
        val vm = viewModel("north")
        search.answer("north").completeExceptionally(IOException("Can't reach the server"))
        advanceUntilIdle()
        val failed = vm.results() as SearchResultsState.Failed
        assertEquals("north", failed.query)
        assertEquals("Can't reach the server", failed.error.message)
    }
}
