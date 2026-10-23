package app.pley.android

import java.net.URI
import java.net.URISyntaxException

/** pley's web port, for addresses typed without one. */
const val DEFAULT_PORT = 7539

/** A server opened before. */
data class SavedServer(val url: String, val name: String, val id: String?, val lastUsed: Long)

/** A server that answered on the local network. */
data class FoundServer(val url: String, val name: String, val id: String?, val version: String)

/** One row of the server list: saved, found on the network, or both. */
data class ServerEntry(val url: String, val name: String, val id: String?, val saved: Boolean, val found: Boolean)

/** The rules the desktop app's picker follows too (apps/desktop/src/servers.ts). */
object Servers {
    /**
     * `nas`, `nas:8080`, `192.168.1.20` or `https://pley.example.com/` as a
     * server's address (its origin); `null` when it can't be one. Without a
     * scheme it's http, and without a port pley's.
     */
    fun normalizeUrl(input: String): String? {
        val text = input.trim()
        if (text.isEmpty() || text.any { it.isWhitespace() }) return null
        val hasScheme = Regex("^[a-zA-Z][a-zA-Z0-9+.-]*://").containsMatchIn(text)
        val uri = try {
            URI(if (hasScheme) text else "http://$text")
        } catch (_: URISyntaxException) {
            return null
        }
        val scheme = uri.scheme?.lowercase() ?: return null
        val host = uri.host?.lowercase() ?: return null
        if (scheme != "http" && scheme != "https") return null
        val port = when {
            uri.port != -1 -> uri.port
            !hasScheme -> DEFAULT_PORT
            else -> -1
        }
        val defaultPort = (scheme == "http" && port == 80) || (scheme == "https" && port == 443)
        return if (port == -1 || defaultPort) "$scheme://$host" else "$scheme://$host:$port"
    }

    /** The address to show for a server: its host and, unless it's pley's, its port. */
    fun displayAddress(url: String): String {
        val uri = try {
            URI(url)
        } catch (_: URISyntaxException) {
            return url
        }
        val host = uri.host ?: return url
        return if (uri.port == -1 || (uri.scheme == "http" && uri.port == DEFAULT_PORT)) host else "$host:${uri.port}"
    }

    private fun same(aUrl: String, aId: String?, bUrl: String, bId: String?): Boolean =
        (aId != null && aId == bId) || aUrl == bUrl

    /** Saved servers, most recently opened first, then the ones found on the network that aren't saved. */
    fun merge(saved: List<SavedServer>, found: List<FoundServer>): List<ServerEntry> {
        val entries = saved.sortedByDescending { it.lastUsed }.map { s ->
            ServerEntry(s.url, s.name, s.id, saved = true, found = found.any { same(s.url, s.id, it.url, it.id) })
        }.toMutableList()
        for (f in found) {
            if (entries.none { same(it.url, it.id, f.url, f.id) }) {
                entries.add(ServerEntry(f.url, f.name, f.id, saved = false, found = true))
            }
        }
        return entries
    }

    /** The server to open by itself at start: the one opened last. */
    fun lastUsed(saved: List<SavedServer>): SavedServer? = saved.maxByOrNull { it.lastUsed }

    /** `servers` with `server` in it, replacing one with the same address or id (a server found at a new address). */
    fun withServer(servers: List<SavedServer>, server: SavedServer): List<SavedServer> =
        servers.filter { it.url != server.url && (server.id == null || it.id != server.id) } + server
}
