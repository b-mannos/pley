package app.pley.android.auth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PkceTest {
    @Test
    fun theChallengeIsRfc7636sAppendixB() {
        // The 32 octets of RFC 7636 Appendix B, their verifier, and its S256 challenge.
        val octets = intArrayOf(
            116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186,
            22, 212, 37, 77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132, 141, 121,
        ).map { it.toByte() }.toByteArray()
        assertEquals("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk", Pkce.verifier(octets))
        assertEquals("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", Pkce.challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"))
    }

    @Test
    fun verifiersAndStatesAre32RandomBytes() {
        val base64Url = Regex("^[A-Za-z0-9_-]{43}$")
        val verifiers = (1..50).map { Pkce.verifier() }
        val states = (1..50).map { Pkce.state() }
        for (value in verifiers + states) assertTrue(value, base64Url.matches(value))
        assertEquals(50, verifiers.toSet().size)
        assertEquals(50, states.toSet().size)
        assertNotEquals(verifiers.first(), Pkce.challenge(verifiers.first()))
        assertTrue(base64Url.matches(Pkce.challenge(verifiers.first())))
    }
}
