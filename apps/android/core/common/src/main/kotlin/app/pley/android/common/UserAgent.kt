package app.pley.android.common

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build

/**
 * How the app's requests introduce themselves: `pleyAndroid/0.1.0
 * (Android 17)`. The server names the device after it in Account →
 * Devices, so it starts with pleyAndroid/ whatever else changes.
 */
object UserAgent {
    fun of(versionName: String, release: String? = Build.VERSION.RELEASE): String =
        "pleyAndroid/$versionName (Android ${release ?: "?"})"

    /** The user agent of the installed app, by its version name. */
    fun of(context: Context): String {
        val version = try {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        } catch (_: PackageManager.NameNotFoundException) {
            null
        }
        return of(version ?: "?")
    }
}
