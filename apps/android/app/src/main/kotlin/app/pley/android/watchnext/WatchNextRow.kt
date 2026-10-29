package app.pley.android.watchnext

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.tvprovider.media.tv.TvContractCompat
import androidx.tvprovider.media.tv.WatchNextProgram
import app.pley.android.TvActivity
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/** The system's Watch Next row, as far as pley's own programs go. */
interface WatchNextRow {
    /** pley's programs in the row now. */
    fun published(): List<PublishedProgram>

    /** Adds [entry] with its picture at [image]. */
    fun insert(entry: WatchNextEntry, image: Uri?)

    /** Program [id] shows [entry] now. */
    fun update(id: Long, entry: WatchNextEntry, image: Uri?)

    fun delete(id: Long)
}

/**
 * The row in the TV provider (androidx.tvprovider): an app reads and
 * writes only its own programs there, without a permission. Selecting one
 * opens [TvActivity] with what it is ([WatchNextLink]).
 */
class TvProviderRow(context: Context) : WatchNextRow {
    private val app = context.applicationContext
    private val resolver get() = app.contentResolver
    // Read on first use, off the main thread (programs are written there).
    private val key by lazy { WatchNextLink.key(app) }

    @SuppressLint("RestrictedApi")
    override fun published(): List<PublishedProgram> {
        val found = mutableListOf<PublishedProgram>()
        resolver.query(TvContractCompat.WatchNextPrograms.CONTENT_URI, WatchNextProgram.PROJECTION, null, null, null)?.use { cursor ->
            while (cursor.moveToNext()) {
                val program = WatchNextProgram.fromCursor(cursor)
                found += PublishedProgram(program.id, program.internalProviderId, program.isBrowsable)
            }
        }
        return found
    }

    override fun insert(entry: WatchNextEntry, image: Uri?) {
        resolver.insert(TvContractCompat.WatchNextPrograms.CONTENT_URI, program(entry, image).toContentValues())
    }

    override fun update(id: Long, entry: WatchNextEntry, image: Uri?) {
        resolver.update(TvContractCompat.buildWatchNextProgramUri(id), program(entry, image).toContentValues(), null, null)
    }

    override fun delete(id: Long) {
        resolver.delete(TvContractCompat.buildWatchNextProgramUri(id), null, null)
    }

    // The setters are WatchNextProgram.Builder's public API; lint reads the generic base builders they
    // come from (BaseProgram.Builder) as the library's own, a known false positive of tvprovider.
    @SuppressLint("RestrictedApi")
    private fun program(entry: WatchNextEntry, image: Uri?): WatchNextProgram {
        val builder = WatchNextProgram.Builder()
            .setType(if (entry.episode) TvContractCompat.WatchNextPrograms.TYPE_TV_EPISODE else TvContractCompat.WatchNextPrograms.TYPE_MOVIE)
            .setWatchNextType(
                if (entry.kind == WatchNextKind.Continue) TvContractCompat.WatchNextPrograms.WATCH_NEXT_TYPE_CONTINUE else TvContractCompat.WatchNextPrograms.WATCH_NEXT_TYPE_NEXT,
            )
            .setLastEngagementTimeUtcMillis(entry.engagedAtMs)
            .setTitle(entry.title)
            .setInternalProviderId(entry.internalId)
            .setIntentUri(Uri.parse(WatchNextLink.intent(app, entry, key).toUri(Intent.URI_INTENT_SCHEME)))
        entry.episodeTitle?.let(builder::setEpisodeTitle)
        entry.season?.let { builder.setSeasonNumber(it) }
        entry.number?.let { builder.setEpisodeNumber(it) }
        entry.durationMs?.let { builder.setDurationMillis(it.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()) }
        if (entry.kind == WatchNextKind.Continue) builder.setLastPlaybackPositionMillis(entry.positionMs.coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
        if (image != null) {
            builder.setPosterArtUri(image).setPosterArtAspectRatio(TvContractCompat.PreviewPrograms.ASPECT_RATIO_16_9)
        }
        return builder.build()
    }
}

/**
 * What a Watch Next program opens: [TvActivity] with the server, the
 * profile and the item, which plays it from where it was when that
 * profile is the one signed in there, and otherwise just opens the app.
 *
 * [TvActivity] is exported (the home screen is another app), so any app
 * could send it this action: a link plays only with the signature this
 * installation gave it ([sign], keyed with a secret of its own), and one
 * without just opens the app.
 */
object WatchNextLink {
    const val ACTION = "app.pley.android.action.WATCH_NEXT"
    private const val EXTRA_ISSUER = "watch_next_issuer"
    private const val EXTRA_PROFILE = "watch_next_profile"
    private const val EXTRA_ITEM = "watch_next_item"
    private const val EXTRA_SIGNATURE = "watch_next_signature"
    private const val PREFS = "watch_next"
    private const val SECRET = "link_secret"

    /** A program's link, as an intent brought it: [verified] says whether this installation made it. */
    data class Link(val owner: WatchNextOwner, val itemId: Long, val signature: String?)

    fun intent(context: Context, entry: WatchNextEntry, key: ByteArray): Intent = Intent(context, TvActivity::class.java)
        .setAction(ACTION)
        // The app open already (a video playing, even): back to its TvActivity, which plays this.
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        .putExtra(EXTRA_ISSUER, entry.owner.issuer)
        .putExtra(EXTRA_PROFILE, entry.owner.profileId)
        .putExtra(EXTRA_ITEM, entry.itemId)
        .putExtra(EXTRA_SIGNATURE, sign(key, entry.owner, entry.itemId))

    /** The link a program's [intent] carries, not checked yet; `null` for any other intent. */
    fun read(intent: Intent?): Link? {
        if (intent?.action != ACTION) return null
        val issuer = intent.getStringExtra(EXTRA_ISSUER) ?: return null
        val profile = intent.getLongExtra(EXTRA_PROFILE, -1).takeIf { it >= 0 } ?: return null
        val item = intent.getLongExtra(EXTRA_ITEM, -1).takeIf { it >= 0 } ?: return null
        return Link(WatchNextOwner(issuer, profile), item, intent.getStringExtra(EXTRA_SIGNATURE))
    }

    /** The owner and the item of [link] when this installation signed it with [key]; `null` for any other. */
    fun verified(link: Link, key: ByteArray): Pair<WatchNextOwner, Long>? {
        val given = link.signature ?: return null
        val expected = sign(key, link.owner, link.itemId)
        // In time that doesn't tell how much of it was right.
        return (link.owner to link.itemId).takeIf { MessageDigest.isEqual(given.toByteArray(), expected.toByteArray()) }
    }

    /** HMAC-SHA256 of the owner and the item, in hex. */
    fun sign(key: ByteArray, owner: WatchNextOwner, itemId: Long): String {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(key, "HmacSHA256"))
        val signed = mac.doFinal("${owner.issuer}\n${owner.profileId}\n$itemId".toByteArray())
        return signed.joinToString("") { "%02x".format(it) }
    }

    /** The installation's key for links, made once (never backed up: backups are off). Blocking: off the main thread. */
    fun key(context: Context): ByteArray {
        val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        prefs.getString(SECRET, null)?.let(::fromHex)?.takeIf { it.size == 32 }?.let { return it }
        return ByteArray(32).also {
            SecureRandom().nextBytes(it)
            prefs.edit().putString(SECRET, it.joinToString("") { b -> "%02x".format(b) }).commit()
        }
    }

    private fun fromHex(text: String): ByteArray? =
        if (text.length % 2 != 0) null else runCatching { ByteArray(text.length / 2) { text.substring(it * 2, it * 2 + 2).toInt(16).toByte() } }.getOrNull()
}
