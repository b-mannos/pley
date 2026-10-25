package app.pley.android.ui.phone.shell

import androidx.compose.runtime.Immutable
import app.pley.android.screens.PleyKey

/**
 * What the first screen is chosen by, read from this device alone before
 * the first frame (the app's startup read): never the network or the
 * Keystore.
 */
@Immutable
data class PhoneStart(
    /** The server in use (its origin); null when none was ever opened. */
    val server: String?,
    /** Whether the app holds tokens for [server]. */
    val signedIn: Boolean,
)

/**
 * The first screen (architecture.md, "Start gates read local state only"):
 * no server, Servers; a server but no tokens, signing in to it; otherwise
 * Home, drawn from what was saved.
 */
fun startKey(start: PhoneStart): PleyKey = when {
    start.server == null -> PleyKey.Servers
    !start.signedIn -> PleyKey.SignIn(start.server)
    else -> PleyKey.Home
}

/** Whether [this] is a full-screen step of getting in, shown without the navigation bar. */
internal val PleyKey.isGettingIn: Boolean
    get() = this is PleyKey.Servers || this is PleyKey.SignIn || this is PleyKey.Link
