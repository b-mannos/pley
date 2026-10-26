package app.pley.android.downloads

import android.annotation.SuppressLint
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

/** Whether downloads may use the networks the device is on now. */
internal interface NetworkGate {
    /** A network downloads may use is up: Wi-Fi or Ethernet that isn't metered, or any when [mobile] data is allowed. */
    fun open(mobile: Boolean): Boolean

    /** Waits until [open], for [timeoutMs] at most: whether it opened. */
    suspend fun await(mobile: Boolean, timeoutMs: Long): Boolean
}

/**
 * [NetworkGate] from Android's networks. Every network counts, not only
 * the default: Wi-Fi with no internet reaches a server at home while the
 * default network is mobile data (LocalRoute sends its traffic there).
 */
@SuppressLint("MissingPermission")
internal class AndroidNetworkGate(context: Context) : NetworkGate {
    private val connectivity = context.applicationContext.getSystemService(ConnectivityManager::class.java)

    override fun open(mobile: Boolean): Boolean {
        val cm = connectivity ?: return true
        // The only list of every network; the callback-based ways give one at a time.
        @Suppress("DEPRECATION")
        return cm.allNetworks.any { network -> allows(cm.getNetworkCapabilities(network), mobile) }
    }

    override suspend fun await(mobile: Boolean, timeoutMs: Long): Boolean {
        if (open(mobile)) return true
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
        return withTimeoutOrNull(timeoutMs) { changes.first { open(mobile) } } != null
    }

    private fun allows(caps: NetworkCapabilities?, mobile: Boolean): Boolean {
        if (caps == null) return false
        val home = caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) || caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)
        return (home && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)) ||
            (mobile && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET))
    }
}
