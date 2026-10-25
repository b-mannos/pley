package app.pley.android.ui.phone.home

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyKey

/**
 * The phone's screens for Home and the list of libraries, added to the
 * app's back stacks: Home ([PhoneHomeScreen]), the Home tab's first
 * screen, and Libraries ([PhoneLibrariesScreen]), the Libraries tab's.
 */
fun EntryProviderScope<NavKey>.homeEntries() {
    entry<PleyKey.Home> { PhoneHomeScreen() }
    entry<PleyKey.Libraries> { PhoneLibrariesScreen() }
}
