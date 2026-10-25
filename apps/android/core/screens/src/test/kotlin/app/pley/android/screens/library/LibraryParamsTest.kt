package app.pley.android.screens.library

import app.pley.android.data.LibraryQuery
import app.pley.android.model.LibraryKind
import app.pley.android.model.LibrarySort
import app.pley.android.model.SortOrder
import app.pley.android.screens.LibraryTab
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The library page's address, as the web's LibraryPage reads and writes it, and the listing it asks for. */
class LibraryParamsTest {
    @Test
    fun aFreshPageIsTheLibraryTabByTitle() {
        val params = LibraryParams.parse("")
        assertEquals(LibraryParams(), params)
        assertEquals("", params.toQueryString())
        assertEquals(LibraryQuery(LibrarySort.Title, SortOrder.Ascending), params.query())
    }

    @Test
    fun theQueryRoundTrips() {
        val params = LibraryParams(
            tab = LibraryTab.Library,
            sort = LibrarySort.Year,
            order = SortOrder.Ascending,
            watched = false,
            genre = "Science Fiction",
            year = 1999,
        )
        val query = params.toQueryString()
        assertEquals("sort=year&order=asc&watched=false&genre=Science+Fiction&year=1999", query)
        assertEquals(params, LibraryParams.parse(query))
    }

    @Test
    fun ordersAtTheirDefaultAreLeftOut() {
        assertEquals("sort=added", LibraryParams(sort = LibrarySort.Added, order = SortOrder.Descending).toQueryString())
        assertEquals("order=desc", LibraryParams(order = SortOrder.Descending).toQueryString())
        assertEquals("tab=collections", LibraryParams(tab = LibraryTab.Collections).toQueryString())
    }

    @Test
    fun sortsHaveTheWebsDefaultOrders() {
        assertEquals(SortOrder.Ascending, LibraryParams(sort = LibrarySort.Title).effectiveOrder)
        for (sort in LibrarySort.entries - LibrarySort.Title) {
            assertEquals(sort.name, SortOrder.Descending, LibraryParams(sort = sort).effectiveOrder)
        }
        // Choosing a sort forgets the order chosen for the one before.
        val flipped = LibraryParams(sort = LibrarySort.Year).flipped()
        assertEquals(SortOrder.Ascending, flipped.effectiveOrder)
        assertEquals(SortOrder.Descending, flipped.sortedBy(LibrarySort.Rating).effectiveOrder)
    }

    @Test
    fun unknownValuesFallBack() {
        val params = LibraryParams.parse("tab=nope&sort=loudness&order=up&watched=maybe&year=soon&genre=")
        assertEquals(LibraryParams(), params)
        // A route's tab, unless the query names one.
        assertEquals(LibraryTab.Recommended, LibraryParams.parse("", LibraryTab.Recommended).tab)
        assertEquals(LibraryTab.Collections, LibraryParams.parse("tab=collections", LibraryTab.Recommended).tab)
    }

    @Test
    fun watchedIsTheWebsAndTheListingAsksForUnwatched() {
        assertEquals(true, LibraryParams(watched = false).query().unwatched)
        assertEquals(false, LibraryParams(watched = true).query().unwatched)
        assertEquals(null, LibraryParams().query().unwatched)
    }

    @Test
    fun eachKindHasTheWebsSorts() {
        assertEquals(9, LibraryParams.sortsFor(LibraryKind.Movies).size)
        assertFalse(LibrarySort.Duration in LibraryParams.sortsFor(LibraryKind.Shows))
        assertTrue(LibrarySort.Bitrate in LibraryParams.sortsFor(LibraryKind.Shows))
        assertEquals(
            listOf(LibrarySort.Title, LibrarySort.Added, LibrarySort.UserRating, LibrarySort.LastPlayed),
            LibraryParams.sortsFor(LibraryKind.Music),
        )
        assertEquals(listOf(LibraryTab.Recommended, LibraryTab.Library), LibraryParams.tabsFor(LibraryKind.Music))
    }

    @Test
    fun musicKeepsNoFiltersOrCollections() {
        val params = LibraryParams(tab = LibraryTab.Collections, sort = LibrarySort.Bitrate, order = SortOrder.Ascending, watched = false, genre = "Jazz")
        assertEquals(LibraryParams(tab = LibraryTab.Library), params.validFor(LibraryKind.Music))
        // Shows can't sort by duration: back to titles, in their order.
        assertEquals(LibraryParams(), LibraryParams(sort = LibrarySort.Duration, order = SortOrder.Descending).validFor(LibraryKind.Shows))
        // What a kind offers stays as it is.
        val movies = LibraryParams(sort = LibrarySort.Duration, order = SortOrder.Ascending, year = 2001)
        assertEquals(movies, movies.validFor(LibraryKind.Movies))
    }

    @Test
    fun clearingKeepsTheSort() {
        val params = LibraryParams(sort = LibrarySort.Rating, watched = true, genre = "Drama", year = 2020)
        assertTrue(params.filtered)
        assertEquals(LibraryParams(sort = LibrarySort.Rating), params.cleared())
        assertFalse(params.cleared().filtered)
    }
}
