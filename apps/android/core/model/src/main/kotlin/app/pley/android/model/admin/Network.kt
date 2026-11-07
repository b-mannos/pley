package app.pley.android.model.admin

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Where the server listens (GET and PUT /api/network, docs/API.md
 * "System"): the addresses of its web app and the Pley apps, of the Plex-
 * and Jellyfin-compatible APIs, discovery and Bonjour, with what the
 * command line or the environment sets, the listeners as they are now and
 * the computer's network adapters. Addresses are written as the server
 * writes them: `192.168.1.10:7539`, `[::1]:7539`.
 */
@Serializable
data class ServerNetwork(
    val web: List<String> = emptyList(),
    /** The Plex-compatible API on addresses of its own: empty when off or on Pley's port. */
    val plex: List<String> = emptyList(),
    val plexOnWeb: Boolean = false,
    /** The Jellyfin-compatible API on addresses of its own: empty when off or on Pley's port. */
    val jellyfin: List<String> = emptyList(),
    val jellyfinOnWeb: Boolean = false,
    val discovery: Boolean = false,
    val bonjour: Boolean = false,
    /** The adapters Bonjour announces from, by name: every one Pley listens on when empty. */
    val bonjourAdapters: List<String> = emptyList(),
    /** Bonjour's host name, the label before `.local`: [bonjourHostDefault] when empty. */
    val bonjourHostName: String = "",
    val bonjourHostDefault: String = "",
    val fixed: NetworkFixed = NetworkFixed(),
    val listeners: List<NetworkListener> = emptyList(),
    val adapters: List<NetworkAdapter> = emptyList(),
    /** Where a change is saved. */
    val configFile: String = "",
    /** Windows Firewall decides which kinds of network reach the server. */
    val windows: Boolean = false,
) {
    /** Where it listens, as a PUT sends it. */
    val choice: ListenChoice
        get() = ListenChoice(web, jellyfin, jellyfinOnWeb, plex, plexOnWeb, discovery, bonjour, bonjourAdapters, bonjourHostName)
}

/**
 * Where the server is to listen: PUT /api/network's body. Every field is
 * sent, as the server takes one left out for off.
 */
@Serializable
data class ListenChoice(
    val web: List<String>,
    val jellyfin: List<String>,
    val jellyfinOnWeb: Boolean,
    val plex: List<String>,
    val plexOnWeb: Boolean,
    val discovery: Boolean,
    val bonjour: Boolean,
    val bonjourAdapters: List<String>,
    val bonjourHostName: String,
)

/** What the command line or the environment sets: the page shows it and can't change it. */
@Serializable
data class NetworkFixed(
    val web: Boolean = false,
    val jellyfin: Boolean = false,
    val plex: Boolean = false,
    val discovery: Boolean = false,
    val bonjour: Boolean = false,
)

/** What listens at an address. */
@Serializable
enum class ListenerKind {
    @SerialName("web") Web,
    @SerialName("jellyfin") Jellyfin,
    @SerialName("plex") Plex,
    @SerialName("discovery") Discovery,
}

/** A listener, and whether it listens: [error] says why it doesn't (yet). */
@Serializable
data class NetworkListener(
    val kind: ListenerKind = ListenerKind.Web,
    val address: String = "",
    val error: String? = null,
)

/** One of the computer's network adapters, with its IPv4 addresses. */
@Serializable
data class NetworkAdapter(
    /** "Wi-Fi", "Ethernet", "eth0". */
    val name: String = "",
    /** The hardware, on Windows. */
    val description: String? = null,
    /** Connected. */
    val up: Boolean = false,
    val addresses: List<String> = emptyList(),
    /** The network it is on, as Windows knows it. */
    val network: NetworkProfile? = null,
)

/** A network as Windows knows it: its name and [kind], `public`, `private` or `domain`. */
@Serializable
data class NetworkProfile(
    val name: String = "",
    val kind: String = "",
)

/**
 * The server's settings Network shows (GET and PUT /api/settings): the
 * addresses with a port it is reached at through a reverse proxy that
 * leaves the port out. `null` from a server from before the setting.
 */
@Serializable
data class ProxySettings(
    val proxyAddresses: List<String>? = null,
)
