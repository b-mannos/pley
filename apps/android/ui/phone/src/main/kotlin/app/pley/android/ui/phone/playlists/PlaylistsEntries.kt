package app.pley.android.ui.phone.playlists

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.PleyKey
import app.pley.android.screens.playlists.NewPlaylistMode
import app.pley.android.ui.phone.shell.Placeholder

/**
 * The phone's playlists and the sheets that add to them, added to the
 * app's back stacks: the Playlists tab ([PhonePlaylistsScreen]), a
 * playlist's page ([PhonePlaylistScreen]), and as bottom sheets over the
 * screen they were opened from, Add to playlist, New playlist and Save
 * these filters.
 */
fun EntryProviderScope<NavKey>.playlistsEntries() {
    entry<PleyKey.Playlists> { key -> WithGraph(key) { PhonePlaylistsScreen() } }
    entry<PleyKey.Playlist> { key -> WithGraph(key) { PhonePlaylistScreen(key.playlistId) } }
    entry<PleyKey.AddToPlaylist> { key -> WithGraph(key) { AddToPlaylistSheet(key.itemIds) } }
    entry<PleyKey.NewPlaylist> { key -> WithGraph(key) { NewPlaylistSheet(NewPlaylistMode.Items(key.itemIds)) } }
    entry<PleyKey.SaveSmartPlaylist> { key -> WithGraph(key) { NewPlaylistSheet(NewPlaylistMode.Smart(key.libraryId, key.query)) } }
}

/**
 * [content], whose ViewModels come from the app's graph; where the
 * Application holds none (the shell's own tests), the shell's placeholder.
 */
@Composable
private fun WithGraph(key: NavKey, content: @Composable () -> Unit) {
    if (LocalContext.current.applicationContext is PleyGraphOwner) content() else Placeholder(key)
}
