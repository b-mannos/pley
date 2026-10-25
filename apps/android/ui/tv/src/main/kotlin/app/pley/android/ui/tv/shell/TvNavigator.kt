package app.pley.android.ui.tv.shell

import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.Navigator
import app.pley.android.screens.PleyKey

/**
 * The TV's navigation: one back stack (Navigation 3), started over by the
 * rail, and the TV's Back (tv-addendum §3.3, design spec §5.2 rule 6):
 *
 * 1. overlays close first (their own BackHandlers, nearer the screen);
 * 2. a page opened from another goes back to it;
 * 3. a page the rail opened gives focus to the rail's current entry;
 * 4. Back on the rail leaves the app. Never "press again to exit".
 */
@Stable
class TvNavigator internal constructor(
    private val backStack: MutableList<NavKey>,
    private val rail: RailState,
    private val exit: () -> Unit,
) : Navigator {
    /** The page the stack starts at, which the rail shows as current. */
    val root: PleyKey? get() = backStack.firstOrNull() as? PleyKey

    override fun open(key: PleyKey) {
        backStack.add(key)
    }

    override fun back() {
        when {
            rail.hasFocus -> exit()
            backStack.size > 1 -> backStack.removeAt(backStack.lastIndex)
            !rail.focusCurrent() -> exit()
        }
    }

    override fun restart(key: PleyKey) {
        if (backStack.size == 1 && backStack[0] == key) return
        backStack.clear()
        backStack.add(key)
    }
}

/** The TV's navigator, for screens: the shell provides it. */
val LocalTvNavigator = staticCompositionLocalOf<TvNavigator> { error("No TvNavigator: screens run inside TvApp") }
