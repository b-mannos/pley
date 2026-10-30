package app.pley.android.ui.tv.livetv

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.PleyKey
import app.pley.android.ui.tv.shell.Placeholder

/**
 * Live TV on TVs, added to the app's back stack: the page with the guide
 * and the channels ([TvLiveTvScreen]), opened from the rail. The live
 * player is a window of its own ([TvLivePlayerActivity]).
 */
fun EntryProviderScope<NavKey>.liveTvEntries() {
    entry<PleyKey.LiveTv> { key -> WithGraph(key) { TvLiveTvScreen() } }
}

/**
 * [content], whose ViewModels come from the app's graph; where the
 * Application holds none (the shell's own tests), the shell's placeholder.
 */
@Composable
private fun WithGraph(key: NavKey, content: @Composable () -> Unit) {
    if (LocalContext.current.applicationContext is PleyGraphOwner) content() else Placeholder(key)
}
