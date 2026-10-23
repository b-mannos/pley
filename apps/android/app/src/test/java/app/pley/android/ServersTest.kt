package app.pley.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ServersTest {
    @Test
    fun addressesGetHttpAndPleysPort() {
        assertEquals("http://nas:7539", Servers.normalizeUrl("nas"))
        assertEquals("http://192.168.1.20:7539", Servers.normalizeUrl(" 192.168.1.20 "))
        assertEquals("http://nas:8080", Servers.normalizeUrl("nas:8080"))
        assertEquals("http://[fd00::5]:7539", Servers.normalizeUrl("[fd00::5]"))
    }

    @Test
    fun spelledOutAddressesKeepTheirSchemeAsOrigins() {
        assertEquals("https://pley.example.com", Servers.normalizeUrl("https://pley.example.com/"))
        assertEquals("http://nas", Servers.normalizeUrl("http://nas"))
        assertEquals("http://nas:7539", Servers.normalizeUrl("HTTP://NAS:7539/library/x"))
        assertEquals("https://pley.example.com", Servers.normalizeUrl("https://pley.example.com:443"))
    }

    @Test
    fun whatCantBeAServerIsRefused() {
        for (bad in listOf("", "   ", "ftp://nas", "file:///sdcard/x", "http://", "two words", "javascript:alert(1)")) {
            assertNull(bad, Servers.normalizeUrl(bad))
        }
    }

    @Test
    fun addressesLeavePleysPortOut() {
        assertEquals("192.168.1.20", Servers.displayAddress("http://192.168.1.20:7539"))
        assertEquals("nas:8080", Servers.displayAddress("http://nas:8080"))
        assertEquals("pley.example.com", Servers.displayAddress("https://pley.example.com"))
    }

    private val saved = listOf(
        SavedServer("http://nas:7539", "NAS", "a1", 10),
        SavedServer("http://office:7539", "Office", null, 20),
    )

    @Test
    fun savedServersComeFirstTheLastOpenedOnTop() {
        val found = listOf(
            FoundServer("http://192.168.1.20:7539", "NAS", "a1", "0.1.0"),
            FoundServer("http://192.168.1.30:7539", "Den", "b2", "0.1.0"),
        )
        assertEquals(
            listOf(
                ServerEntry("http://office:7539", "Office", null, saved = true, found = false),
                ServerEntry("http://nas:7539", "NAS", "a1", saved = true, found = true),
                ServerEntry("http://192.168.1.30:7539", "Den", "b2", saved = false, found = true),
            ),
            Servers.merge(saved, found),
        )
    }

    @Test
    fun serversWithoutAnIdMatchByAddress() {
        val found = listOf(FoundServer("http://office:7539", "Office", "c3", "0.1.0"))
        assertEquals(listOf(true to true, true to false), Servers.merge(saved, found).map { it.saved to it.found })
        assertEquals("Office", Servers.lastUsed(saved)?.name)
        assertNull(Servers.lastUsed(emptyList()))
    }

    @Test
    fun savingReplacesTheSameAddressOrId() {
        var list = Servers.withServer(saved, SavedServer("http://office:7539", "Office", null, 30))
        assertEquals(listOf("http://nas:7539" to 10L, "http://office:7539" to 30L), list.map { it.url to it.lastUsed })
        // The same server at a new address.
        list = Servers.withServer(list, SavedServer("http://192.168.1.5:7539", "NAS", "a1", 40))
        assertEquals(listOf("http://office:7539", "http://192.168.1.5:7539"), list.map { it.url })
    }

    @Test
    fun savedServersSurviveARoundTrip() {
        assertEquals(saved, ServerStore.decode(ServerStore.encode(saved)))
        assertEquals(emptyList<SavedServer>(), ServerStore.decode("not json"))
        assertEquals(emptyList<SavedServer>(), ServerStore.decode(null))
    }
}
