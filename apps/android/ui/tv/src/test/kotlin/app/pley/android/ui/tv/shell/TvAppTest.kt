package app.pley.android.ui.tv.shell

import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.navigation3.runtime.NavKey
import app.pley.android.common.TestTags
import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.ui.tv.kit.focused
import app.pley.android.ui.tv.kit.press
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The frame with the D-pad and Back (D-pad rule 11): initial focus on every
 * start screen, the rail and content, the Back sequence, and a row's focus
 * coming back after Back. Home is the real one, over the JSON fixtures
 * ([TvTestGraph]); the screens not built yet are placeholders.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-land-television-xhdpi", application = TvTestApplication::class)
class TvAppTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private var exits = 0

    private class FakeSource(private val facts: StartFacts, libraries: List<Library>) : TvShellSource {
        override suspend fun startFacts() = facts

        override val libraries: Flow<List<Library>> = MutableStateFlow(libraries)

        override val profile: Flow<RailProfile?> = MutableStateFlow(RailProfile("Anna"))

        override suspend fun keepUpdated() = Unit
    }

    private val home = StartFacts(server = "http://192.168.1.20:7539", signedIn = true)
    private val threeLibraries = listOf(Library(1, "Movies", LibraryKind.Movies), Library(2, "Shows", LibraryKind.Shows), Library(3, "Music", LibraryKind.Music))

    private fun app(facts: StartFacts = home, placeholder: @androidx.compose.runtime.Composable (NavKey) -> Unit = { Placeholder(it) }) {
        compose.setContent {
            TvApp(FakeSource(facts, threeLibraries), onExit = { exits++ }, reducedMotion = true, placeholder = placeholder)
        }
        compose.waitForIdle()
    }

    private fun back() {
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
    }

    private fun railEntry(id: String) = compose.onNodeWithTag("${TestTags.TV_RAIL}.$id")

    private fun startsFocusedOn(facts: StartFacts, name: String) {
        app(facts)
        compose.onNodeWithText("$name isn't built yet").assertExists()
        compose.onNodeWithText("Open another screen").assertIsFocused()
    }

    @Test
    fun serversStartsFocused() = startsFocusedOn(StartFacts(null, false), "Servers")

    @Test
    fun linkStartsFocused() = startsFocusedOn(StartFacts("http://s", false), "Link")

    @Test
    fun whoIsWatchingStartsFocused() = startsFocusedOn(StartFacts("http://s", true, profiles = 2), "Profiles")

    @Test
    fun homeStartsFocused() {
        app()
        compose.onNodeWithText("Resume").assertIsFocused()
    }

    @Test
    fun leftFromContentEntersTheRailAtTheCurrentPage() {
        app()
        compose.onNodeWithText("Resume").assertIsFocused()
        compose.press(Key.DirectionLeft)
        railEntry("home").assertIsFocused()
        // Right goes back to where focus was.
        compose.press(Key.DirectionRight)
        compose.onNodeWithText("Resume").assertIsFocused()
    }

    @Test
    fun theRailOpensItsPagesAndFocusFollows() {
        app()
        compose.press(Key.DirectionLeft)
        compose.press(Key.DirectionDown)
        railEntry("library:1").assertIsFocused()
        // Library pages are built (they need the app's graph): a page still to come instead.
        compose.press(Key.DirectionDown, 3)
        railEntry("playlists").assertIsFocused()
        compose.press(Key.DirectionCenter)
        compose.onNodeWithText("Playlists isn't built yet").assertExists()
        compose.onNodeWithText("Open another screen").assertIsFocused()
    }

    @Test
    fun backGoesFromThePageToTheRailThenLeaves() {
        app()
        // A page opened from Home: Back returns to Home, on the tile it was opened from.
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionCenter)
        compose.onNodeWithText("Item isn't built yet").assertExists()
        back()
        compose.focused().assertContentDescriptionEquals(HALF_LOOP)
        assertEquals(0, exits)
        // On the rail's page: Back goes to the rail's current entry.
        back()
        railEntry("home").assertIsFocused()
        assertEquals(0, exits)
        // On the rail: Back leaves the app.
        back()
        assertEquals(1, exits)
    }

    @Test
    fun theSignInScreensHaveNoRail() {
        app(StartFacts(null, false))
        compose.onNode(hasTestTag(TestTags.TV_RAIL)).assertDoesNotExist()
        compose.press(Key.DirectionLeft)
        compose.onNodeWithText("Open another screen").assertIsFocused()
        back()
        assertEquals(1, exits)
    }

    @Test
    fun rightFromTheRailComesBackToTheTile() {
        app()
        compose.press(Key.DirectionDown, times = 2)
        compose.focused().assertContentDescriptionEquals(DUNE)
        compose.press(Key.DirectionLeft)
        railEntry("home").assertIsFocused()
        compose.press(Key.DirectionRight)
        compose.focused().assertContentDescriptionEquals(DUNE)
    }

    @Test
    fun aRowsFocusComesBackAfterBack() {
        app()
        compose.press(Key.DirectionDown, times = 2)
        compose.focused().assertContentDescriptionEquals(DUNE)
        compose.press(Key.DirectionCenter)
        compose.onNodeWithText("Item isn't built yet").assertExists()
        back()
        compose.focused().assertContentDescriptionEquals(DUNE)
    }

    private companion object {
        /** What the fixtures' tiles say to screen readers. */
        const val HALF_LOOP = "Half Loop, Severance, Season 1 Episode 2, 34 minutes left"
        const val DUNE = "Dune: Part Two, 2024"
    }
}
