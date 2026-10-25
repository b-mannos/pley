package app.pley.android.network

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat
import app.pley.android.common.LocalNetwork
import java.net.InetAddress
import java.net.URI
import java.net.URISyntaxException

/**
 * [LocalNetworkAccess] for a device on API [sdk] (a Fire TV when
 * [fireTv]), where [granted] says whether the app holds a permission.
 */
internal class PermissionsLocalNetworkAccess(
    private val sdk: Int,
    private val fireTv: Boolean,
    private val granted: (String) -> Boolean,
) : LocalNetworkAccess {
    override fun required(): List<String> = LocalNetwork.permissions(sdk, fireTv)

    override fun missing(): List<String> = required().filterNot(granted)

    override fun missingFor(url: String): List<String> {
        val host = try {
            URI(url).host
        } catch (_: URISyntaxException) {
            null
        } ?: return emptyList()
        if (!atHome(host)) return emptyList()
        // Connections need Android 17's permission only; Fire OS 16's is for finding devices.
        return missing().filter { it == LocalNetwork.ACCESS_LOCAL_NETWORK }
    }

    override fun isPermissionBlock(error: Throwable): Boolean = LocalNetwork.isPermissionBlock(error)

    /** Whether [host] is on the home network: a private or link-local address, or a name only it knows. */
    private fun atHome(host: String): Boolean {
        val h = host.removePrefix("[").removeSuffix("]")
        val literal = Regex("""^[\d.]+$""").matches(h) || h.contains(':')
        if (!literal) return LocalRoute.isHomeName(h)
        return try {
            // A literal address: no lookup happens.
            LocalRoute.isHomeAddress(InetAddress.getByName(h))
        } catch (_: Exception) {
            false
        }
    }

    companion object {
        /** This device's. */
        fun of(context: Context): PermissionsLocalNetworkAccess {
            val app = context.applicationContext
            return PermissionsLocalNetworkAccess(
                sdk = Build.VERSION.SDK_INT,
                fireTv = app.packageManager.hasSystemFeature(LocalNetwork.FIRE_TV_FEATURE),
                granted = { ContextCompat.checkSelfPermission(app, it) == PackageManager.PERMISSION_GRANTED },
            )
        }
    }
}
