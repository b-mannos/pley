package app.pley.android.screens.admin.network

import app.pley.android.model.admin.NetworkAdapter
import app.pley.android.model.admin.NetworkFixed
import app.pley.android.model.admin.ServerNetwork
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Network's rules, as the web's page has them (NetworkSettings.test.tsx). */
class NetworkFormTest {
    private val network = ServerNetwork(web = listOf("127.0.0.1:7539"), plexOnWeb = true, discovery = true)

    @Test
    fun theFormIsWhereTheServerListens() {
        assertEquals(ListenMode.Local, formOf(network.choice).mode)
        assertEquals(ListenMode.All, formOf(network.copy(web = listOf("0.0.0.0:7539")).choice).mode)
        val networks = formOf(network.copy(web = listOf("127.0.0.1:8000", "192.168.1.10:8000")).choice)
        assertEquals(ListenMode.Networks, networks.mode)
        assertEquals(listOf("192.168.1.10"), networks.ips)
        assertEquals(8000, networks.port)
        assertEquals(ApiForm(on = true, onWeb = true, port = 32400), networks.plex)
        assertEquals(ApiForm(on = false, onWeb = true, port = 8096), networks.jellyfin)
        // On ports of its own: the first's.
        assertEquals(ApiForm(on = true, onWeb = false, port = 8920), formOf(network.copy(jellyfin = listOf("0.0.0.0:8920")).choice).jellyfin)
        assertEquals(HostPort("::1", 7539), splitAddress("[::1]:7539"))
    }

    @Test
    fun theChoiceIsTheFormsAddressesButWhatTheCommandLineSets() {
        val form = formOf(network.choice).copy(mode = ListenMode.Networks, ips = listOf("192.168.1.10"), jellyfin = ApiForm(true, false, 8096))
        val choice = choiceOf(form, network)
        assertEquals(listOf("127.0.0.1:7539", "192.168.1.10:7539"), choice.web)
        assertEquals(listOf("127.0.0.1:8096", "192.168.1.10:8096"), choice.jellyfin)
        assertFalse(choice.jellyfinOnWeb)
        assertTrue(choice.plexOnWeb)

        val fixed = network.copy(plex = listOf("0.0.0.0:32400"), plexOnWeb = false, fixed = NetworkFixed(plex = true))
        val off = choiceOf(formOf(fixed.choice).copy(plex = ApiForm(false, false, 32400)), fixed)
        assertEquals(listOf("0.0.0.0:32400"), off.plex)

        // An IPv6 address is written in brackets.
        assertEquals(listOf("127.0.0.1:7539", "[fd00::5]:7539"), choiceOf(form.copy(ips = listOf("fd00::5"), jellyfin = ApiForm(false, true, 8096)), network).web)
    }

    @Test
    fun saysWhyAFormCantBeSaved() {
        val all = formOf(network.copy(web = listOf("0.0.0.0:7539")).choice)
        assertNull(problemOf(all))
        assertEquals(NetworkProblem.Port, problemOf(all.copy(port = 0)))
        assertEquals(NetworkProblem.JellyfinSamePort, problemOf(all.copy(jellyfin = ApiForm(true, false, 7539))))
        assertEquals(NetworkProblem.PlexPortRange, problemOf(all.copy(plex = ApiForm(true, false, 70000))))
        assertEquals(
            NetworkProblem.SharedPort,
            problemOf(all.copy(jellyfin = ApiForm(true, false, 32400), plex = ApiForm(true, false, 32400))),
        )
        // On Pley's port, its own port doesn't count.
        assertNull(problemOf(all.copy(jellyfin = ApiForm(true, true, 7539))))
        assertEquals(NetworkProblem.NoNetwork, problemOf(all.copy(mode = ListenMode.Networks, ips = emptyList())))
        assertEquals(NetworkProblem.HostName, problemOf(all.copy(bonjour = true, bonjourHost = "my media")))
        // Hidden, a host name that can't be saved doesn't stop the rest: the saved one stays.
        assertNull(problemOf(all.copy(bonjour = false, bonjourHost = "my media")))
        assertEquals("", choiceOf(all.copy(bonjourHost = "my media"), network).bonjourHostName)
    }

    @Test
    fun aHostNameIsKeptAsItsLabel() {
        assertEquals("media", hostLabel(" Media.local. "))
        assertTrue(hostNameOk(""))
        assertTrue(hostNameOk("living-room"))
        assertFalse(hostNameOk("-media"))
        assertFalse(hostNameOk("my media"))
        assertFalse(hostNameOk("a".repeat(64)))
    }

    @Test
    fun anAdapterIsChosenWithEveryAddressItHas() {
        val form = formOf(network.choice).copy(mode = ListenMode.Networks, ips = listOf("10.9.9.9"))
        val ethernet = NetworkAdapter("Ethernet", up = true, addresses = listOf("192.168.1.10", "192.168.1.11"))
        val chosen = toggleAdapter(form, ethernet)
        assertEquals(listOf("10.9.9.9", "192.168.1.10", "192.168.1.11"), chosen.ips)
        assertEquals(listOf("10.9.9.9"), toggleAdapter(chosen, ethernet).ips)
        // Chosen before, on an adapter that isn't here now (a VPN that is off).
        assertEquals(listOf("10.9.9.9"), addressesElsewhere(chosen, listOf(ethernet)))
    }

    @Test
    fun bonjourIsOfferedOnlyTheNetworksTheServerListensOn() {
        val ethernet = NetworkAdapter("Ethernet", up = true, addresses = listOf("192.168.1.10"))
        val wifi = NetworkAdapter("Wi-Fi", up = false, addresses = listOf("10.0.0.5"))
        val form = formOf(network.copy(web = listOf("127.0.0.1:7539", "192.168.1.10:7539"), bonjour = true, bonjourAdapters = listOf("Wi-Fi")).choice)
        assertEquals(BonjourChoices(listOf(ethernet), listOf("Wi-Fi")), bonjourChoices(form, listOf(ethernet, wifi)))
        assertEquals(BonjourChoices(listOf(ethernet, wifi), emptyList()), bonjourChoices(form.copy(mode = ListenMode.All), listOf(ethernet, wifi, ethernet.copy(addresses = listOf("192.168.1.12")))))
    }

    @Test
    fun knowsWhenThisPhoneWouldLoseTheServer() {
        assertEquals(false, stillServes(listOf("127.0.0.1:7539"), listOf("127.0.0.1:8000"), "http://localhost:7539"))
        // From another device, when the server stops listening on its network.
        assertEquals(false, stillServes(listOf("0.0.0.0:7539"), listOf("127.0.0.1:7539"), "http://192.168.1.10:7539"))
        assertEquals(true, stillServes(listOf("0.0.0.0:7539"), listOf("127.0.0.1:7539", "192.168.1.10:7539"), "http://192.168.1.10:7539"))
        assertEquals(false, stillServes(listOf("[::]:7539"), listOf("127.0.0.1:7539"), "http://[fd00::5]:7539"))
        // Through a reverse proxy, or at a name, which can't be seen past.
        assertNull(stillServes(listOf("127.0.0.1:7539"), listOf("127.0.0.1:8000"), "https://pley.example.com"))
        assertNull(stillServes(listOf("127.0.0.1:7539"), listOf("127.0.0.1:8000"), "https://localhost"))
        assertNull(stillServes(listOf("0.0.0.0:7539"), listOf("127.0.0.1:7539"), "http://nas.local:7539"))
    }
}
