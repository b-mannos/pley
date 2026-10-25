package app.pley.android.network

import app.pley.android.common.UserFacing

/**
 * An answer the server refused: its status, with the `code` and `message`
 * of its `{ code, message }` JSON (apps/server/src/error.rs), or axum's
 * own text for a request it couldn't read (then [code] is `null`). The
 * message is the server's sentence, written for people.
 *
 * Refusals of an app's token also say why in `WWW-Authenticate`
 * (RFC 6750): [bearerError] is its `error` ("invalid_token",
 * "invalid_request", "insufficient_scope"). A 403 `insufficient_scope`
 * keeps the [scope] that would open the route, `null` when none does.
 */
class ApiException(
    val status: Int,
    val code: String?,
    message: String,
    val bearerError: String? = null,
    val scope: String? = null,
) : java.io.IOException(message), UserFacing {
    /** The token doesn't work any more: expired, signed out, or its account disabled. */
    val signedOut: Boolean get() = status == 401 && (bearerError == "invalid_token" || code == "unauthorized")

    /** The app wasn't allowed this: signing in again can ask for [scope]. */
    val needsScope: Boolean get() = status == 403 && (code == "insufficient_scope" || bearerError == "insufficient_scope")
}
