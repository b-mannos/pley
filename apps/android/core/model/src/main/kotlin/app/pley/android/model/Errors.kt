package app.pley.android.model

import kotlinx.serialization.Serializable

/**
 * The body of a refusal: `{ "code": "not_found", "message": "item 42 not
 * found" }` (apps/server/src/error.rs). [code] is one of `bad_request`,
 * `unauthorized`, `forbidden`, `not_found`, `conflict`,
 * `too_many_requests`, `two_step`, `insufficient_scope`, `unavailable` or
 * `internal`; [message] is written for people. A `409` from watching a
 * live channel also lists the [connections] in use.
 */
@Serializable
data class ApiErrorBody(
    val code: String? = null,
    val message: String? = null,
    val connections: List<BusyConnection> = emptyList(),
)

/**
 * A connection of a live source whose connections are all in use (a
 * `409`'s `connections`): the channel it watches, who watches it
 * ([watchers]: named to administrators only), how many ([people]),
 * whether only you do, on another device ([yours]), and whether you may
 * take it over ([canTakeOver]: it's yours, or you're an administrator and
 * nobody watching it is).
 */
@Serializable
data class BusyConnection(
    val channelId: Long,
    val channelName: String = "",
    val watchers: List<String> = emptyList(),
    val people: Int = 0,
    val yours: Boolean = false,
    val canTakeOver: Boolean = false,
)
