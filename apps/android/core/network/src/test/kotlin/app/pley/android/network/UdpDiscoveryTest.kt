package app.pley.android.network

import app.pley.android.common.FoundServer
import java.io.IOException
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class UdpDiscoveryTest {
    private fun ip(text: String): InetAddress = InetAddress.getByName(text)

    private fun lan(vararg addresses: String, prefix: Int = 24) =
        Lan(addresses.map { LanAddress(ip(it) as Inet4Address, prefix) })

    private class Lans(private val lans: List<Lan>) : LanNetworks {
        override fun current(): List<Lan> = lans

        override fun bypass(): Lan? = null
    }

    /** A socket of a test: what it sent and when, answers fed to it. */
    private class FakePort(private val now: () -> Long) : DatagramPort {
        val sent = mutableListOf<Pair<Long, InetSocketAddress>>()
        val answers = Channel<Datagram>(Channel.UNLIMITED)
        var sendError: IOException? = null
        var closed = false

        override fun send(payload: ByteArray, to: InetSocketAddress) {
            assertEquals("pley?", String(payload))
            sendError?.let { throw it }
            sent += now() to to
        }

        override suspend fun receive(): Datagram? = answers.receive()

        override fun close() {
            closed = true
        }

        fun answer(json: String, from: String) {
            val bytes = json.toByteArray()
            answers.trySend(Datagram(bytes, bytes.size, InetAddress.getByName(from)))
        }
    }

    private fun TestScope.discovery(lans: List<Lan>, ports: MutableMap<Lan?, FakePort>, failOpen: IOException? = null) = UdpDiscovery(
        networks = Lans(lans),
        open = { lan ->
            failOpen?.let { throw it }
            FakePort { testScheduler.currentTime }.also { ports[lan] = it }
        },
        io = StandardTestDispatcher(testScheduler),
    )

    private fun reply(name: String, id: String?, port: Int = 7539) =
        """{"pley":1,"name":"$name",${if (id != null) "\"id\":\"$id\"," else ""}"version":"0.42.0","port":$port}"""

    @Test
    fun sendsThreeTimesToEveryNetworkThenListensUntil1500ms() = runTest {
        val home = lan("192.168.1.20")
        val ports = mutableMapOf<Lan?, FakePort>()
        val events = discovery(listOf(home), ports).search().toList()

        assertEquals(listOf(DiscoveryEvent.Done), events)
        assertEquals(1_500L, testScheduler.currentTime)
        val port = ports.getValue(home)
        val targets = listOf(InetSocketAddress(ip("192.168.1.255"), 7539), InetSocketAddress(ip("255.255.255.255"), 7539))
        assertEquals(listOf(0L, 200L, 600L).flatMap { at -> targets.map { at to it } }, port.sent)
        assertTrue(port.closed)
    }

    @Test
    fun eachNetworkGetsItsOwnSocket() = runTest {
        val wifi = lan("192.168.1.20")
        val ethernet = lan("10.0.0.5", prefix = 16)
        val ports = mutableMapOf<Lan?, FakePort>()
        discovery(listOf(wifi, ethernet), ports).search().toList()

        assertEquals(setOf(wifi, ethernet), ports.keys)
        assertEquals(setOf(ip("10.0.255.255"), ip("255.255.255.255")), ports.getValue(ethernet).sent.map { it.second.address }.toSet())
    }

    @Test
    fun withoutAHomeNetworkTheDefaultOneIsAsked() = runTest {
        val ports = mutableMapOf<Lan?, FakePort>()
        discovery(emptyList(), ports).search().toList()

        assertEquals(setOf<Lan?>(null), ports.keys)
        assertEquals(List(3) { ip("255.255.255.255") }, ports.getValue(null).sent.map { it.second.address })
    }

    @Test
    fun eachServerComesOnceByItsId() = runTest {
        val wifi = lan("192.168.1.20")
        val ethernet = lan("192.168.50.20")
        val ports = mutableMapOf<Lan?, FakePort>()
        val search = discovery(listOf(wifi, ethernet), ports).search()
        val collected = backgroundScope.collectAll(search)
        testScheduler.runCurrent()

        ports.getValue(wifi).apply {
            answer(reply("Living Room", "abc"), "192.168.1.5")
            answer(reply("Living Room", "abc"), "192.168.1.5")
            answer("""{"not":"pley"}""", "192.168.1.9")
            answer("garbage", "192.168.1.9")
            answer(reply("Old pley", id = null, port = 8080), "192.168.1.7")
        }
        // The same server, seen from the other network.
        ports.getValue(ethernet).answer(reply("Living Room", "abc"), "192.168.50.5")

        assertEquals(
            listOf(
                DiscoveryEvent.Found(FoundServer("http://192.168.1.5:7539", "Living Room", "abc", "0.42.0")),
                DiscoveryEvent.Found(FoundServer("http://192.168.1.7:8080", "Old pley", null, "0.42.0")),
                DiscoveryEvent.Done,
            ),
            collected.await(),
        )
    }

    @Test
    fun aRefusalByTheSystemIsBlocked() = runTest {
        val ports = mutableMapOf<Lan?, FakePort>()
        val refused = IOException("socket failed: EPERM (Operation not permitted)")
        val events = discovery(listOf(lan("192.168.1.20")), ports, failOpen = refused).search().toList()

        assertEquals(listOf(DiscoveryEvent.Blocked, DiscoveryEvent.Done), events)
        // Nothing to wait for.
        assertEquals(0L, testScheduler.currentTime)
    }

    @Test
    fun aRefusedSendIsBlockedOnceAndTheSearchGoesOn() = runTest {
        val home = lan("192.168.1.20")
        val search = UdpDiscovery(
            networks = Lans(listOf(home)),
            open = { FakePort { testScheduler.currentTime }.apply { sendError = IOException("sendto failed: EPERM (Operation not permitted)") } },
            io = StandardTestDispatcher(testScheduler),
        )
        assertEquals(listOf(DiscoveryEvent.Blocked, DiscoveryEvent.Done), search.search().toList())
        assertEquals(1_500L, testScheduler.currentTime)
    }

    @Test
    fun aNetworkThatTakesNoBroadcastIsNotBlocked() = runTest {
        val search = UdpDiscovery(
            networks = Lans(listOf(lan("192.168.1.20"))),
            open = { FakePort { testScheduler.currentTime }.apply { sendError = IOException("sendto failed: ENETUNREACH (Network is unreachable)") } },
            io = StandardTestDispatcher(testScheduler),
        )
        assertEquals(listOf(DiscoveryEvent.Done), search.search().toList())
    }
}

/** Collects [flow] to a list in this scope. */
private fun <T> CoroutineScope.collectAll(flow: Flow<T>) = async { flow.toList() }
