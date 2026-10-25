package app.pley.android.ui.phone.item

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.PleyKey
import app.pley.android.ui.phone.person.PersonScreen
import app.pley.android.ui.phone.shell.Placeholder

/**
 * The phone's item and person pages and an item's action sheet (Item,
 * Person, ItemActions), added to the app's back stacks. The shell shows
 * ItemActions as a sheet over the screen it was opened from.
 */
fun EntryProviderScope<NavKey>.itemEntries() {
    entry<PleyKey.Item> { key -> WithGraph(key) { ItemScreen(key.itemId) } }
    entry<PleyKey.Person> { key -> WithGraph(key) { PersonScreen(key.personId) } }
    entry<PleyKey.ItemActions> { key -> WithGraph(key) { ItemActionsSheet(key.itemId) } }
}

/**
 * [content], whose ViewModels come from the app's graph; where the
 * Application holds none (the shell's own tests), the shell's placeholder.
 */
@Composable
private fun WithGraph(key: NavKey, content: @Composable () -> Unit) {
    if (LocalContext.current.applicationContext is PleyGraphOwner) content() else Placeholder(key)
}
