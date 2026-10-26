package app.pley.android.ui.tv.shell

import androidx.compose.ui.graphics.Color
import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.screens.PleyKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The start gate (tv-addendum §3.3) and what the rail lists (design spec §2.1). */
class ShellRulesTest {
    @Test
    fun startGoesToServersWithoutAServer() {
        assertEquals(PleyKey.Servers, startKey(StartFacts(server = null, signedIn = false)))
    }

    @Test
    fun startGoesToLinkWithoutTokens() {
        assertEquals(PleyKey.Link("http://192.168.1.20:7539"), startKey(StartFacts("http://192.168.1.20:7539", signedIn = false)))
    }

    @Test
    fun startAsksWhoIsWatchingInAHouseholdWithoutAlwaysStartAs() {
        assertEquals(PleyKey.Profiles, startKey(StartFacts("http://s", signedIn = true, profiles = 3)))
        assertEquals(PleyKey.Home, startKey(StartFacts("http://s", signedIn = true, profiles = 3, alwaysStartAs = 2)))
        assertEquals(PleyKey.Home, startKey(StartFacts("http://s", signedIn = true, profiles = 1)))
    }

    private fun libraries(n: Int) = (1..n).map { Library(it.toLong(), "Library $it", if (it % 2 == 0) LibraryKind.Shows else LibraryKind.Movies) }

    @Test
    fun theRailListsUpToThreeLibrariesThenCollections() {
        val items = railItems(libraries(3))
        assertEquals(
            listOf("search", "home", "library:1", "library:2", "library:3", "playlists", "collections"),
            items.map { it.id },
        )
        assertEquals(PleyKey.Collections(1), items.last().key)
    }

    @Test
    fun moreThanThreeLibrariesAddLibrariesInCollectionsPlace() {
        val items = railItems(libraries(5))
        assertEquals(
            listOf("search", "home", "library:1", "library:2", "library:3", "libraries", "playlists"),
            items.map { it.id },
        )
        // Seven entries between the avatar and Settings, as the rail has room for.
        assertEquals(7, items.size)
    }

    @Test
    fun whileMusicPlaysNowPlayingComesLastWithinTheRailsSevenEntries() {
        assertEquals(
            "in Collections' place",
            listOf("search", "home", "library:1", "library:2", "library:3", "playlists", "now-playing"),
            railItems(libraries(3), nowPlaying = true).map { it.id },
        )
        assertEquals(
            "in the last library's, which Libraries lists",
            listOf("search", "home", "library:1", "library:2", "libraries", "playlists", "now-playing"),
            railItems(libraries(5), nowPlaying = true).map { it.id },
        )
        assertEquals(
            "added where there is room",
            listOf("search", "home", "library:1", "library:2", "playlists", "collections", "now-playing"),
            railItems(libraries(2), nowPlaying = true).map { it.id },
        )
        val items = railItems(libraries(3), nowPlaying = true)
        assertEquals("now-playing", railCurrent(PleyKey.NowPlaying, items))
    }

    @Test
    fun aRailWithoutLibrariesHasNoCollections() {
        assertEquals(listOf("search", "home", "playlists"), railItems(emptyList()).map { it.id })
    }

    @Test
    fun theCurrentEntryFollowsTheBackStacksFirstPage() {
        val items = railItems(libraries(5)) + SettingsItem
        assertEquals("home", railCurrent(PleyKey.Home, items))
        assertEquals("search", railCurrent(PleyKey.Search("north"), items))
        assertEquals("library:2", railCurrent(PleyKey.Library(2), items))
        assertEquals("libraries", railCurrent(PleyKey.Library(5), items))
        assertEquals("libraries", railCurrent(PleyKey.Libraries, items))
        assertEquals("settings", railCurrent(PleyKey.Settings, items))
        assertEquals(PROFILE_ITEM, railCurrent(PleyKey.Profiles, items))
        assertNull(railCurrent(PleyKey.Servers, items))
    }

    @Test
    fun profileColoursParseAsTheWebWritesThem() {
        assertEquals(Color(0xFFE07A5F), RailProfile.parseColor("#e07a5f"))
        assertNull(RailProfile.parseColor("teal"))
        assertEquals("A", RailProfile("anna").initial)
    }
}
