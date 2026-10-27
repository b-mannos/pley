package app.pley.android.data

import app.pley.android.common.Addresses
import app.pley.android.common.sha1Hex

/**
 * Whose things these are: profile [userId] on server [issuer]. Snapshots,
 * memory caches and the HTTP cache are kept per scope, so switching
 * profiles never shows one profile another's.
 */
data class ProfileScope(val issuer: String, val userId: Long) {
    /** The server's folder name: the SHA-1 of its origin, as the HTTP cache names it too. */
    val server: String get() = serverKey(issuer)

    companion object {
        /** The folder name of [issuer]'s snapshots and HTTP caches: the SHA-1 of its origin, in hex. */
        fun serverKey(issuer: String): String = sha1Hex(originOf(issuer))

        /** [issuer] as an origin, as NetworkGraph names the server's clients. */
        fun originOf(issuer: String): String = Addresses.origin(issuer) ?: issuer.trimEnd('/')
    }
}
