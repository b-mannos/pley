@file:OptIn(ExperimentalCoroutinesApi::class)

package app.pley.android.screens.home

import app.pley.android.data.Loadable
import app.pley.android.data.Session
import app.pley.android.model.Me
import app.pley.android.network.ApiException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class LibrariesViewModelTest {
    private val dispatcher = UnconfinedTestDispatcher()
    private val sources = FakeHomeSources()

    @Before
    fun main() = Dispatchers.setMain(dispatcher)

    @After
    fun reset() = Dispatchers.resetMain()

    private fun TestScope.libraries(): Pair<LibrariesViewModel, MutableList<LibrariesEvent>> {
        val vm = LibrariesViewModel({ sources.libraries }, { sources.scans }, dispatcher, session = { sources.sessionFlow })
        val events = mutableListOf<LibrariesEvent>()
        // Something on screen keeps the state flowing.
        backgroundScope.launch { vm.state.collect { } }
        backgroundScope.launch { vm.events.collect { events += it } }
        return vm to events
    }

    private fun LibrariesViewModel.names(): List<String> = (state.value as LibrariesState.Ready).libraries.map { it.library.name }

    @Test
    fun aMoveShowsAtOnceAndIsSavedForTheProfile() = runTest(dispatcher) {
        sources.libraryList.value = Loadable.Ready(listOf(MOVIES, SHOWS, MUSIC), fresh = true)
        val (vm, events) = libraries()
        assertTrue((vm.state.value as LibrariesState.Ready).sortable)

        vm.move(2, 0)
        assertEquals(listOf("Music", "Movies", "Shows"), vm.names())
        assertEquals(listOf(listOf(3L, 1L, 2L)), sources.sorts)

        // By one place, from where it is shown.
        vm.moveBy(MOVIES.id, 1)
        assertEquals(listOf("Music", "Shows", "Movies"), vm.names())
        assertEquals(listOf(3L, 2L, 1L), sources.sorts.last())
        // Not past either end.
        vm.moveBy(MOVIES.id, 1)
        vm.moveBy(MUSIC.id, -1)
        assertEquals(2, sources.sorts.size)
        assertTrue(events.isEmpty())
    }

    @Test
    fun aTvsMovesAreSavedWhenTheLibraryIsDroppedOrForgotten() = runTest(dispatcher) {
        sources.libraryList.value = Loadable.Ready(listOf(MOVIES, SHOWS, MUSIC), fresh = true)
        val (vm, _) = libraries()

        vm.moveBy(MOVIES.id, 1, save = false)
        vm.moveBy(MOVIES.id, 1, save = false)
        assertEquals(listOf("Shows", "Music", "Movies"), vm.names())
        assertTrue(sources.sorts.isEmpty())
        // Back puts it where it was.
        vm.cancelMoves()
        assertEquals(listOf("Movies", "Shows", "Music"), vm.names())

        vm.moveBy(SHOWS.id, -1, save = false)
        vm.saveOrder()
        assertEquals(listOf(listOf(2L, 1L, 3L)), sources.sorts)
        assertEquals(listOf("Shows", "Movies", "Music"), vm.names())

        vm.useServerOrder()
        assertEquals(1, sources.orderForgotten)
    }

    @Test
    fun anOrderTheServerRefusesGoesBackAndSaysSo() = runTest(dispatcher) {
        sources.libraryList.value = Loadable.Ready(listOf(MOVIES, SHOWS, MUSIC), fresh = true)
        val refused = ApiException(409, null, "your libraries have changed: the order must list each of them once")
        sources.sortFailure = refused
        val (vm, events) = libraries()

        vm.move(0, 2)
        assertEquals(listOf("Movies", "Shows", "Music"), vm.names())
        assertEquals(listOf<LibrariesEvent>(LibrariesEvent.Failed(refused)), events)
    }

    @Test
    fun aServerOlderThanSortingSaysSo() = runTest(dispatcher) {
        sources.libraryList.value = Loadable.Ready(listOf(MOVIES, SHOWS), fresh = true)
        sources.sortFailure = ApiException(404, "not_found", "no such endpoint")
        val (vm, events) = libraries()

        vm.move(0, 1)
        assertEquals(listOf("Movies", "Shows"), vm.names())
        assertEquals(listOf<LibrariesEvent>(LibrariesEvent.NeedsNewerServer), events)
    }

    @Test
    fun onlyTwoOrMoreLibrariesWithASignInAllowedToPlayCanBeSorted() = runTest(dispatcher) {
        sources.libraryList.value = Loadable.Ready(listOf(MOVIES), fresh = true)
        val (vm, _) = libraries()
        assertFalse((vm.state.value as LibrariesState.Ready).sortable)

        sources.libraryList.value = Loadable.Ready(listOf(MOVIES, SHOWS), fresh = true)
        assertTrue((vm.state.value as LibrariesState.Ready).sortable)
        val ann = Me(id = 4, username = "ann", name = "Ann")
        sources.sessionFlow.value = Session(FakeHomeSources.ORIGIN, ann, setOf("library"))
        assertFalse((vm.state.value as LibrariesState.Ready).sortable)
        vm.move(0, 1)
        assertTrue(sources.sorts.isEmpty())
        sources.sessionFlow.value = Session(FakeHomeSources.ORIGIN, ann, setOf("library", "play"))
        assertTrue((vm.state.value as LibrariesState.Ready).sortable)
    }
}
