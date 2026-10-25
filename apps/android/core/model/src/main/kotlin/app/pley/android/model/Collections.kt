package app.pley.android.model

import kotlinx.serialization.Serializable

/** The order of a collection's items. */
@Serializable(with = CollectionSort.Serializer::class)
enum class CollectionSort(override val wire: String) : WireEnum {
    Release("release"),
    Title("title"),

    /** The order an administrator gave. */
    Custom("custom"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<CollectionSort>("CollectionSort", CollectionSort.entries, CollectionSort.Unknown)
}

/**
 * A collection: movies or shows of one library, shared by every profile
 * (apps/server/src/collections.rs, `CollectionDto`). Times are unix seconds.
 */
@Serializable
data class CollectionSummary(
    val id: Long,
    val libraryId: Long,
    val title: String,
    val summary: String? = null,
    /** Mirrors a TMDB movie collection. */
    val automatic: Boolean = false,
    val sort: CollectionSort = CollectionSort.Unknown,
    /** Smart collections: the library filter that fills them (the Plex-compatible API's syntax). */
    val smartFilter: String? = null,
    /** Items this profile can see. */
    val itemCount: Int = 0,
    /** Its own artwork, else its first item's that has some. */
    val posterUrl: String? = null,
    val backdropUrl: String? = null,
    val createdAt: Long = 0,
    val updatedAt: Long = 0,
)

/** GET /api/collections/{id}: a collection and its items, in its order. */
@Serializable
data class CollectionDetail(
    val id: Long,
    val libraryId: Long,
    val title: String,
    val summary: String? = null,
    val automatic: Boolean = false,
    val sort: CollectionSort = CollectionSort.Unknown,
    val smartFilter: String? = null,
    val itemCount: Int = 0,
    val posterUrl: String? = null,
    val backdropUrl: String? = null,
    val createdAt: Long = 0,
    val updatedAt: Long = 0,
    val items: List<ItemSummary> = emptyList(),
) {
    fun toSummary(): CollectionSummary = CollectionSummary(
        id = id,
        libraryId = libraryId,
        title = title,
        summary = summary,
        automatic = automatic,
        sort = sort,
        smartFilter = smartFilter,
        itemCount = itemCount,
        posterUrl = posterUrl,
        backdropUrl = backdropUrl,
        createdAt = createdAt,
        updatedAt = updatedAt,
    )
}

/** `{ collections }`: GET /api/libraries/{id}/collections, by title. */
@Serializable
data class CollectionList(
    val collections: List<CollectionSummary> = emptyList(),
)

/** A collection named only: what an item is "Part of". */
@Serializable
data class CollectionRef(
    val id: Long,
    val title: String,
)

/** `{ collections }`: GET /api/items/{id}/collections, those the movie or show is in. */
@Serializable
data class ItemCollections(
    val collections: List<CollectionRef> = emptyList(),
)
