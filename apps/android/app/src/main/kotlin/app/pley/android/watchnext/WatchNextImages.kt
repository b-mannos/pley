package app.pley.android.watchnext

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import java.security.MessageDigest
import java.security.SecureRandom
import okhttp3.OkHttpClient
import okhttp3.Request

/**
 * The Watch Next row's pictures. The home screen can't fetch them from the
 * server (pley's pictures need the profile's token, which never leaves the
 * app), so they are fetched here, kept in the app's files and handed to the
 * home screen by [WatchNextImageProvider]. Each is named by a keyed hash of
 * its address, with a secret of this installation: no other app can name
 * one it wasn't given. Blocking: off the main thread.
 */
class WatchNextImages(context: Context, private val client: () -> OkHttpClient) {
    private val app = context.applicationContext
    // Read on first use, off the main thread: filesDir touches the disk.
    private val dir by lazy { File(app.filesDir, DIR) }

    /** The installation's secret, made once. */
    private val secret: ByteArray by lazy {
        val prefs = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        prefs.getString(SECRET, null)?.let(::fromHex) ?: ByteArray(32).also {
            SecureRandom().nextBytes(it)
            prefs.edit().putString(SECRET, hex(it)).apply()
        }
    }

    /** The picture at [url] (absolute, on the server) for the home screen: fetched once, then kept. `null` when it can't be had. */
    fun uriFor(url: String): Uri? {
        val name = nameOf(url)
        val file = File(dir, name)
        if (!file.exists()) {
            val part = File(dir, "$name.part")
            try {
                dir.mkdirs()
                client().newCall(Request.Builder().url(url).build()).execute().use { response ->
                    if (!response.isSuccessful) return null
                    part.outputStream().use { out -> response.body.byteStream().copyTo(out) }
                }
                if (!part.renameTo(file)) return null
            } catch (_: IOException) {
                part.delete()
                return null
            } catch (_: IllegalArgumentException) {
                // Not an address OkHttp takes.
                return null
            }
        }
        return Uri.Builder().scheme("content").authority(authority(app)).appendPath(name).build()
    }

    /** Deletes the pictures of programs gone: those not among [urls]. */
    fun keepOnly(urls: Collection<String>) {
        val kept = urls.mapTo(HashSet()) { nameOf(it) }
        dir.listFiles()?.forEach { if (it.name !in kept) it.delete() }
    }

    private fun nameOf(url: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update(secret)
        digest.update(url.toByteArray())
        return hex(digest.digest())
    }

    companion object {
        const val DIR = "watch-next"
        private const val PREFS = "watch_next"
        private const val SECRET = "image_secret"

        fun authority(context: Context): String = "${context.packageName}.watchnext"

        private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it) }

        private fun fromHex(text: String): ByteArray? =
            if (text.length % 2 != 0) null else runCatching { ByteArray(text.length / 2) { text.substring(it * 2, it * 2 + 2).toInt(16).toByte() } }.getOrNull()
    }
}

/**
 * Hands the Watch Next row's pictures to the home screen, read only: a
 * name it was given, nothing else (no listing, no paths). Exported, as the
 * home screen is another app; the names can't be guessed ([WatchNextImages]).
 */
class WatchNextImageProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") throw FileNotFoundException("read only")
        val name = uri.pathSegments.singleOrNull()?.takeIf { NAME.matches(it) } ?: throw FileNotFoundException("no such picture")
        val file = File(File(context!!.filesDir, WatchNextImages.DIR), name)
        if (!file.isFile) throw FileNotFoundException("no such picture")
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri): String = "image/*"

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0

    private companion object {
        val NAME = Regex("[0-9a-f]{64}")
    }
}
