package app.pley.android.common

import android.content.Context
import android.content.pm.PackageManager

/**
 * Which build of the app this is: its version name (pley's version, VERSION
 * at the repository's root) and the commit it was made from, which the app
 * sets from its BuildConfig when it starts.
 */
object AppBuild {
    /** The commit's short hash, empty until the app says or when it isn't known. */
    @Volatile
    var commit: String = ""

    /** "0.2.0 (1a2b3c4d5)", the version alone without a commit, or "" when the system won't say. */
    fun label(context: Context): String {
        val version = try {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName.orEmpty()
        } catch (_: PackageManager.NameNotFoundException) {
            ""
        }
        return label(version, commit)
    }

    fun label(version: String, commit: String): String = when {
        version.isEmpty() -> ""
        commit.isEmpty() || commit == "unknown" -> version
        else -> "$version ($commit)"
    }
}
