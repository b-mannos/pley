package app.pley.android.ui.phone.shell

import android.content.res.Resources
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import app.pley.android.auth.AuthEvent
import app.pley.android.common.describe
import app.pley.android.data.Loadable
import app.pley.android.network.ApiException
import app.pley.android.network.OfflineException
import app.pley.android.player.NowPlaying
import app.pley.android.screens.PleyKey
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.kit.ToastHostState

/**
 * What the shell shows around every screen, and how screens tell it
 * about what went wrong (design spec §3.4, §8):
 *
 * - the toasts, above the navigation bar;
 * - the offline banner, while what shows came from what was saved
 *   because the server can't be reached;
 * - a refusal for want of a scope (403 insufficient_scope): a toast
 *   offering to sign in again, which asks for it;
 * - being signed out by the server: back to signing in, keeping the
 *   server, with the reason.
 *
 * Screens reach it through [LocalPhoneShell].
 */
@Stable
class PhoneShellState internal constructor(
    private val navigation: PhoneNavigation,
    private val resources: Resources,
    server: String?,
) {
    val toasts = ToastHostState()

    internal val offlineState = mutableStateOf(false)

    /** Whether what shows came from what was saved, the server out of reach. */
    val offline: Boolean get() = offlineState.value

    /** The server in use (its origin); null before one is chosen. */
    var server: String? by mutableStateOf(server)
        internal set

    /** Whether the server is scanning a library now (only known while the app is in the foreground). */
    var scanning: Boolean by mutableStateOf(false)
        internal set

    /** The music playing in the background, for the mini player. */
    var nowPlaying: NowPlaying? by mutableStateOf(null)
        internal set

    /**
     * A screen's load: saved data shown because the server couldn't be
     * reached raises the offline banner; a fresh answer lowers it.
     */
    fun report(load: Loadable<*>) {
        when (load) {
            is Loadable.Ready -> if (load.fresh) offlineState.value = false
            is Loadable.Failed -> if (load.error is OfflineException) offlineState.value = true
            Loadable.Loading -> Unit
        }
    }

    /**
     * Something a person did failed with [error]: the scope toast for a
     * missing scope, the offline banner when the server is out of reach,
     * otherwise a toast with the error's own sentence.
     */
    fun report(error: Throwable) {
        when {
            error is ApiException && error.needsScope -> toasts.show(
                message = resources.getString(R.string.shell_scope),
                actionLabel = resources.getString(R.string.shell_scope_action),
                onAction = ::signInAgain,
                error = true,
            )
            error is OfflineException -> {
                offlineState.value = true
                toasts.show(sentence(describe(error)), error = true)
            }
            else -> toasts.show(sentence(describe(error)), error = true)
        }
    }

    /** Signs in to the server again (keeping it), or chooses one when there is none. */
    fun signInAgain() {
        navigation.restart(server?.let { PleyKey.SignIn(it) } ?: PleyKey.Servers)
    }

    /** The sign-in module ended a sign-in: if it was this server's, back to signing in to it. */
    internal fun on(event: AuthEvent) {
        when (event) {
            is AuthEvent.SignedOut -> {
                val current = server
                if (!navigation.inside || (current != null && current != event.issuer)) return
                offlineState.value = false
                navigation.restart(PleyKey.SignIn(event.issuer))
                toasts.show(sentence(event.reason.message), error = true)
            }
        }
    }

    private fun sentence(text: String): String {
        val capital = text.replaceFirstChar { it.uppercase() }
        return if (capital.endsWith('.') || capital.endsWith('…')) capital else "$capital."
    }
}

/** The shell around the screen: toasts, the offline banner, error handling. */
val LocalPhoneShell = staticCompositionLocalOf<PhoneShellState> { error("No phone shell here") }
