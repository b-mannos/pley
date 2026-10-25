package app.pley.android.model

import kotlinx.serialization.Serializable

/**
 * The body of a refusal: `{ "code": "not_found", "message": "item 42 not
 * found" }` (apps/server/src/error.rs). [code] is one of `bad_request`,
 * `unauthorized`, `forbidden`, `not_found`, `conflict`,
 * `too_many_requests`, `two_step`, `insufficient_scope`, `unavailable` or
 * `internal`; [message] is written for people.
 */
@Serializable
data class ApiErrorBody(
    val code: String? = null,
    val message: String? = null,
)
