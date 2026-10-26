package app.pley.android.network

import app.pley.android.common.LocalNetwork.ACCESS_LOCAL_NETWORK
import app.pley.android.common.LocalNetwork.NEARBY_WIFI_DEVICES
import app.pley.android.testing.network.MockPley
import java.io.IOException
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Local network access, and the route to the home network over mobile data. */
class LocalNetworkTest {
    private fun access(sdk: Int, fireTv: Boolean = false, held: Set<String> = emptySet()) =
        PermissionsLocalNetworkAccess(sdk, fireTv) { it in held }

    @Test
    fun android17AsksForLocalNetworkAccess() {
        val access = access(37)
        assertEquals(listOf(ACCESS_LOCAL_NETWORK), access.required())
        assertEquals(listOf(ACCESS_LOCAL_NETWORK), access.missing())
        assertEquals(emptyList<String>(), access(37, held = setOf(ACCESS_LOCAL_NETWORK)).missing())
        assertEquals(emptyList<String>(), access(36).required())
        assertEquals(emptyList<String>(), access(26).missing())
    }

    @Test
    fun fireOs16AsksForNearbyDevicesToFindServers() {
        val access = access(36, fireTv = true)
        assertEquals(listOf(NEARBY_WIFI_DEVICES), access.missing())
        // Connecting doesn't need it.
        assertEquals(emptyList<String>(), access.missingFor("http://192.168.1.5:7539"))
        assertEquals(listOf(ACCESS_LOCAL_NETWORK, NEARBY_WIFI_DEVICES), access(37, fireTv = true).required())
    }

    @Test
    fun onlyHomeAddressesNeedTheirPermission() {
        val access = access(37)
        for (home in listOf("http://192.168.1.5:7539", "http://10.0.0.2", "http://172.20.1.1:7539", "http://nas:7539", "http://nas.local", "http://[fd12::5]:7539", "http://169.254.3.4")) {
            assertEquals(home, listOf(ACCESS_LOCAL_NETWORK), access.missingFor(home))
        }
        for (away in listOf("http://127.0.0.1:7539", "http://localhost:7539", "https://pley.example.com", "http://100.101.102.103:7539", "https://nas.tail1234.ts.net", "not a url")) {
            assertEquals(away, emptyList<String>(), access.missingFor(away))
        }
    }

    @Test
    fun aRefusalByTheSystemIsRecognised() {
        val access = access(37)
        assertTrue(access.isPermissionBlock(IOException("connect failed: EPERM (Operation not permitted)")))
        assertTrue(access.isPermissionBlock(OfflineException(IOException("sendto failed: EACCES (Permission denied)"))))
        assertFalse(access.isPermissionBlock(IOException("connect failed: ECONNREFUSED (Connection refused)")))
    }

    // ---------------------------------------------------------------- the route

    @Test
    fun theRouteIsNeededOnlyOverMobileData() {
        assertTrue(LocalRoute.needsBypass(cellular = true, home = false, vpn = false))
        assertFalse(LocalRoute.needsBypass(cellular = false, home = true, vpn = false))
        // A VPN (Tailscale) reaches home its own way.
        assertFalse(LocalRoute.needsBypass(cellular = true, home = false, vpn = true))
    }

    @Test
    fun homeAddressesAndNames() {
        for (home in listOf("192.168.1.5", "10.1.2.3", "172.16.0.1", "169.254.1.1", "fd00::1", "fe80::1")) {
            assertTrue(home, LocalRoute.isHomeAddress(InetAddress.getByName(home)))
        }
        for (away in listOf("127.0.0.1", "::1", "100.64.0.1", "8.8.8.8", "2001:db8::1")) {
            assertFalse(away, LocalRoute.isHomeAddress(InetAddress.getByName(away)))
        }
        for (home in listOf("nas", "nas.local", "pley.lan", "box.home.arpa")) assertTrue(home, LocalRoute.isHomeName(home))
        for (away in listOf("localhost", "pley.example.com", "nas.tail1234.ts.net", "192.168.1.5", "fd00::1")) {
            assertFalse(away, LocalRoute.isHomeName(away))
        }
        for (home in listOf("192.168.1.20", "10.0.2.2", "fd00::20", "[fd00::20]", "fe80::1", "nas", "nas.local")) {
            assertTrue(home, LocalRoute.isHomeHost(home))
        }
        for (away in listOf("8.8.8.8", "100.101.102.103", "127.0.0.1", "pley.example.com", "box.tail1234.ts.net", "2001:db8::1", "")) {
            assertFalse(away, LocalRoute.isHomeHost(away))
        }
    }

    private class Bypass {
        val bound = mutableListOf<Socket>()
        val looked = mutableListOf<String>()
        var on = true
        val lan = Lan(
            addresses = emptyList(),
            bindSocket = { bound += it },
            lookup = { looked += it; listOf(InetAddress.getByName("192.168.1.5")) },
        )
        val route = LocalRoute { if (on) lan else null }
    }

    @Test
    fun homeConnectionsGoThroughTheHomeNetwork() {
        val bypass = Bypass()
        val socket = bypass.route.socketFactory.createSocket()
        // Nothing answers there: the socket was bound before trying.
        runCatching { socket.connect(InetSocketAddress(InetAddress.getByName("10.255.255.1"), 7539), 50) }
        socket.close()
        assertEquals(listOf(socket), bypass.bound)
    }

    @Test
    fun otherConnectionsGoTheDefaultWay() {
        val bypass = Bypass()
        MockPley.server().use { server ->
            bypass.route.socketFactory.createSocket().use { it.connect(InetSocketAddress(InetAddress.getByName("127.0.0.1"), server.port), 1_000) }
        }
        bypass.on = false
        val socket = bypass.route.socketFactory.createSocket()
        runCatching { socket.connect(InetSocketAddress(InetAddress.getByName("10.255.255.1"), 7539), 50) }
        socket.close()
        assertEquals(emptyList<Socket>(), bypass.bound)
    }

    @Test
    fun homeNamesAreLookedUpOnTheHomeNetwork() {
        val bypass = Bypass()
        assertEquals(listOf(InetAddress.getByName("192.168.1.5")), bypass.route.dns.lookup("nas"))
        assertEquals(listOf("nas"), bypass.looked)
        // Addresses and names on the internet: the system's DNS.
        bypass.route.dns.lookup("127.0.0.1")
        assertEquals(listOf("nas"), bypass.looked)
    }
}
