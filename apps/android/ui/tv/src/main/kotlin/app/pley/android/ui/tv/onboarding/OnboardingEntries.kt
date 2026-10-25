package app.pley.android.ui.tv.onboarding

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyKey

/**
 * The TV's screens for getting in, added to the app's back stack:
 * choosing a server (Servers), signing in with a code (Link, which SignIn
 * leads to as well: TVs never open a browser) and "Who's watching?"
 * (Profiles). No password field anywhere: passwords are typed only on
 * pley's own page, on the phone that approves.
 */
fun EntryProviderScope<NavKey>.onboardingEntries() {
    entry<PleyKey.Servers> { TvServersScreen() }
    entry<PleyKey.Link> { key -> TvLinkScreen(key.server) }
    entry<PleyKey.SignIn> { key -> TvLinkScreen(key.server) }
    entry<PleyKey.Profiles> { TvProfilesScreen() }
}
