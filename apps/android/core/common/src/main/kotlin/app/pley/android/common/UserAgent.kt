package app.pley.android.common

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import java.text.Normalizer

/**
 * How the app's requests introduce themselves: `pleyAndroid/0.1.0
 * (Android 17; Living room TV)`. The server names the device after it in
 * Account → Devices and Apps, so it starts with pleyAndroid/ whatever else
 * changes, and says which device it is: two TVs on the same Android tell
 * apart.
 */
object UserAgent {
    /** Long enough for "Galaxy Note20 Ultra 5G" and most names people give. */
    private const val MAX_NAME = 40

    fun of(versionName: String, release: String? = Build.VERSION.RELEASE, device: String? = null): String {
        val name = device?.let(::cleaned)?.takeIf { it.isNotEmpty() }
        return "pleyAndroid/$versionName (Android ${release ?: "?"}${name?.let { "; $it" }.orEmpty()})"
    }

    /** The user agent of the installed app, by its version name. */
    fun of(context: Context): String {
        val version = try {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName
        } catch (_: PackageManager.NameNotFoundException) {
            null
        }
        return of(version ?: "?", device = deviceName(context))
    }

    /**
     * What this device is called: its name in Android's settings ("Living
     * room TV", "Galaxy Note20 Ultra 5G"), else its model.
     */
    fun deviceName(context: Context): String? {
        val named = try {
            Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)
        } catch (_: Exception) {
            null
        }
        // A name of letters a header can't carry (Cyrillic, CJK…) gives the model instead.
        return named?.takeIf { cleaned(it).isNotEmpty() } ?: Build.MODEL
    }

    /**
     * [name] as a header carries it: printable ASCII (typographic quotes
     * and dashes made plain, the rest left out), without the brackets and
     * semicolons the user agent is made of.
     */
    internal fun cleaned(name: String): String = Normalizer.normalize(name, Normalizer.Form.NFD)
        // "Télé" is "Tele": accents go, their letters stay.
        .filterNot { Character.getType(it) == Character.NON_SPACING_MARK.toInt() }
        .map {
            when (it) {
                '‘', '’', '‛', '′' -> '\''
                '“', '”', '″' -> '"'
                '–', '—', '‐', '‑' -> '-'
                in ' '..'~' -> if (it in "();\\") ' ' else it
                else -> ' '
            }
        }
        .joinToString("")
        .split(' ')
        .filter { it.isNotEmpty() }
        .joinToString(" ")
        .take(MAX_NAME)
        .trim()
}
