package app.pley.android

import android.content.Context
import android.net.ConnectivityManager
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * Finding pley servers on the local network: `pley?` broadcast to UDP port
 * 7539, the answers collected for a moment (the server side is
 * apps/server/src/discovery.rs).
 */
object Discovery {
    private val REQUEST = "pley?".toByteArray()
    private const val WAIT_MS = 1500L

    /** The server behind an answer from `from`, if it is one. */
    fun readAnswer(bytes: ByteArray, length: Int, from: InetAddress): FoundServer? {
        val answer = try {
            JSONObject(String(bytes, 0, length, Charsets.UTF_8))
        } catch (_: org.json.JSONException) {
            return null
        }
        if (answer.optInt("pley") != 1) return null
        val port = answer.optInt("port")
        if (port <= 0) return null
        val host = when (from) {
            is Inet6Address -> "[${from.hostAddress?.substringBefore('%')}]"
            else -> from.hostAddress ?: return null
        }
        return FoundServer(
            url = "http://$host:$port",
            name = answer.optString("name", host),
            id = answer.optString("id").takeIf { it.isNotEmpty() },
            version = answer.optString("version"),
        )
    }

    /** The broadcast address of an IPv4 network: `192.168.1.20/24` → `192.168.1.255`. */
    fun broadcastOf(address: Inet4Address, prefixLength: Int): Inet4Address {
        val ip = address.address.fold(0) { acc, b -> (acc shl 8) or (b.toInt() and 0xff) }
        val mask = if (prefixLength <= 0) 0 else -1 shl (32 - prefixLength.coerceAtMost(32))
        val broadcast = ip or mask.inv()
        val bytes = ByteArray(4) { i -> (broadcast shr (24 - 8 * i)).toByte() }
        return InetAddress.getByAddress(bytes) as Inet4Address
    }

    /** Where to send the request: the current network's broadcast address, and the general one. */
    private fun targets(context: Context): List<InetAddress> {
        val out = mutableListOf<InetAddress>(InetAddress.getByName("255.255.255.255"))
        val connectivity = context.getSystemService(ConnectivityManager::class.java)
        val links = connectivity?.getLinkProperties(connectivity.activeNetwork)
        for (link in links?.linkAddresses.orEmpty()) {
            val address = link.address
            if (address is Inet4Address && !address.isLoopbackAddress) {
                val broadcast = broadcastOf(address, link.prefixLength)
                if (broadcast !in out) out.add(broadcast)
            }
        }
        return out
    }

    /** Servers that answered, one entry per server, by name. */
    suspend fun search(context: Context): List<FoundServer> = withContext(Dispatchers.IO) {
        val found = linkedMapOf<String, FoundServer>()
        try {
            DatagramSocket().use { socket ->
                socket.broadcast = true
                for (target in targets(context)) {
                    try {
                        socket.send(DatagramPacket(REQUEST, REQUEST.size, InetSocketAddress(target, DEFAULT_PORT)))
                    } catch (_: java.io.IOException) {
                        // That network doesn't take broadcasts: the others may.
                    }
                }
                val deadline = System.currentTimeMillis() + WAIT_MS
                val buffer = ByteArray(1024)
                while (true) {
                    val left = deadline - System.currentTimeMillis()
                    if (left <= 0) break
                    socket.soTimeout = left.toInt()
                    val packet = DatagramPacket(buffer, buffer.size)
                    try {
                        socket.receive(packet)
                    } catch (_: SocketTimeoutException) {
                        break
                    }
                    val server = readAnswer(packet.data, packet.length, packet.address) ?: continue
                    found.putIfAbsent(server.id ?: server.url, server)
                }
            }
        } catch (_: java.io.IOException) {
            // No network: nothing found.
        }
        found.values.sortedBy { it.name.lowercase() }
    }
}
