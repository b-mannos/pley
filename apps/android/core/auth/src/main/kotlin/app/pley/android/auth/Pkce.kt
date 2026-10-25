package app.pley.android.auth

import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Base64

/**
 * PKCE (RFC 7636) with S256: the app keeps a random verifier, sends its
 * SHA-256 with the sign-in, and shows the verifier itself only to the
 * server's token endpoint. A code caught on the way back is useless without it.
 */
object Pkce {
    private val random = SecureRandom()

    /** base64url without padding (RFC 7636 Appendix A). */
    fun base64Url(bytes: ByteArray): String = Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

    /** 32 random bytes as 43 base64url characters (RFC 7636 §4.1). */
    fun verifier(bytes: ByteArray = randomBytes()): String {
        require(bytes.size == 32) { "a verifier is made of 32 bytes" }
        return base64Url(bytes)
    }

    /** BASE64URL(SHA-256(verifier)) (RFC 7636 §4.2). */
    fun challenge(verifier: String): String =
        base64Url(MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray(Charsets.US_ASCII)))

    /** A random state: 32 bytes, 43 characters, matched when the browser comes back. */
    fun state(): String = base64Url(randomBytes())

    private fun randomBytes(): ByteArray = ByteArray(32).also { random.nextBytes(it) }
}
