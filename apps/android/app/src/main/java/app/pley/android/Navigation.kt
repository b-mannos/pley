package app.pley.android

import java.net.URI
import java.net.URISyntaxException

/** Where the web app sends people who want another server. */
const val SWITCH_SERVER_PATH = "/__app/servers"

/** What the app does with a link the web view is asked to open. */
enum class Navigation {
    /** The server's own page: the web view loads it. */
    Allow,

    /** Back to the list of servers. */
    Servers,

    /** Another site: the browser opens it. */
    Browser,

    /** Nothing to do with it. */
    Ignore,
}

object Links {
    private fun origin(uri: URI): String? {
        val scheme = uri.scheme?.lowercase() ?: return null
        val host = uri.host?.lowercase() ?: return null
        val port = when {
            uri.port != -1 -> uri.port
            scheme == "https" -> 443
            else -> 80
        }
        return "$scheme://$host:$port"
    }

    fun navigation(url: String, server: String): Navigation {
        val target = try {
            URI(url)
        } catch (_: URISyntaxException) {
            return Navigation.Ignore
        }
        val home = try {
            URI(server)
        } catch (_: URISyntaxException) {
            return Navigation.Ignore
        }
        val sameServer = origin(target) != null && origin(target) == origin(home)
        return when {
            sameServer && (target.path ?: "").startsWith(SWITCH_SERVER_PATH) -> Navigation.Servers
            sameServer -> Navigation.Allow
            target.scheme?.lowercase() in setOf("http", "https", "mailto") -> Navigation.Browser
            else -> Navigation.Ignore
        }
    }

    /** The session token in the cookies the web view keeps for the server (`pley_token`). */
    fun token(cookies: String?): String? = cookies
        ?.split(';')
        ?.map { it.trim() }
        ?.firstOrNull { it.startsWith("pley_token=") }
        ?.removePrefix("pley_token=")
        ?.takeIf { it.isNotEmpty() }
}
