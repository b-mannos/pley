package app.pley.android.ui.tv.home

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyKey

/**
 * The TV's screens for Home and the list of libraries, added to the app's
 * back stack: Home ([TvHomeScreen]) and Libraries ([TvLibrariesScreen]),
 * which the rail's "Libraries" opens when there are more than three.
 */
fun EntryProviderScope<NavKey>.homeEntries() {
    entry<PleyKey.Home> { TvHomeScreen() }
    entry<PleyKey.Libraries> { TvLibrariesScreen() }
}
