package app.pley.android.model

import kotlinx.serialization.Serializable

/** What a playlist plays: video (movies and episodes) or audio (tracks). */
@Serializable(with = PlaylistKind.Serializer::class)
enum class PlaylistKind(override val wire: String) : WireEnum {
    Video("video"),
    Audio("audio"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<PlaylistKind>("PlaylistKind", PlaylistKind.entries, PlaylistKind.Unknown)
}

/**
 * A playlist of the profile's own, private to it
 * (apps/server/src/playlists.rs, `PlaylistDto`). Times are unix seconds.
 */
@Serializable
data class PlaylistSummary(
    val id: Long,
    val title: String,
    val kind: PlaylistKind,
    /** Entries this profile can see. */
    val itemCount: Int = 0,
    val durationMs: Long = 0,
    /** Picture addresses of the first entries (at most 4), for a mosaic. */
    val artwork: List<String> = emptyList(),
    /** Smart playlists: the listing of [libraryId] they play. */
    val smartFilter: String? = null,
    val libraryId: Long? = null,
    val createdAt: Long = 0,
    val updatedAt: Long = 0,
)

/** An entry of a playlist: the same item can be in one more than once. */
@Serializable
data class PlaylistEntry(
    /** This entry; a smart playlist's are its items' ids. */
    val entryId: Long,
    val item: ItemSummary,
)

/** A playlist and its entries: GET, PATCH /api/playlists/{id}, POST /api/playlists and PUT …/order. */
@Serializable
data class PlaylistDetail(
    val id: Long,
    val title: String,
    val kind: PlaylistKind,
    val itemCount: Int = 0,
    val durationMs: Long = 0,
    val artwork: List<String> = emptyList(),
    val smartFilter: String? = null,
    val libraryId: Long? = null,
    val createdAt: Long = 0,
    val updatedAt: Long = 0,
    val entries: List<PlaylistEntry> = emptyList(),
) {
    fun toSummary(): PlaylistSummary = PlaylistSummary(
        id = id,
        title = title,
        kind = kind,
        itemCount = itemCount,
        durationMs = durationMs,
        artwork = artwork,
        smartFilter = smartFilter,
        libraryId = libraryId,
        createdAt = createdAt,
        updatedAt = updatedAt,
    )
}

/** `{ playlists }`: GET /api/playlists, by title. */
@Serializable
data class PlaylistList(
    val playlists: List<PlaylistSummary> = emptyList(),
)

/**
 * The body of POST /api/playlists: [title] with [itemIds] (and [kind],
 * else the first item the profile can see decides), or a smart playlist:
 * [title], [libraryId] and [smartFilter].
 */
@Serializable
data class NewPlaylist(
    val title: String,
    val kind: PlaylistKind? = null,
    val itemIds: List<Long> = emptyList(),
    val smartFilter: String? = null,
    val libraryId: Long? = null,
)

/** The body of PATCH /api/playlists/{id}: what changes; a field left `null` stays as it is. */
@Serializable
data class PlaylistPatch(
    val title: String? = null,
    /** Smart playlists only. */
    val smartFilter: String? = null,
)

/** `{ item_ids }`: the body of POST /api/playlists/{id}/items. Shows, seasons, albums and artists add their episodes or tracks. */
@Serializable
data class ItemIds(
    val itemIds: List<Long>,
)

/** `{ entry_ids }`: the body of PUT /api/playlists/{id}/order: these first, in this order, the rest after. */
@Serializable
data class EntryIds(
    val entryIds: List<Long>,
)

/** `{ added }`: the answer of POST /api/playlists/{id}/items, how many entries were added. */
@Serializable
data class Added(
    val added: Int = 0,
)
