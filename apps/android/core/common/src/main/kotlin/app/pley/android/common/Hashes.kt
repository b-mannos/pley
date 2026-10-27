package app.pley.android.common

import java.security.MessageDigest

/**
 * The SHA-1 of [text] (its UTF-8 bytes) in lower-case hex: 40 characters
 * that name what the app keeps per server on disk, whatever its address
 * holds.
 */
fun sha1Hex(text: String): String {
    val digest = MessageDigest.getInstance("SHA-1").digest(text.toByteArray(Charsets.UTF_8))
    return buildString(digest.size * 2) {
        for (b in digest) append(HEX[(b.toInt() shr 4) and 0xf]).append(HEX[b.toInt() and 0xf])
    }
}

private const val HEX = "0123456789abcdef"
