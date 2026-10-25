package app.pley.android.data

import app.pley.android.common.Addresses
import java.security.MessageDigest

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
        fun serverKey(issuer: String): String = sha1(originOf(issuer))

        /** [issuer] as an origin, as NetworkGraph names the server's clients. */
        fun originOf(issuer: String): String = Addresses.origin(issuer) ?: issuer.trimEnd('/')

        internal fun sha1(text: String): String {
            val digest = MessageDigest.getInstance("SHA-1").digest(text.toByteArray(Charsets.UTF_8))
            return buildString(digest.size * 2) {
                for (b in digest) append(HEX[(b.toInt() shr 4) and 0xf]).append(HEX[b.toInt() and 0xf])
            }
        }

        private const val HEX = "0123456789abcdef"
    }
}
