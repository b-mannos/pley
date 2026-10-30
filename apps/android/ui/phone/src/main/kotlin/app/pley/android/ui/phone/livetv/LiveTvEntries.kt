package app.pley.android.ui.phone.livetv

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.PleyKey
import app.pley.android.ui.phone.shell.Placeholder

/**
 * Live TV on phones, added to the app's back stacks: the page with the
 * guide and the channels ([PhoneLiveTvScreen]), opened from the Libraries
 * tab. The live player is a window of its own ([LivePlayerActivity]).
 */
fun EntryProviderScope<NavKey>.liveTvEntries() {
    entry<PleyKey.LiveTv> { key -> WithGraph(key) { PhoneLiveTvScreen() } }
}

/**
 * [content], whose ViewModels come from the app's graph; where the
 * Application holds none (the shell's own tests), the shell's placeholder.
 */
@Composable
private fun WithGraph(key: NavKey, content: @Composable () -> Unit) {
    if (LocalContext.current.applicationContext is PleyGraphOwner) content() else Placeholder(key)
}
