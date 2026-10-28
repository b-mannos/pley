package app.pley.android.ui.tv.playlists

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.navigation3.runtime.NavKey
import app.pley.android.common.Tier
import app.pley.android.data.Loadable
import app.pley.android.data.PlaylistsRepository
import app.pley.android.data.valueOrNull
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.PlaylistDetail
import app.pley.android.model.PlaylistEntry
import app.pley.android.model.PlaylistKind
import app.pley.android.model.PlaylistSummary
import app.pley.android.player.PlayRequest
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.PleyKey
import app.pley.android.screens.playlists.PlaylistDeps
import app.pley.android.screens.playlists.PlaylistViewModel
import app.pley.android.ui.tv.kit.focused
import app.pley.android.ui.tv.kit.press
import app.pley.android.ui.tv.shell.LocalTvNavigator
import app.pley.android.ui.tv.shell.RailState
import app.pley.android.ui.tv.shell.TvNavigator
import app.pley.android.ui.tv.theme.TvTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * A playlist's page with the remote (design spec §4.9, D-pad rules 1 and
 * 6): focus starts on Play; an entry's More opens its actions; Move lifts
 * it, Up and Down move it, Select drops it and sends the order, Back puts
 * it back. Smart playlists offer no edits.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvPlaylistTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    /** The playlist as the data side keeps it; what was sent. */
    private class Playlists(start: PlaylistDetail) : PlaylistsRepository {
        val detail = MutableStateFlow<Loadable<PlaylistDetail>>(Loadable.Ready(start, fresh = true))
        val sent = mutableListOf<String>()

        override fun playlists(): Flow<Loadable<List<PlaylistSummary>>> = MutableStateFlow(Loadable.Loading)
        override fun playlist(playlistId: Long): Flow<Loadable<PlaylistDetail>> = detail
        override suspend fun create(title: String, kind: PlaylistKind?, itemIds: List<Long>): Long = error("unused")
        override suspend fun createSmart(title: String, libraryId: Long, query: String): Long = error("unused")
        override suspend fun rename(playlistId: Long, title: String) = Unit
        override suspend fun delete(playlistId: Long) = Unit
        override suspend fun add(playlistId: Long, itemIds: List<Long>) = Unit

        override suspend fun remove(playlistId: Long, entryIds: List<Long>) {
            sent += "remove $entryIds"
            val d = (detail.value as Loadable.Ready).value
            detail.value = Loadable.Ready(d.copy(entries = d.entries.filterNot { it.entryId in entryIds }), fresh = true)
        }

        override suspend fun reorder(playlistId: Long, entryIds: List<Long>) {
            sent += "reorder $entryIds"
            val d = (detail.value as Loadable.Ready).value
            detail.value = Loadable.Ready(d.copy(entries = entryIds.mapNotNull { id -> d.entries.firstOrNull { it.entryId == id } }), fresh = true)
        }
    }

    private val played = mutableListOf<PlayRequest>()

    private fun film(id: Long, title: String) =
        ItemSummary(id = id, libraryId = 1, kind = ItemKind.Movie, title = title, year = 2024, durationMs = 6_000_000)

    private fun detail(smart: String? = null) = PlaylistDetail(
        id = 1,
        title = "Road trip",
        kind = PlaylistKind.Video,
        itemCount = 3,
        durationMs = 18_000_000,
        smartFilter = smart,
        entries = listOf(film(10, "Northern Lights"), film(20, "Low Orbit"), film(30, "Halcyon"))
            .mapIndexed { i, item -> PlaylistEntry(i + 1L, item) },
    )

    private fun show(repo: Playlists): PlaylistViewModel<ArtworkTileModel> {
        val launcher = object : PlayerLauncher {
            override fun play(request: PlayRequest) {
                played += request
            }
        }
        val vm = PlaylistViewModel(1, PlaylistDeps(repo, MutableStateFlow("http://pley"), Dispatchers.Main.immediate), launcher, TvPlaylistTiles)
        val navigator = TvNavigator(mutableListOf<NavKey>(PleyKey.Playlists, PleyKey.Playlist(1)), RailState()) {}
        compose.setContent {
            TvTheme(Tier.T2, reducedMotion = true) {
                CompositionLocalProvider(LocalTvNavigator provides navigator) { TvPlaylistScreen(vm) }
            }
        }
        compose.waitForIdle()
        return vm
    }

    private fun PlaylistViewModel<ArtworkTileModel>.order() = state.value.valueOrNull!!.rows.map { it.entryId }

    private fun back() {
        compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
    }

    /** From Play: down to the first entry, right to its More, and its actions' Move. */
    private fun liftFirstEntry() {
        compose.press(Key.DirectionDown)
        compose.focused().assert(hasContentDescription("Northern Lights, 2024, 1h 40m", substring = true))
        compose.press(Key.DirectionRight)
        compose.focused().assert(hasContentDescription("More"))
        compose.press(Key.Enter)
        compose.onNodeWithTag(TV_PLAYLIST_PANEL).assertExists()
        compose.focused().assert(hasText("Play from here"))
        compose.press(Key.DirectionDown, 2)
        compose.focused().assert(hasText("Move"))
        compose.press(Key.Enter)
        compose.onNodeWithText("Up and down to move it, OK to drop it here, Back to put it back").assertExists()
        compose.focused().assert(hasContentDescription("Northern Lights, 2024, 1h 40m", substring = true))
    }

    @Test
    fun focusStartsOnPlayWhichPlaysTheQueue() {
        show(Playlists(detail()))
        compose.onNodeWithText("Play").assertIsFocused()
        compose.press(Key.Enter)
        assertEquals(1, played.size)
        assertEquals(10L, played.single().itemId)
    }

    @Test
    fun aLiftedEntryMovesWithUpAndDownAndSelectDropsIt() {
        val repo = Playlists(detail())
        val vm = show(repo)
        liftFirstEntry()
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionDown)
        // Moved on screen, focus still on it, nothing sent yet.
        assertEquals(listOf(2L, 3L, 1L), vm.order())
        compose.focused().assert(hasContentDescription("Northern Lights, 2024, 1h 40m", substring = true))
        assertEquals(emptyList<String>(), repo.sent)
        compose.press(Key.DirectionUp)
        compose.press(Key.Enter)
        assertEquals(listOf("reorder [2, 1, 3]"), repo.sent)
        compose.onNodeWithText("Up and down to move it, OK to drop it here, Back to put it back").assertDoesNotExist()
    }

    @Test
    fun backPutsALiftedEntryBack() {
        val repo = Playlists(detail())
        val vm = show(repo)
        liftFirstEntry()
        compose.press(Key.DirectionDown)
        assertEquals(listOf(2L, 1L, 3L), vm.order())
        back()
        assertEquals(listOf(1L, 2L, 3L), vm.order())
        assertEquals(emptyList<String>(), repo.sent)
        compose.focused().assert(hasContentDescription("Northern Lights, 2024, 1h 40m", substring = true))
    }

    @Test
    fun deletesQuestionKeepsFocusAndCancelGivesItBackToDelete() {
        show(Playlists(detail()))
        compose.press(Key.DirectionRight, 3)
        compose.focused().assert(hasContentDescription("Delete"))
        compose.press(Key.Enter)
        compose.focused().assert(hasText("Cancel"))
        // Nothing behind the scrim takes focus.
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionLeft)
        compose.focused().assert(hasText("Cancel"))
        compose.press(Key.Enter)
        compose.onNodeWithText("Cancel").assertDoesNotExist()
        compose.focused().assert(hasContentDescription("Delete"))
    }

    @Test
    fun removingTheOnlyEntryGivesFocusToDelete() {
        val repo = Playlists(detail().let { it.copy(itemCount = 1, entries = it.entries.take(1)) })
        show(repo)
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionRight)
        compose.press(Key.Enter)
        compose.press(Key.DirectionDown, 3)
        compose.focused().assert(hasText("Remove from playlist"))
        compose.press(Key.Enter)
        assertEquals(listOf("remove [1]"), repo.sent)
        compose.focused().assert(hasContentDescription("Delete"))
    }

    @Test
    fun aSmartPlaylistOffersNoEdits() {
        show(Playlists(detail(smart = "genre=Comedy")))
        compose.onNodeWithText("Smart playlist").assertExists()
        compose.press(Key.DirectionRight)
        compose.focused().assert(hasContentDescription("Shuffle"))
        compose.press(Key.DirectionRight)
        compose.focused().assert(hasContentDescription("Delete"))
        compose.press(Key.DirectionDown)
        compose.press(Key.DirectionRight)
        compose.press(Key.Enter)
        compose.onNodeWithText("Play from here").assertExists()
        compose.onNodeWithText("Move").assertDoesNotExist()
        compose.onNodeWithText("Remove from playlist").assertDoesNotExist()
    }
}
