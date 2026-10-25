package app.pley.android.screens.library

import app.pley.android.model.LibrarySort
import app.pley.android.model.SortOrder
import org.junit.Assert.assertEquals
import org.junit.Test

/** apps/web/src/lib/smartFilter.test.ts's cases. */
class SmartFilterTest {
    @Test
    fun theLibraryPagesFiltersInPlexsSyntax() {
        assertEquals(
            "genre=Science+Fiction&unwatched=1&sort=year%3Adesc",
            SmartFilter.of(LibraryParams(sort = LibrarySort.Year, order = SortOrder.Descending, watched = false, genre = "Science Fiction")),
        )
        assertEquals(
            "year=1999&unwatched=0&sort=titleSort",
            SmartFilter.of(LibraryParams(sort = LibrarySort.Title, order = SortOrder.Ascending, watched = true, year = 1999)),
        )
    }

    @Test
    fun filtersInWords() {
        assertEquals(
            listOf("Genre: Comedy", "Unwatched", "Newest first"),
            SmartFilter.describe("type=1&genre=Comedy&unwatched=1&sort=year%3Adesc"),
        )
        // As python-plexapi writes them.
        assertEquals(
            listOf("After 2019", "Recently added first", "At most 20"),
            SmartFilter.describe("year%3E%3E=2019&sort=movie.addedAt:desc&includeGuids=1&limit=20"),
        )
        assertEquals(listOf("resolution: 4k"), SmartFilter.describe("resolution=4k"))
    }

    @Test
    fun whatItSavesReadsBackAsTheSameWords() {
        val filter = SmartFilter.of(LibraryParams(sort = LibrarySort.Added, watched = false, genre = "Drama", year = 2001))
        assertEquals(listOf("Genre: Drama", "From 2001", "Unwatched", "Recently added first"), SmartFilter.describe(filter))
    }
}
