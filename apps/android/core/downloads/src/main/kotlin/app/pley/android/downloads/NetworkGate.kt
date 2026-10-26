package app.pley.android.downloads

import android.annotation.SuppressLint
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import app.pley.android.network.LocalRoute
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

/** Whether downloads may use the networks the device is on now. */
internal interface NetworkGate {
    /**
     * A network downloads from [server] may use is up: Wi-Fi or Ethernet
     * that isn't metered, or any when [mobile] data is allowed.
     */
    fun open(mobile: Boolean, server: String? = null): Boolean

    /** Waits until [open], for [timeoutMs] at most: whether it opened. */
    suspend fun await(mobile: Boolean, timeoutMs: Long, server: String? = null): Boolean
}

/**
 * [NetworkGate] from Android's networks. For a server at a home address
 * every network counts, not only the default: Wi-Fi with no internet
 * reaches it while the default network is mobile data (LocalRoute sends
 * its traffic there). Any other server is reached over the default
 * network, which alone counts then (a hotel's Wi-Fi before its sign-in
 * page leaves mobile data the default).
 */
@SuppressLint("MissingPermission")
internal class AndroidNetworkGate(context: Context) : NetworkGate {
    private val connectivity = context.applicationContext.getSystemService(ConnectivityManager::class.java)

    override fun open(mobile: Boolean, server: String?): Boolean {
        val cm = connectivity ?: return true
        if (server != null && !atHome(server)) return allows(cm.getNetworkCapabilities(cm.activeNetwork), mobile)
        // The only list of every network; the callback-based ways give one at a time.
        @Suppress("DEPRECATION")
        return cm.allNetworks.any { network -> allows(cm.getNetworkCapabilities(network), mobile) }
    }

    override suspend fun await(mobile: Boolean, timeoutMs: Long, server: String?): Boolean {
        if (open(mobile, server)) return true
        val cm = connectivity ?: return true
        val changes = callbackFlow {
            val callback = object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    trySend(Unit)
                }

                override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
                    trySend(Unit)
                }
            }
            cm.registerNetworkCallback(NetworkRequest.Builder().build(), callback)
            awaitClose { cm.unregisterNetworkCallback(callback) }
        }
        return withTimeoutOrNull(timeoutMs) { changes.first { open(mobile, server) } } != null
    }

    private fun allows(caps: NetworkCapabilities?, mobile: Boolean): Boolean {
        if (caps == null) return false
        // A VPN (Tailscale) is as metered as the network under it.
        val home = caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) || caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) ||
            caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)
        return (home && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)) ||
            (mobile && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET))
    }

    /** Whether [server] is at a home address or name, which LocalRoute reaches over the home network. */
    private fun atHome(server: String): Boolean = server.toHttpUrlOrNull()?.host?.let(LocalRoute::isHomeHost) ?: false
}
