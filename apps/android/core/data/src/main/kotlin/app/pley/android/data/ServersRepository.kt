package app.pley.android.data

import app.pley.android.common.SavedServer
import app.pley.android.model.Me
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

/**
 * The servers this device knows, and the one it uses. The web view app's
 * saved servers move here once, keeping its device id for the player.
 */
interface ServersRepository {
    /** Saved servers, most recently used first. */
    val saved: Flow<List<SavedServer>>

    /** The server in use (its origin); `null` before one is chosen. */
    val active: StateFlow<String?>

    suspend fun use(server: SavedServer)

    /** Forgets [origin]: signs out there (ending the sign-in on the server) and deletes what was saved from it. */
    suspend fun forget(origin: String)
}

/** Who is signed in where. */
data class Session(
    /** The server (its origin, the tokens' issuer). */
    val issuer: String,
    /** The profile, from the snapshot first; `null` until known. */
    val me: Me?,
    /** What the sign-in allows ("library play playlists profiles"). */
    val scopes: Set<String>,
)

/**
 * The signed-in session and its profile. Everything saved is kept per
 * server and profile, so switching profiles shows the other profile's
 * things, never a mix. Whether the profile is an administrator is never
 * read: the app shows the same to everyone.
 */
interface SessionRepository {
    /** The session; `null` when signed out. */
    val session: StateFlow<Session?>

    /** Signs out of the active server, ending the sign-in there. */
    suspend fun signOut()
}

/**
 * The last answer for each screen, on disk per server and profile, so the
 * app shows what it showed before at once, even offline. Read off the main
 * thread; written whole, so a crash never leaves half an answer.
 */
interface SnapshotStore {
    suspend fun read(key: String): ByteArray?

    suspend fun write(key: String, bytes: ByteArray)

    /** Deletes everything saved for [issuer]. */
    suspend fun clear(issuer: String)
}
