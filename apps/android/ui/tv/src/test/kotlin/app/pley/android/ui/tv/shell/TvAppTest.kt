package app.pley.android.ui.tv.shell

import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.NavKey
import app.pley.android.common.TestTags
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.screens.PleyKey
import app.pley.android.ui.tv.kit.TvRow
import app.pley.android.ui.tv.kit.TvRowColumn
import app.pley.android.ui.tv.kit.focused
import app.pley.android.ui.tv.kit.press
import app.pley.android.ui.tv.kit.rememberFocusMemory
import app.pley.android.ui.tv.kit.tiles
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
 * coming back after Back.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-land-television-xhdpi")
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
    fun homeStartsFocused() = startsFocusedOn(home, "Home")

    @Test
    fun leftFromContentEntersTheRailAtTheCurrentPage() {
        app()
        compose.onNodeWithText("Open another screen").assertIsFocused()
        compose.press(Key.DirectionLeft)
        railEntry("home").assertIsFocused()
        // Right goes back to where focus was.
        compose.press(Key.DirectionRight)
        compose.onNodeWithText("Open another screen").assertIsFocused()
    }

    @Test
    fun theRailOpensItsPagesAndFocusFollows() {
        app()
        compose.press(Key.DirectionLeft)
        compose.press(Key.DirectionDown)
        railEntry("library:1").assertIsFocused()
        compose.press(Key.DirectionCenter)
        compose.onNodeWithText("Library isn't built yet").assertExists()
        compose.onNodeWithText("Open another screen").assertIsFocused()
    }

    @Test
    fun backGoesFromThePageToTheRailThenLeaves() {
        app()
        // A page opened from Home: Back returns to Home.
        compose.press(Key.DirectionCenter)
        compose.onNodeWithText("Libraries isn't built yet").assertExists()
        back()
        compose.onNodeWithText("Home isn't built yet").assertExists()
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

    /** Home as two rows of tiles that open items. */
    private fun rowsApp() = app(placeholder = { key ->
        if (key == PleyKey.Home) {
            val navigator = LocalTvNavigator.current
            val memory = rememberFocusMemory()
            TvRowColumn(contentPadding = PaddingValues(top = 188.dp)) {
                item("a") { TvRow("a", "Row A", tiles("A", 10), TileKind.Landscape, onOpen = { navigator.open(PleyKey.Item(it.id)) }, memory = memory, initialFocus = true) }
                item("b") { TvRow("b", "Row B", tiles("B", 10, from = 100), TileKind.Poster, onOpen = { navigator.open(PleyKey.Item(it.id)) }, memory = memory) }
                item("end") { Spacer(Modifier.height(200.dp)) }
            }
        } else {
            Placeholder(key)
        }
    })

    @Test
    fun rightFromTheRailComesBackToTheTile() {
        rowsApp()
        compose.press(Key.DirectionRight, times = 2)
        compose.press(Key.DirectionDown)
        compose.focused().assertContentDescriptionEquals("B 100")
        compose.press(Key.DirectionLeft)
        railEntry("home").assertIsFocused()
        compose.press(Key.DirectionRight)
        compose.focused().assertContentDescriptionEquals("B 100")
    }

    @Test
    fun aRowsFocusComesBackAfterBack() {
        rowsApp()
        compose.focused().assertContentDescriptionEquals("A 1")
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionRight, times = 3)
        compose.focused().assertContentDescriptionEquals("B 103")
        compose.press(Key.DirectionCenter)
        compose.onNodeWithText("Item isn't built yet").assertExists()
        back()
        compose.focused().assertContentDescriptionEquals("B 103")
    }
}
