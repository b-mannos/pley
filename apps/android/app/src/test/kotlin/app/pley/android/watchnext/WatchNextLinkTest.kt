package app.pley.android.watchnext

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Watch Next links play only when this installation made them: TvActivity is exported. */
class WatchNextLinkTest {
    private val key = ByteArray(32) { it.toByte() }
    private val alex = WatchNextOwner("http://nas:7539", 2)

    @Test
    fun aLinkThisInstallationSignedPlays() {
        val link = WatchNextLink.Link(alex, 42, WatchNextLink.sign(key, alex, 42))
        assertEquals(alex to 42L, WatchNextLink.verified(link, key))
    }

    @Test
    fun anyOtherAppsLinkDoesNot() {
        val signed = WatchNextLink.sign(key, alex, 42)
        // None at all, as another app sends it.
        assertNull(WatchNextLink.verified(WatchNextLink.Link(alex, 42, null), key))
        // Another item, profile or server under the same signature.
        assertNull(WatchNextLink.verified(WatchNextLink.Link(alex, 43, signed), key))
        assertNull(WatchNextLink.verified(WatchNextLink.Link(alex.copy(profileId = 3), 42, signed), key))
        assertNull(WatchNextLink.verified(WatchNextLink.Link(alex.copy(issuer = "http://other:7539"), 42, signed), key))
        // Another installation's key.
        val other = ByteArray(32) { (it + 1).toByte() }
        assertNull(WatchNextLink.verified(WatchNextLink.Link(alex, 42, WatchNextLink.sign(other, alex, 42)), key))
        assertNull(WatchNextLink.verified(WatchNextLink.Link(alex, 42, ""), key))
    }
}
