package app.pley.android.network

import android.content.Context
import app.pley.android.common.Servers
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketAddress
import javax.net.SocketFactory
import okhttp3.Dns
import okhttp3.OkHttpClient

/**
 * Reaching the home network while mobile data is the default network.
 * Android makes mobile data the default when the Wi-Fi has no internet
 * (a home network with a NAS and no router to the internet, a hotel's
 * network before its sign-in page): connections to the NAS would then go
 * out over mobile data and never arrive. With this, a connection to a home
 * address (RFC 1918, link-local, IPv6 unique local) goes through the Wi-Fi
 * or Ethernet network instead, and home names (`nas`, `nas.local`) are
 * looked up with its DNS. Everything else, Tailscale's addresses included,
 * goes the default way.
 */
class LocalRoute internal constructor(private val bypass: () -> Lan?) {
    /** Sockets that connect to home addresses through the home network when [bypass] says so. */
    val socketFactory: SocketFactory = object : SocketFactory() {
        override fun createSocket(): Socket = RoutedSocket()

        override fun createSocket(host: String, port: Int): Socket =
            RoutedSocket().apply { connect(InetSocketAddress(host, port)) }

        override fun createSocket(host: String, port: Int, localHost: InetAddress, localPort: Int): Socket =
            RoutedSocket().apply {
                bind(InetSocketAddress(localHost, localPort))
                connect(InetSocketAddress(host, port))
            }

        override fun createSocket(host: InetAddress, port: Int): Socket =
            RoutedSocket().apply { connect(InetSocketAddress(host, port)) }

        override fun createSocket(address: InetAddress, port: Int, localAddress: InetAddress, localPort: Int): Socket =
            RoutedSocket().apply {
                bind(InetSocketAddress(localAddress, localPort))
                connect(InetSocketAddress(address, port))
            }
    }

    /** Home names looked up on the home network when [bypass] says so; the rest as usual. */
    val dns: Dns = Dns { host ->
        val lan = if (isHomeName(host)) bypass() else null
        lan?.lookup?.invoke(host) ?: Dns.SYSTEM.lookup(host)
    }

    /** Sets [builder] up to route home connections this way. */
    fun applyTo(builder: OkHttpClient.Builder): OkHttpClient.Builder = builder.socketFactory(socketFactory).dns(dns)

    private inner class RoutedSocket : Socket() {
        override fun connect(endpoint: SocketAddress?, timeout: Int) {
            val address = (endpoint as? InetSocketAddress)?.address
            if (address != null && isHomeAddress(address)) bypass()?.bindSocket?.invoke(this)
            super.connect(endpoint, timeout)
        }
    }

    companion object {
        /** The route of this device's networks. */
        fun of(context: Context): LocalRoute {
            val networks = AndroidLanNetworks(context)
            return LocalRoute(networks::bypass)
        }

        /**
         * Whether the default network misses the home network: it is mobile
         * data, not a Wi-Fi or Ethernet network, and not a VPN (Tailscale
         * reaches home its own way).
         */
        fun needsBypass(cellular: Boolean, home: Boolean, vpn: Boolean): Boolean = cellular && !home && !vpn

        /** An address on a home network: private IPv4, link-local, IPv6 unique local. Not this device, not Tailscale's. */
        fun isHomeAddress(address: InetAddress): Boolean = when {
            address.isLoopbackAddress -> false
            address.isSiteLocalAddress || address.isLinkLocalAddress -> true
            address is Inet6Address -> (address.address[0].toInt() and 0xfe) == 0xfc
            address is Inet4Address -> false
            else -> false
        }

        /** A name only the home network's DNS knows: one without dots, or under `.local`, `.lan`, `.home`… */
        fun isHomeName(host: String): Boolean {
            val h = host.lowercase()
            if (h == "localhost" || h.endsWith(".ts.net")) return false
            if (Regex("""^[\d.]+$""").matches(h) || h.contains(':')) return false
            return Servers.isLocalHost(h)
        }
    }
}
