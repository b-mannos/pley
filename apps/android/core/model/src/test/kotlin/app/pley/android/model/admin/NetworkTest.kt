package app.pley.android.model.admin

import app.pley.android.model.PleyJson
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Network's JSON as the server writes it (apps/server/src/api/admin.rs, `NetworkInfo`) and reads it (`Listen`). */
class NetworkTest {
    @Test
    fun readsWhereTheServerListens() {
        val network = PleyJson.decodeFromString(
            ServerNetwork.serializer(),
            """
            {
              "web": ["127.0.0.1:7539", "192.168.1.10:7539"],
              "plex": [], "plex_on_web": true,
              "jellyfin": ["0.0.0.0:8096"], "jellyfin_on_web": false,
              "discovery": true, "bonjour": true,
              "bonjour_adapters": ["Ethernet"], "bonjour_host_name": "media",
              "fixed": { "web": false, "plex": true, "jellyfin": false, "discovery": false, "bonjour": false },
              "listeners": [
                { "kind": "web", "address": "127.0.0.1:7539", "error": null },
                { "kind": "jellyfin", "address": "0.0.0.0:8096", "error": "address in use" },
                { "kind": "discovery", "address": "192.168.1.10:7539", "error": null }
              ],
              "adapters": [
                { "name": "Ethernet", "description": "A network card", "up": true, "addresses": ["192.168.1.10"],
                  "network": { "name": "Home", "kind": "private" } },
                { "name": "Wi-Fi", "description": null, "up": false, "addresses": ["10.0.0.5"], "network": null }
              ],
              "bonjour_host_default": "pley-0123456789ab",
              "config_file": "/config/pley.toml",
              "windows": false
            }
            """,
        )
        assertEquals(listOf("127.0.0.1:7539", "192.168.1.10:7539"), network.web)
        assertEquals(true, network.plexOnWeb)
        assertEquals(listOf("0.0.0.0:8096"), network.jellyfin)
        assertEquals(NetworkFixed(plex = true), network.fixed)
        assertEquals(ListenerKind.Jellyfin, network.listeners[1].kind)
        assertEquals("address in use", network.listeners[1].error)
        assertNull(network.listeners[0].error)
        assertEquals(NetworkProfile("Home", "private"), network.adapters[0].network)
        assertEquals(false, network.adapters[1].up)
        assertEquals("pley-0123456789ab", network.bonjourHostDefault)
        assertEquals("/config/pley.toml", network.configFile)
        assertEquals(
            ListenChoice(
                web = listOf("127.0.0.1:7539", "192.168.1.10:7539"),
                jellyfin = listOf("0.0.0.0:8096"),
                jellyfinOnWeb = false,
                plex = emptyList(),
                plexOnWeb = true,
                discovery = true,
                bonjour = true,
                bonjourAdapters = listOf("Ethernet"),
                bonjourHostName = "media",
            ),
            network.choice,
        )
    }

    @Test
    fun aChoiceSendsEveryFieldAsTheServerReadsIt() {
        val choice = ServerNetwork(web = listOf("0.0.0.0:7539"), plexOnWeb = true, discovery = true).choice
        val body = PleyJson.encodeToJsonElement(ListenChoice.serializer(), choice).jsonObject
        // Left out, the server would take an API, discovery or Bonjour for off.
        assertEquals(
            setOf("web", "jellyfin", "jellyfin_on_web", "plex", "plex_on_web", "discovery", "bonjour", "bonjour_adapters", "bonjour_host_name"),
            body.keys,
        )
        assertEquals("true", body.getValue("plex_on_web").toString())
    }

    @Test
    fun proxyAddressesAreNoneFromAServerFromBeforeThem() {
        assertNull(PleyJson.decodeFromString(ProxySettings.serializer(), """{ "server_name": "Den" }""").proxyAddresses)
        assertEquals(
            listOf("https://media.example.com:9999"),
            PleyJson.decodeFromString(ProxySettings.serializer(), """{ "proxy_addresses": ["https://media.example.com:9999"] }""").proxyAddresses,
        )
    }
}
