package app.pley.android.auth

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/** Why the app was signed out of a server without being asked to. */
enum class SignOutReason(val message: String) {
    /** The server ended the sign-in: a refresh or a profile switch answered invalid_grant. */
    ENDED("you were signed out: sign in again"),

    /** This device can't read its tokens anymore (the Keystore's key went with a reset or a restore). */
    UNREADABLE("this device can't read its sign-in anymore: sign in again"),

    /** The server allowed more than the app asks for, so the app ended that sign-in itself. */
    WIDER_SCOPE(WIDER_SCOPE_MESSAGE),
}

/** Something about signing in that the screens should hear about, wherever it happens. */
sealed interface AuthEvent {
    /** The app holds no tokens for [issuer] anymore: show sign-in. */
    data class SignedOut(val issuer: String, val reason: SignOutReason) : AuthEvent
}

/**
 * The sign-in module's events, one stream per process. Sending never waits:
 * nobody listening means nobody misses it, and a slow listener loses the
 * oldest.
 */
class AuthEvents {
    private val flow = MutableSharedFlow<AuthEvent>(extraBufferCapacity = 16, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    val events: SharedFlow<AuthEvent> = flow.asSharedFlow()

    fun signedOut(issuer: String, reason: SignOutReason) {
        flow.tryEmit(AuthEvent.SignedOut(issuer, reason))
    }
}
