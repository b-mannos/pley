package app.pley.android

import java.net.URI
import java.net.URISyntaxException

/** Where the web app sends people who want another server. */
const val SWITCH_SERVER_PATH = "/__app/servers"

/** Where a pley server's single sign-on pages are (`/api/auth/sso/start`…). */
private const val SIGN_IN = "/api/auth/sso/"

/** What the app does with a link the web view is asked to open. */
enum class Navigation {
    /** The server's own page (or its sign-in provider's, during a sign-in): the web view loads it. */
    Allow,

    /** Back to the list of servers. */
    Servers,

    /** Another site (the browser opens it), or a link for another app. */
    Browser,

    /** Nothing to do with it. */
    Ignore,
}

/**
 * Where the web view is: [server]'s pages, or, while [signingIn] is set, a
 * sign-in going through the server's single sign-on provider, whose pages
 * show in the web view until it is back at that pley address (the server's
 * public one, maybe: it then becomes [server]).
 */
class Places(var server: String, var signingIn: String? = null)

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
    fun address(url: String): String? {
        val uri = parse(url) ?: return null
        val scheme = uri.scheme?.lowercase() ?: return null
        val host = uri.host?.lowercase() ?: return null
        val defaultPort = uri.port == -1 || (scheme == "http" && uri.port == 80) || (scheme == "https" && uri.port == 443)
        return if (defaultPort) "$scheme://$host" else "$scheme://$host:${uri.port}"
    }

    fun navigation(url: String, places: Places): Navigation {
        val target = parse(url) ?: return Navigation.Ignore
        val home = parse(places.server) ?: return Navigation.Ignore
        val scheme = target.scheme?.lowercase()
        val web = scheme == "http" || scheme == "https"
        val path = target.rawPath ?: ""
        val signIn = web && path.startsWith(SIGN_IN)
        val at = origin(target)
        if (at != null && at == origin(home)) {
            if (path.startsWith(SWITCH_SERVER_PATH)) return Navigation.Servers
            places.signingIn = if (signIn) places.server else null
            return Navigation.Allow
        }
        // The server sends sign-ins to its public address first.
        if (signIn && path.startsWith("${SIGN_IN}start")) {
            places.signingIn = url
            return Navigation.Allow
        }
        val back = places.signingIn?.let(::parse)
        if (back != null) {
            if (at != null && at == origin(back) && !signIn) {
                // Signed in there: that address is the server's now.
                places.server = address(url) ?: places.server
                places.signingIn = null
                return Navigation.Allow
            }
            // The provider's pages (and the ones it sends people to).
            if (web) return Navigation.Allow
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
