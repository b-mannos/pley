package app.pley.android.fixtures

/**
 * What apps wrote to the FixtureServer, in order: every POST, PUT, PATCH and
 * DELETE under /api but the sign-in's (whose forms hold codes and tokens).
 * Progress reports, watched marks, ratings, track choices, playlists and
 * track defaults are all here, for tests to read.
 */
class Journal internal constructor() {
    /** One write: [body] as it came (JSON, or empty), by the profile [profileId] acted as. */
    data class Entry(
        val method: String,
        val path: String,
        val query: String?,
        val profileId: Long?,
        val body: String,
        /** When it came, in ms on the server's clock. */
        val atMs: Long,
    )

    private val entries = mutableListOf<Entry>()

    /** Every write so far. */
    fun entries(): List<Entry> = synchronized(entries) { entries.toList() }

    /** The writes of [method] to [path] ("/api/items/1/progress"). */
    fun writes(method: String, path: String): List<Entry> = entries().filter { it.method == method && it.path == path }

    fun clear() = synchronized(entries) { entries.clear() }

    internal fun add(entry: Entry) = synchronized(entries) { entries += entry }
}
