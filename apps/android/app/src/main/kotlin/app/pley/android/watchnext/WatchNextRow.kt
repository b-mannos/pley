package app.pley.android.watchnext

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.tvprovider.media.tv.TvContractCompat
import androidx.tvprovider.media.tv.WatchNextProgram
import app.pley.android.TvActivity

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
            .setIntentUri(Uri.parse(WatchNextLink.intent(app, entry).toUri(Intent.URI_INTENT_SCHEME)))
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
 */
object WatchNextLink {
    const val ACTION = "app.pley.android.action.WATCH_NEXT"
    private const val EXTRA_ISSUER = "watch_next_issuer"
    private const val EXTRA_PROFILE = "watch_next_profile"
    private const val EXTRA_ITEM = "watch_next_item"

    fun intent(context: Context, entry: WatchNextEntry): Intent = Intent(context, TvActivity::class.java)
        .setAction(ACTION)
        // The app open already (a video playing, even): back to its TvActivity, which plays this.
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        .putExtra(EXTRA_ISSUER, entry.owner.issuer)
        .putExtra(EXTRA_PROFILE, entry.owner.profileId)
        .putExtra(EXTRA_ITEM, entry.itemId)

    /** The owner and the item of a program's [intent]; `null` for any other intent. */
    fun read(intent: Intent?): Pair<WatchNextOwner, Long>? {
        if (intent?.action != ACTION) return null
        val issuer = intent.getStringExtra(EXTRA_ISSUER) ?: return null
        val profile = intent.getLongExtra(EXTRA_PROFILE, -1).takeIf { it >= 0 } ?: return null
        val item = intent.getLongExtra(EXTRA_ITEM, -1).takeIf { it >= 0 } ?: return null
        return WatchNextOwner(issuer, profile) to item
    }
}
