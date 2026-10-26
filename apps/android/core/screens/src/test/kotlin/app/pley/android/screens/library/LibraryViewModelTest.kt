package app.pley.android.screens.library

import app.pley.android.data.GridDensity
import app.pley.android.data.LibraryItems
import app.pley.android.data.LibraryQuery
import app.pley.android.data.Loadable
import app.pley.android.data.valueOrNull
import app.pley.android.model.CollectionSummary
import app.pley.android.model.Hub
import app.pley.android.model.Hubs
import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.model.LibrarySort
import app.pley.android.model.SortOrder
import app.pley.android.screens.LibraryTab
import app.pley.android.screens.PleyKey
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class LibraryViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val libraries = FakeLibraries(
        Library(1, "Movies", LibraryKind.Movies),
        Library(2, "Music", LibraryKind.Music),
    )
    private val items = FakeLibraryItems()
    private val hubs = FakeHubs()
    private val collections = FakeCollections()
    private val settings = FakeSettings()
    private val views = FakeLibraryViews()

    @Before
    fun main() = Dispatchers.setMain(dispatcher)

    @After
    fun reset() = Dispatchers.resetMain()

    private fun viewModel(libraryId: Long = 1, tab: LibraryTab = LibraryTab.Library, saved: String? = null) = LibraryViewModel(
        libraryId = libraryId,
        tab = tab,
        saved = saved,
        deps = LibraryDeps(libraries, items, hubs, collections, settings, MutableStateFlow("http://pley"), dispatcher, views),
        tiles = FakeTiles,
    )

    private val byTitle = LibraryQuery(LibrarySort.Title, SortOrder.Ascending)

    @Test
    fun theGridIsSizedFromTheTotalWithAWindowInIt() = runTest(dispatcher) {
        items.set(byTitle, LibraryItems(total = 9_000, items = (3_001L..5_000L).map { movie(it) }, offset = 3_000, windowed = true))
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        val cells = vm.state.value.grid.cells.valueOrNull!!
        assertEquals(9_000, cells.total)
        assertNull(cells[0])
        assertEquals(3_001L, cells[3_000]!!.id)
        assertEquals(-1L, cells.key(0))
        assertEquals(3_001L, cells.key(3_000))
        // A windowed listing has no A to Z: not every title is known.
        assertNull(vm.state.value.grid.index)
        vm.showing(6_200)
        assertEquals(byTitle to 6_200, items.showings.last())
        job.cancel()
    }

    @Test
    fun aCompleteListingByTitleHasItsAToZ() = runTest(dispatcher) {
        val titles = (1..40).map { i -> movie(i.toLong(), if (i <= 20) "Alien $i" else "Zodiac $i") }
        items.set(byTitle, LibraryItems(40, titles))
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        val index = vm.state.value.grid.index
        assertNotNull(index)
        assertEquals(20, index!!.positionOf(AlphaIndex.LETTERS.indexOf("Z")))
        // By year, none.
        val byYear = LibraryQuery(LibrarySort.Year, SortOrder.Descending)
        items.set(byYear, LibraryItems(40, titles))
        vm.sortBy(LibrarySort.Year)
        advanceUntilIdle()
        assertEquals(byYear, items.asked.last())
        assertNull(vm.state.value.grid.index)
        job.cancel()
    }

    @Test
    fun shortListingsHaveNoAToZ() = runTest(dispatcher) {
        items.set(byTitle, LibraryItems(30, (1L..30L).map { movie(it) }))
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(30, vm.state.value.grid.cells.valueOrNull!!.total)
        assertNull(vm.state.value.grid.index)
        job.cancel()
    }

    @Test
    fun sortedByBitrateTilesSayWhatTheFilesAreLike() = runTest(dispatcher) {
        val byBitrate = LibraryQuery(LibrarySort.Bitrate, SortOrder.Descending)
        items.set(byBitrate, LibraryItems(1, listOf(movie(7))))
        val vm = viewModel(saved = "sort=bitrate")
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(TileLook.Bitrate, vm.state.value.grid.look)
        assertEquals(TileLook.Bitrate, vm.state.value.grid.cells.valueOrNull!![0]!!.look)
        job.cancel()
    }

    @Test
    fun filtersChangeTheListingAndSaveAsCarriesThem() = runTest(dispatcher) {
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(listOf("Comedy", "Drama"), vm.state.value.filters.genres)
        vm.watched(false)
        vm.genre("Comedy")
        vm.year(1999)
        advanceUntilIdle()
        assertEquals(LibraryQuery(LibrarySort.Title, SortOrder.Ascending, unwatched = true, genre = "Comedy", year = 1999), items.asked.last())
        assertTrue(vm.state.value.filtered)
        assertEquals("watched=false&genre=Comedy&year=1999", vm.state.value.params.toQueryString())
        assertEquals(PleyKey.SaveSmartPlaylist(1, "genre=Comedy&year=1999&unwatched=1&sort=titleSort"), vm.saveAs())
        vm.clearFilters()
        advanceUntilIdle()
        assertEquals(byTitle, items.asked.last())
        job.cancel()
    }

    @Test
    fun aSavedPageComesBackAsItWas() = runTest(dispatcher) {
        val vm = viewModel(tab = LibraryTab.Library, saved = "tab=collections&sort=year&order=asc")
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(LibraryTab.Collections, vm.state.value.params.tab)
        assertEquals(LibraryParams(LibraryTab.Collections, LibrarySort.Year, SortOrder.Ascending), vm.state.value.params)
        job.cancel()
    }

    @Test
    fun aLibraryOpensAsItWasLeft() = runTest(dispatcher) {
        views.kept[1] = "sort=added&watched=false"
        val vm = viewModel(tab = LibraryTab.Library)
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(LibraryParams(sort = LibrarySort.Added, watched = false), vm.state.value.params)
        // Read before the listing: it isn't asked for twice.
        assertEquals(listOf(LibraryQuery(LibrarySort.Added, SortOrder.Descending, unwatched = true)), items.asked.distinct())
        job.cancel()
    }

    @Test
    fun whatIsChosenIsHowTheLibraryOpensNextTime() = runTest(dispatcher) {
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertTrue("opening keeps nothing", views.kept.isEmpty())
        vm.sortBy(LibrarySort.Year)
        vm.genre("Comedy")
        vm.select(LibraryTab.Recommended)
        advanceUntilIdle()
        assertEquals("sort=year&genre=Comedy", views.kept[1])
        vm.clearFilters()
        vm.sortBy(LibrarySort.Title)
        advanceUntilIdle()
        assertNull("the default view is forgotten", views.kept[1])
        job.cancel()
    }

    @Test
    fun aPageThatComesBackKeepsItsOwnView() = runTest(dispatcher) {
        views.kept[1] = "sort=added"
        val vm = viewModel(saved = "sort=year")
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(LibrarySort.Year, vm.state.value.params.sort)
        assertEquals("sort=added", views.kept[1])
        job.cancel()
    }

    @Test
    fun tabsReadOnlyOnceShown() = runTest(dispatcher) {
        hubs.library.value = Loadable.Ready(Hubs(listOf(Hub("continue_watching:1", "Continue Watching", 1, listOf(movie(3))), Hub("top_rated:1", "Top Rated", 1, listOf(movie(4))))), true)
        collections.lists.value = mapOf(1L to listOf(CollectionSummary(9, 1, "Alien Collection", itemCount = 4)))
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(0, hubs.opened)
        assertEquals(Loadable.Loading, vm.state.value.recommended)
        vm.select(LibraryTab.Recommended)
        advanceUntilIdle()
        assertEquals(1, hubs.opened)
        val rows = vm.state.value.recommended.valueOrNull!!
        assertEquals(listOf("continue_watching:1", "top_rated:1", TileRow.COLLECTIONS), rows.map { it.key })
        assertEquals(TileLook.Landscape, rows[0].look)
        assertEquals(listOf(PleyKey.Collection(9)), rows[2].opens)
        val shelf = vm.state.value.collections.valueOrNull!!
        assertEquals(CollectionTile(9, 4, FakeTile(9, "Alien Collection", TileLook.Poster, "4 movies")), shelf.single())
        job.cancel()
    }

    @Test
    fun musicHasNoCollectionsOrFiltersAndOpensArtists() = runTest(dispatcher) {
        val vm = viewModel(libraryId = 2, saved = "tab=collections&watched=false&sort=bitrate")
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        val state = vm.state.value
        assertEquals(listOf(LibraryTab.Recommended, LibraryTab.Library), state.tabs)
        assertEquals(LibraryParams(), state.params)
        assertEquals(byTitle, items.asked.last())
        assertEquals(PleyKey.Artist(5), state.opens(5))
        job.cancel()
    }

    @Test
    fun anUnknownLibraryIsMissing() = runTest(dispatcher) {
        val vm = viewModel(libraryId = 42)
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertTrue(vm.state.value.missing)
        job.cancel()
    }

    @Test
    fun densityIsTheDevicesSetting() = runTest(dispatcher) {
        val vm = viewModel()
        val job = launch { vm.state.collect {} }
        advanceUntilIdle()
        assertEquals(GridDensity.Medium, vm.state.value.density)
        vm.density(GridDensity.Large)
        advanceUntilIdle()
        assertEquals(GridDensity.Large, settings.state.first().gridDensity)
        assertEquals(GridDensity.Large, vm.state.value.density)
        job.cancel()
    }
}
