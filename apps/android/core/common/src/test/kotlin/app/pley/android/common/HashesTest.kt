package app.pley.android.common

import org.junit.Assert.assertEquals
import org.junit.Test

class HashesTest {
    @Test
    fun aServersNameOnDiskIsTheSha1OfItsAddressInHex() {
        assertEquals("a94a8fe5ccb19ba61c4c0873d391e987982fbbd3", sha1Hex("test"))
        assertEquals(40, sha1Hex("http://192.168.1.20:7539").length)
    }
}
