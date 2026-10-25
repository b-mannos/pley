package app.pley.android

import app.pley.android.common.Addresses
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

    /** Another site (the browser opens it), or a link for another app. */
    Browser,

    /** Nothing to do with it. */
    Ignore,
}

/** Where the web view is: [server]'s pages. */
class Places(val server: String)

object Links {
    private fun parse(url: String): URI? = try {
        URI(url)
    } catch (_: URISyntaxException) {
        null
    }

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

    /** The origin of [url] as browsers write it (`https://pley.example.com`, `http://nas:7539`). */
    fun address(url: String): String? = Addresses.origin(url)

    fun navigation(url: String, places: Places): Navigation {
        val target = parse(url) ?: return Navigation.Ignore
        val home = parse(places.server) ?: return Navigation.Ignore
        val scheme = target.scheme?.lowercase()
        val path = target.rawPath ?: ""
        val at = origin(target)
        if (at != null && at == origin(home)) {
            if (path.startsWith(SWITCH_SERVER_PATH)) return Navigation.Servers
            return Navigation.Allow
        }
        return when (scheme) {
            // otpauth: adds pley to an authenticator app on this phone (two-step sign-in).
            "http", "https", "mailto", "otpauth" -> Navigation.Browser
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
