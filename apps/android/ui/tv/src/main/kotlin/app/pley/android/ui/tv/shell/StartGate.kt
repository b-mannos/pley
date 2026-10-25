package app.pley.android.ui.tv.shell

import app.pley.android.screens.PleyKey

/**
 * What the TV's first screen is chosen by, read from this device only:
 * never the network, so the first frame waits for nothing.
 */
data class StartFacts(
    /** The server in use (its origin); `null` when none was opened. */
    val server: String?,
    /** Whether the app holds tokens for [server], read without the Keystore. */
    val signedIn: Boolean,
    /** How many profiles the household had when last seen; 1 while unknown. */
    val profiles: Int = 1,
    /** Settings' "Always start as" profile; `null` when not set. */
    val alwaysStartAs: Long? = null,
)

/**
 * The TV's start gate (tv-addendum §3.3): no server → Servers; no tokens →
 * Link; a household of several profiles without "Always start as" → Who's
 * watching?; otherwise Home.
 */
fun startKey(facts: StartFacts): PleyKey = when {
    facts.server == null -> PleyKey.Servers
    !facts.signedIn -> PleyKey.Link(facts.server)
    facts.profiles > 1 && facts.alwaysStartAs == null -> PleyKey.Profiles
    else -> PleyKey.Home
}
