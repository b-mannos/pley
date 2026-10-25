package app.pley.android.common

import java.net.URI
import java.net.URISyntaxException

/** Server addresses as the app compares and keeps them. */
object Addresses {
    /**
     * The origin of [url] as browsers write it (`https://pley.example.com`,
     * `http://nas:7539`): scheme and host in lower case, the port only when
     * it isn't the scheme's own. `null` when [url] has no scheme or host.
     * A server's issuer is its origin, so tokens are matched against this.
     */
    fun origin(url: String): String? {
        val uri = try {
            URI(url)
        } catch (_: URISyntaxException) {
            return null
        }
        val scheme = uri.scheme?.lowercase() ?: return null
        val host = uri.host?.lowercase() ?: return null
        val defaultPort = uri.port == -1 || (scheme == "http" && uri.port == 80) || (scheme == "https" && uri.port == 443)
        return if (defaultPort) "$scheme://$host" else "$scheme://$host:${uri.port}"
    }
}
