package app.pley.android.watchnext

import app.pley.android.model.Hubs
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary

/** Whose row it is: the server and the profile signed in there. */
data class WatchNextOwner(val issuer: String, val profileId: Long) {
    /** The start of every program's internal id: programs of another owner are someone else's. */
    val key: String get() = "$issuer#$profileId"
}

/** Why a title is in the row: watched part way, or the next episode of a show being watched. */
enum class WatchNextKind { Continue, Next }

/** One program of the Watch Next row, as pley publishes it. */
data class WatchNextEntry(
    val owner: WatchNextOwner,
    val itemId: Long,
    val kind: WatchNextKind,
    val episode: Boolean,
    /** A movie's title; an episode's show. */
    val title: String,
    val episodeTitle: String?,
    val season: Int?,
    val number: Int?,
    val durationMs: Long?,
    val positionMs: Long,
    /** The 16:9 picture's address on the server (an episode's still, else the backdrop); `null` for none. */
    val imagePath: String?,
    /** When the profile last watched it (Unix ms), for the row's order. */
    val engagedAtMs: Long,
) {
    /** The program's internal provider id: the owner, then the item. */
    val internalId: String get() = "${owner.key}|$itemId"
}

/**
 * What the Watch Next row shows for [owner]: home's Continue Watching, in
 * its order (the profile's own: the server's answer), the titles watched
 * part way as Continue and episodes not started (the next of their show) as
 * Next. At most [MAX] of them; [now] (Unix ms) dates those the server
 * doesn't date, a moment apart so the order holds.
 */
object WatchNextPlan {
    const val MAX = 10

    fun of(hubs: Hubs, owner: WatchNextOwner, now: Long): List<WatchNextEntry> {
        val row = hubs.hubs.firstOrNull { it.key == app.pley.android.model.HubKeys.CONTINUE_WATCHING } ?: return emptyList()
        return row.items
            .filter { it.kind == ItemKind.Movie || it.kind == ItemKind.Episode }
            .distinctBy { it.id }
            .take(MAX)
            .mapIndexed { i, item -> entry(item, owner, now - i * ORDER_STEP_MS) }
    }

    private fun entry(item: ItemSummary, owner: WatchNextOwner, fallbackAt: Long): WatchNextEntry {
        val episode = item.kind == ItemKind.Episode
        val position = item.userState.positionMs.coerceAtLeast(0)
        return WatchNextEntry(
            owner = owner,
            itemId = item.id,
            kind = if (position > 0) WatchNextKind.Continue else WatchNextKind.Next,
            episode = episode,
            title = if (episode) item.grandparentTitle ?: item.title else item.title,
            episodeTitle = if (episode) item.title else null,
            season = if (episode) item.parentIndex else null,
            number = if (episode) item.index else null,
            durationMs = item.durationMs?.takeIf { it > 0 },
            positionMs = position,
            imagePath = (if (episode) item.thumbUrl ?: item.backdropUrl else item.backdropUrl ?: item.thumbUrl),
            engagedAtMs = item.userState.lastPlayedAt?.let { it * 1_000 } ?: fallbackAt,
        )
    }

    private const val ORDER_STEP_MS = 1_000L
}

/** A program in the row now, as the system has it. */
data class PublishedProgram(
    val id: Long,
    val internalId: String?,
    /** False once the person took it off the row on the home screen: it isn't put back. */
    val browsable: Boolean,
)

/** What a sync does to the row. */
data class WatchNextChanges(
    val insert: List<WatchNextEntry>,
    /** The program's id and what it shows now. */
    val update: List<Pair<Long, WatchNextEntry>>,
    val delete: List<Long>,
)

/** How the row's programs become [wanted]'s. */
object WatchNextDiff {
    /**
     * Programs pley published and doesn't want any more (another owner's,
     * or titles that left Continue Watching) go; those wanted are added or
     * updated, but never one the person took off the row: the system keeps
     * it hidden, and pley leaves it so until it leaves the row's list.
     */
    fun of(published: List<PublishedProgram>, wanted: List<WatchNextEntry>): WatchNextChanges {
        val byId = wanted.associateBy { it.internalId }
        val kept = HashSet<String>()
        val update = mutableListOf<Pair<Long, WatchNextEntry>>()
        val delete = mutableListOf<Long>()
        for (p in published) {
            val entry = p.internalId?.let(byId::get)
            when {
                entry == null || !kept.add(entry.internalId) -> delete += p.id
                p.browsable -> update += p.id to entry
            }
        }
        return WatchNextChanges(insert = wanted.filter { it.internalId !in kept }, update = update, delete = delete)
    }
}
