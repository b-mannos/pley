package app.pley.android.ui.tv.playlists

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.PleyKey
import app.pley.android.screens.playlists.NewPlaylistMode
import app.pley.android.ui.tv.shell.Placeholder

/**
 * The TV's playlists and the panels that add to them, added to the app's
 * back stack: Playlists ([TvPlaylistsScreen]), a playlist's page
 * ([TvPlaylistScreen]), and as side panels over a backdrop, Add to
 * playlist, New playlist and Save these filters.
 */
fun EntryProviderScope<NavKey>.playlistsEntries() {
    entry<PleyKey.Playlists> { key -> WithGraph(key) { TvPlaylistsScreen() } }
    entry<PleyKey.Playlist> { key -> WithGraph(key) { TvPlaylistScreen(key.playlistId) } }
    entry<PleyKey.AddToPlaylist> { key -> WithGraph(key) { TvAddToPlaylistScreen(key.itemIds) } }
    entry<PleyKey.NewPlaylist> { key -> WithGraph(key) { TvNewPlaylistScreen(NewPlaylistMode.Items(key.itemIds)) } }
    entry<PleyKey.SaveSmartPlaylist> { key -> WithGraph(key) { TvNewPlaylistScreen(NewPlaylistMode.Smart(key.libraryId, key.query)) } }
}

/**
 * [content], whose ViewModels come from the app's graph; where the
 * Application holds none (the shell's own tests), the shell's placeholder.
 */
@Composable
private fun WithGraph(key: NavKey, content: @Composable () -> Unit) {
    if (LocalContext.current.applicationContext is PleyGraphOwner) content() else Placeholder(key)
}
