package app.pley.android.model

import kotlinx.serialization.Serializable

/**
 * A row of the home page or of a library's front page
 * (apps/server/src/api/libraries.rs, `Hub`). Empty rows are left out.
 */
@Serializable
data class Hub(
    /**
     * "continue_watching" on the home page; "recently_added:<library id>"
     * there and on a library's page, with "continue_watching:<id>",
     * "recently_released:<id>" and "top_rated:<id>".
     */
    val key: String,
    val title: String,
    /** `null` for the home page's Continue Watching. */
    val libraryId: Long? = null,
    val items: List<ItemSummary> = emptyList(),
)

/**
 * `{ hubs }`: GET /api/hubs/home (Continue Watching, then Recently Added of
 * each library in the libraries' order) and GET /api/libraries/{id}/hubs.
 */
@Serializable
data class Hubs(
    val hubs: List<Hub> = emptyList(),
)
