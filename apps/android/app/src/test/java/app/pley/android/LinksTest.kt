package app.pley.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LinksTest {
    private val server = "http://192.168.1.20:7539"

    @Test
    fun theWebViewKeepsToItsServer() {
        assertEquals(Navigation.Allow, Links.navigation("http://192.168.1.20:7539/item/kx7bq2m", server))
        assertEquals(Navigation.Servers, Links.navigation("http://192.168.1.20:7539/__app/servers", server))
        assertEquals(Navigation.Browser, Links.navigation("https://www.themoviedb.org/movie/603", server))
        assertEquals(Navigation.Browser, Links.navigation("http://192.168.1.20:8080/", server), "another port")
        assertEquals(Navigation.Ignore, Links.navigation("intent://scan/#Intent;end", server))
        assertEquals(Navigation.Allow, Links.navigation("https://pley.example.com/library/x", "https://pley.example.com"))
        assertEquals(Navigation.Allow, Links.navigation("https://pley.example.com:443/", "https://pley.example.com"))
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
