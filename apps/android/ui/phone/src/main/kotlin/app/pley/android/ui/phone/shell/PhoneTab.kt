package app.pley.android.ui.phone.shell

import androidx.annotation.StringRes
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.screens.PleyKey
import app.pley.android.ui.phone.R

/**
 * The navigation bar's (or rail's) five places (design spec §3.1), each
 * with its own back stack starting at [root].
 */
enum class PhoneTab(val root: PleyKey, val icon: PleyIcon, @StringRes val label: Int) {
    Home(PleyKey.Home, PleyIcons.House, R.string.shell_tab_home),
    Libraries(PleyKey.Libraries, PleyIcons.Clapperboard, R.string.shell_tab_libraries),
    Search(PleyKey.Search(), PleyIcons.Search, R.string.shell_tab_search),
    Playlists(PleyKey.Playlists, PleyIcons.ListMusic, R.string.shell_tab_playlists),
    Me(PleyKey.Me, PleyIcons.UserRound, R.string.shell_tab_me),
    ;

    companion object {
        /** The tab whose root [key] is: Search with any query counts. */
        fun rootOf(key: PleyKey): PhoneTab? = when (key) {
            PleyKey.Home -> Home
            PleyKey.Libraries -> Libraries
            is PleyKey.Search -> Search
            PleyKey.Playlists -> Playlists
            PleyKey.Me -> Me
            else -> null
        }
    }
}
