package app.pley.android.network

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.Socket
import javax.net.SocketFactory

/** An IPv4 address of the device on a home network, with its network's prefix (`192.168.1.20/24`). */
data class LanAddress(val address: Inet4Address, val prefixLength: Int) {
    /** Where a broadcast to the whole network goes: `192.168.1.255`. */
    val broadcast: Inet4Address get() = app.pley.android.common.Discovery.broadcastOf(address, prefixLength)

    /**
     * The other addresses of the device's /24 (or of its network, when
     * that is smaller): `192.168.1.1` to `.254` but this device's own. A
     * bigger network is still searched by its /24 only: 254 addresses are
     * a few seconds' work, 65,534 are not.
     */
    fun neighbours(): List<Inet4Address> {
        val ip = address.address.fold(0) { acc, b -> (acc shl 8) or (b.toInt() and 0xff) }
        val prefix = prefixLength.coerceIn(24, 30)
        val mask = -1 shl (32 - prefix)
        val network = ip and mask
        val size = 1 shl (32 - prefix)
        return (1 until size - 1).map { network + it }.filter { it != ip }.map { n ->
            InetAddress.getByAddress(ByteArray(4) { i -> (n shr (24 - 8 * i)).toByte() }) as Inet4Address
        }
    }
}

/**
 * A Wi-Fi or Ethernet network the device is on: its addresses, and how to
 * send through it whatever the default network is (mobile data, when the
 * Wi-Fi has no internet).
 */
class Lan(
    val addresses: List<LanAddress>,
    /** Sends a socket's traffic through this network (Network.bindSocket). */
    val bindDatagram: (DatagramSocket) -> Unit = {},
    val bindSocket: (Socket) -> Unit = {},
    /** Looks a name up with this network's DNS (Network.getAllByName). */
    val lookup: (String) -> List<InetAddress> = { InetAddress.getAllByName(it).toList() },
    /** Sockets that go through this network. */
    val socketFactory: SocketFactory = SocketFactory.getDefault(),
)

/** The home networks the device is on now. */
interface LanNetworks {
    /** The Wi-Fi and Ethernet networks that are up, with their IPv4 addresses. */
    fun current(): List<Lan>

    /**
     * The one to reach the home network through when the default network
     * isn't it (mobile data while the Wi-Fi has no internet); `null` when
     * the default network reaches it already.
     */
    fun bypass(): Lan?
}

/** [LanNetworks] from Android's ConnectivityManager. */
internal class AndroidLanNetworks(context: Context) : LanNetworks {
    private val connectivity = context.applicationContext.getSystemService(ConnectivityManager::class.java)

    override fun current(): List<Lan> {
        val cm = connectivity ?: return emptyList()
        // The only list of every network; the callback-based ways give one network at a time.
        @Suppress("DEPRECATION")
        val networks = cm.allNetworks
        return networks.mapNotNull { network ->
            val caps = cm.getNetworkCapabilities(network) ?: return@mapNotNull null
            if (!isHome(caps)) return@mapNotNull null
            lanOf(cm, network)
        }
    }

    override fun bypass(): Lan? {
        val cm = connectivity ?: return null
        val default = cm.activeNetwork?.let { cm.getNetworkCapabilities(it) } ?: return null
        if (!LocalRoute.needsBypass(
                cellular = default.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR),
                home = isHome(default),
                vpn = default.hasTransport(NetworkCapabilities.TRANSPORT_VPN),
            )
        ) {
            return null
        }
        return current().firstOrNull()
    }

    private fun isHome(caps: NetworkCapabilities): Boolean =
        !caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN) &&
            (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) || caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET))

    private fun lanOf(cm: ConnectivityManager, network: Network): Lan? {
        val link = cm.getLinkProperties(network) ?: return null
        val addresses = link.linkAddresses.mapNotNull { la ->
            (la.address as? Inet4Address)?.takeIf { !it.isLoopbackAddress }?.let { LanAddress(it, la.prefixLength) }
        }
        if (addresses.isEmpty()) return null
        return Lan(
            addresses = addresses,
            bindDatagram = { network.bindSocket(it) },
            bindSocket = { network.bindSocket(it) },
            lookup = { network.getAllByName(it).toList() },
            socketFactory = network.socketFactory,
        )
    }
}
