package app.pley.android.model

import kotlinx.serialization.Serializable

/**
 * GET /api/server, which answers without signing in: the server's name and
 * version, and whether it still has to be set up in the browser
 * (apps/server/src/api/auth.rs, `server_status`).
 */
@Serializable
data class ServerInfo(
    val name: String,
    val version: String,
    val setupRequired: Boolean = false,
    /** Setting up takes the code from the server's log. */
    val setupCodeRequired: Boolean = false,
)

/** GET /api/health: `{ "status": "ok" }` while the server runs. */
@Serializable
data class Health(
    val status: String,
)
