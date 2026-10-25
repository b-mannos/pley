package app.pley.android.model

import kotlinx.serialization.Serializable

/** Who played an entry of the history (apps/server/src/history.rs, `HistoryUser`). */
@Serializable
data class HistoryUser(
    val id: Long,
    /** The display name, or the username. */
    val name: String,
    val avatarUrl: String? = null,
    val avatarColor: String? = null,
)

/**
 * A play: recorded once it reached the point where the item counts as
 * watched (90 %), with the device it played on. Marking something watched
 * by hand is not a play.
 */
@Serializable
data class HistoryEntry(
    val id: Long,
    /** Unix seconds. */
    val viewedAt: Long,
    /** "Chrome on Windows", a Plex client's name. */
    val device: String? = null,
    val user: HistoryUser,
    val item: ItemSummary,
)

/** A page of GET /api/history, newest first. [total] counts every match. */
@Serializable
data class HistoryPage(
    val total: Int = 0,
    val entries: List<HistoryEntry> = emptyList(),
)
