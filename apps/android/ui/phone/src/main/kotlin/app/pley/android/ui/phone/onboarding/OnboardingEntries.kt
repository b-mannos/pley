package app.pley.android.ui.phone.onboarding

import androidx.navigation3.runtime.EntryProviderScope
import androidx.navigation3.runtime.NavKey
import app.pley.android.screens.PleyKey

/**
 * The phone's screens for getting in, added to the app's back stack:
 * choosing a server and signing in on its page in the browser (Servers,
 * and SignIn for a server the app was signed out of), a code from
 * another device (Link) and switching profiles (Profiles, from Me). No
 * password or two-step field anywhere: those are typed only on pley's
 * own page, in the browser.
 */
fun EntryProviderScope<NavKey>.onboardingEntries() {
    entry<PleyKey.Servers> { PhoneSignInScreen(preselect = null) }
    entry<PleyKey.SignIn> { key -> PhoneSignInScreen(preselect = key.server) }
    entry<PleyKey.Link> { key -> PhoneLinkScreen(key.server) }
    entry<PleyKey.Profiles> { PhoneProfilesScreen() }
}
