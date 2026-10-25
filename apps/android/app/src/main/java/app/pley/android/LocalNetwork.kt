package app.pley.android

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat

/**
 * Asking to reach devices on the home network. Android 17 asks people
 * before an app that targets it talks to their network
 * (ACCESS_LOCAL_NETWORK): without it, discovery fails with EPERM and a
 * connection to a server at home looks like one that never answers. Fire
 * OS 16 asks before an app looks for devices (NEARBY_WIFI_DEVICES, never
 * used for location).
 */
object LocalNetwork {
    const val ACCESS_LOCAL_NETWORK = "android.permission.ACCESS_LOCAL_NETWORK"
    const val NEARBY_WIFI_DEVICES = "android.permission.NEARBY_WIFI_DEVICES"

    /** What Fire TV devices report, and other Android TVs don't. */
    const val FIRE_TV_FEATURE = "amazon.hardware.fire_tv"

    /** Android 17's API level. */
    const val ANDROID_17 = 37

    /** Fire OS 16's API level (Android 16). */
    const val FIRE_OS_16 = 36

    /** The permissions a device on [sdk] asks for (a Fire TV when [fireTv]). */
    fun permissions(sdk: Int, fireTv: Boolean): List<String> = buildList {
        if (sdk >= ANDROID_17) add(ACCESS_LOCAL_NETWORK)
        if (fireTv && sdk >= FIRE_OS_16) add(NEARBY_WIFI_DEVICES)
    }

    /** Those of [permissions] this device asks for and the app doesn't hold yet. */
    fun missing(context: Context): List<String> =
        permissions(Build.VERSION.SDK_INT, context.packageManager.hasSystemFeature(FIRE_TV_FEATURE))
            .filter { ContextCompat.checkSelfPermission(context, it) != PackageManager.PERMISSION_GRANTED }

    /** Whether the app has to ask before finding or opening a server at home. */
    fun needsRequest(context: Context): Boolean = missing(context).isNotEmpty()

    /**
     * Whether [error] is the system refusing the app the network (EPERM or
     * EACCES), not the network failing: it reads like a refused or timed-out
     * connection otherwise.
     */
    fun isPermissionBlock(error: Throwable): Boolean {
        var cause: Throwable? = error
        var depth = 0
        while (cause != null && depth < 8) {
            if (cause is android.system.ErrnoException && (cause.errno == EPERM || cause.errno == EACCES)) return true
            val message = cause.message.orEmpty()
            if ("EPERM" in message || "EACCES" in message || "Operation not permitted" in message) return true
            cause = cause.cause
            depth++
        }
        return false
    }

    // Linux's numbers, which Android's are.
    private const val EPERM = 1
    private const val EACCES = 13
}
