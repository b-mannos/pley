package app.pley.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LinksTest {
    private val server = "http://192.168.1.20:7539"

    private fun at(url: String, home: String = server) = Links.navigation(url, Places(home))

    @Test
    fun theWebViewKeepsToItsServer() {
        assertEquals(Navigation.Allow, at("http://192.168.1.20:7539/item/kx7bq2m"))
        assertEquals(Navigation.Servers, at("http://192.168.1.20:7539/__app/servers"))
        assertEquals(Navigation.Browser, at("https://www.themoviedb.org/movie/603"))
        assertEquals(Navigation.Browser, at("http://192.168.1.20:8080/"), "another port")
        assertEquals(Navigation.Browser, at("otpauth://totp/pley:bob?secret=JBSWY3DPEHPK3PXP&issuer=pley"), "authenticator apps")
        assertEquals(Navigation.Ignore, at("intent://scan/#Intent;end"))
        assertEquals(Navigation.Ignore, at("file:///sdcard/Download/x.html"))
        assertEquals(Navigation.Allow, at("https://pley.example.com/library/x", "https://pley.example.com"))
        assertEquals(Navigation.Allow, at("https://pley.example.com:443/", "https://pley.example.com"))
    }

    @Test
    fun singleSignOnPassesThroughTheProvidersPages() {
        val places = Places("https://pley.example.com")
        val go = { url: String -> Links.navigation(url, places) }
        assertEquals(Navigation.Browser, go("https://auth.example.com/authorize"), "outside a sign-in")
        assertEquals(Navigation.Allow, go("https://pley.example.com/api/auth/sso/start?next=%2F"))
        assertEquals(Navigation.Allow, go("https://auth.example.com/authorize?client_id=pley"))
        assertEquals(Navigation.Allow, go("https://pley.example.com/api/auth/sso/callback?code=c"))
        assertEquals(Navigation.Allow, go("https://pley.example.com/"))
        assertEquals(Navigation.Browser, go("https://auth.example.com/account"), "back at pley")
    }

    @Test
    fun aSignInAtThePublicAddressMovesTheAppThere() {
        val places = Places(server)
        for (step in listOf(
            "https://pley.example.com/api/auth/sso/start?next=%2F&hop=1",
            "https://auth.example.com/authorize?client_id=pley",
            "https://pley.example.com/api/auth/sso/callback?code=c",
            "https://pley.example.com/",
        )) {
            assertEquals(Navigation.Allow, Links.navigation(step, places), step)
        }
        org.junit.Assert.assertEquals("https://pley.example.com", places.server)
        assertNull(places.signingIn)
    }

    @Test
    fun addressesAreOriginsAsBrowsersWriteThem() {
        org.junit.Assert.assertEquals("https://pley.example.com", Links.address("https://pley.example.com:443/x?y"))
        org.junit.Assert.assertEquals("http://nas:7539", Links.address("http://NAS:7539/"))
        assertNull(Links.address("not a url"))
    }

    @Test
    fun theTokenComesFromTheSessionCookie() {
        assertEquals("abc123", Links.token("theme=dark; pley_token=abc123; other=1"))
        assertEquals("abc123", Links.token("pley_token=abc123"))
        assertNull(Links.token("theme=dark"))
        assertNull(Links.token("pley_token="))
        assertNull(Links.token(null))
    }

    private fun assertEquals(expected: Navigation, actual: Navigation, message: String) =
        org.junit.Assert.assertEquals(message, expected, actual)
}
