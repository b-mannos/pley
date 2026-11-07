package app.pley.android.screens.admin.network

import app.pley.android.model.admin.ListenChoice
import app.pley.android.model.admin.NetworkAdapter
import app.pley.android.model.admin.ServerNetwork
import java.net.URI

/**
 * Where the server listens as Network's page edits it, as the web app's
 * Settings → Network does (apps/web/src/pages/settings/NetworkSettings.tsx):
 * a choice of networks and a port rather than addresses, which [choiceOf]
 * turns into them.
 */

/** Which networks the server listens on. */
enum class ListenMode {
    /** Only this computer: a reverse proxy on it, or the server on its own. */
    Local,

    /** This computer and the networks chosen. */
    Networks,

    /** Every network, those that come later too. */
    All,
}

/** The APIs for other apps, in the page's order, with their own ports. */
enum class CompatApi(val ownPort: Int) {
    Jellyfin(8096),
    Plex(32400),
}

/** Whether an API for other apps is on, and where: on the server's port ([onWeb]) or on [port]. */
data class ApiForm(val on: Boolean, val onWeb: Boolean, val port: Int)

/** What the page edits; [choiceOf] turns it into addresses. */
data class NetworkForm(
    val mode: ListenMode,
    /** The networks' addresses, for [ListenMode.Networks]. */
    val ips: List<String>,
    val port: Int,
    val jellyfin: ApiForm,
    val plex: ApiForm,
    val discovery: Boolean,
    val bonjour: Boolean,
    /** The adapters Bonjour announces from: every one when empty. */
    val bonjourAdapters: List<String>,
    /** Bonjour's host name as typed: the generated one when empty. */
    val bonjourHost: String,
) {
    fun api(api: CompatApi): ApiForm = when (api) {
        CompatApi.Jellyfin -> jellyfin
        CompatApi.Plex -> plex
    }

    fun withApi(api: CompatApi, form: ApiForm): NetworkForm = when (api) {
        CompatApi.Jellyfin -> copy(jellyfin = form)
        CompatApi.Plex -> copy(plex = form)
    }

    /** Whether discovery and Bonjour can work: other devices can't reach this computer's own address. */
    val onNetworks: Boolean get() = mode != ListenMode.Local
}

/** Why a [NetworkForm] can't be saved. */
enum class NetworkProblem {
    Port,
    JellyfinPortRange,
    JellyfinSamePort,
    PlexPortRange,
    PlexSamePort,
    SharedPort,
    NoNetwork,
    HostName,
}

/** The server's own port, when its addresses say none. */
const val DEFAULT_PORT = 7539

/** The ports an address can have. */
val PORTS = 1L..65535L

/** One DNS label of letters, digits and hyphens, not starting or ending with one; empty for the generated name. */
private val HOST_LABEL = Regex("^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)?$")

/** A host name as typed (`Media`, `media.local`) as the server keeps it: its label, lowercase. */
fun hostLabel(typed: String): String =
    typed.trim().trimEnd('.').lowercase().removeSuffix(".local")

/** Whether [typed] makes a host name Bonjour can use (empty: the generated one). */
fun hostNameOk(typed: String): Boolean = HOST_LABEL.matches(hostLabel(typed))

/** An address and its port: `192.168.1.10:7539`, or `[::1]:7539`. */
data class HostPort(val ip: String, val port: Int)

/** `192.168.1.10:7539` or `[::1]:7539`: its address and port (`-1` when it has none). */
fun splitAddress(address: String): HostPort {
    val at = address.lastIndexOf(':')
    if (at < 0) return HostPort(address, -1)
    return HostPort(address.substring(0, at).removeSurrounding("[", "]"), address.substring(at + 1).toIntOrNull() ?: -1)
}

private fun joinAddress(ip: String, port: Int): String = if (':' in ip) "[$ip]:$port" else "$ip:$port"

private fun isLoopback(ip: String): Boolean = ip.startsWith("127.") || ip == "::1"

private fun isAny(ip: String): Boolean = ip == "0.0.0.0" || ip == "::"

/** An API's form from its addresses of its own and whether it answers on the server's port. */
private fun apiFormOf(addresses: List<String>, onWeb: Boolean, port: Int): ApiForm {
    val own = addresses.map(::splitAddress)
    return ApiForm(on = own.isNotEmpty() || onWeb, onWeb = own.isEmpty(), port = own.firstOrNull()?.port ?: port)
}

/** The form for where the server listens now. */
fun formOf(choice: ListenChoice): NetworkForm {
    val web = choice.web.map(::splitAddress)
    val mode = when {
        web.any { isAny(it.ip) } -> ListenMode.All
        web.all { isLoopback(it.ip) } -> ListenMode.Local
        else -> ListenMode.Networks
    }
    return NetworkForm(
        mode = mode,
        ips = web.filter { !isLoopback(it.ip) && !isAny(it.ip) }.map { it.ip },
        port = web.firstOrNull()?.port ?: DEFAULT_PORT,
        jellyfin = apiFormOf(choice.jellyfin, choice.jellyfinOnWeb, CompatApi.Jellyfin.ownPort),
        plex = apiFormOf(choice.plex, choice.plexOnWeb, CompatApi.Plex.ownPort),
        discovery = choice.discovery,
        bonjour = choice.bonjour,
        bonjourAdapters = choice.bonjourAdapters,
        bonjourHost = choice.bonjourHostName,
    )
}

/** The addresses [form] stands for, but what the command line or the environment sets, which stays as [network] has it. */
fun choiceOf(form: NetworkForm, network: ServerNetwork): ListenChoice {
    val ips = when (form.mode) {
        ListenMode.All -> listOf("0.0.0.0")
        ListenMode.Local -> listOf("127.0.0.1")
        ListenMode.Networks -> listOf("127.0.0.1") + form.ips
    }
    fun own(api: ApiForm) = if (api.on && !api.onWeb) ips.map { joinAddress(it, api.port) } else emptyList()
    val fixed = network.fixed
    // Both APIs, always: the server takes one left out for off.
    return ListenChoice(
        web = if (fixed.web) network.web else ips.map { joinAddress(it, form.port) },
        jellyfin = if (fixed.jellyfin) network.jellyfin else own(form.jellyfin),
        jellyfinOnWeb = if (fixed.jellyfin) network.jellyfinOnWeb else form.jellyfin.on && form.jellyfin.onWeb,
        plex = if (fixed.plex) network.plex else own(form.plex),
        plexOnWeb = if (fixed.plex) network.plexOnWeb else form.plex.on && form.plex.onWeb,
        discovery = if (fixed.discovery) network.discovery else form.discovery,
        bonjour = if (fixed.bonjour) network.bonjour else form.bonjour,
        bonjourAdapters = form.bonjourAdapters.sorted(),
        bonjourHostName = if (hostNameOk(form.bonjourHost)) hostLabel(form.bonjourHost) else network.bonjourHostName,
    )
}

/** Why [form] can't be saved, if it can't. */
fun problemOf(form: NetworkForm): NetworkProblem? {
    fun port(p: Int) = p.toLong() in PORTS
    if (!port(form.port)) return NetworkProblem.Port
    for (api in CompatApi.entries) {
        val (on, onWeb, own) = form.api(api)
        if (!on || onWeb) continue
        if (!port(own)) return if (api == CompatApi.Jellyfin) NetworkProblem.JellyfinPortRange else NetworkProblem.PlexPortRange
        if (own == form.port) return if (api == CompatApi.Jellyfin) NetworkProblem.JellyfinSamePort else NetworkProblem.PlexSamePort
    }
    val (jellyfin, plex) = form.jellyfin to form.plex
    if (jellyfin.on && !jellyfin.onWeb && plex.on && !plex.onWeb && jellyfin.port == plex.port) return NetworkProblem.SharedPort
    if (form.mode == ListenMode.Networks && form.ips.isEmpty()) return NetworkProblem.NoNetwork
    // Only where it shows: hidden, the saved one stays (see choiceOf).
    if (form.bonjour && form.onNetworks && !hostNameOk(form.bonjourHost)) return NetworkProblem.HostName
    return null
}

/** [form] with [adapter] chosen, or no longer: every one of its addresses goes with it. */
fun toggleAdapter(form: NetworkForm, adapter: NetworkAdapter): NetworkForm {
    val chosen = adapter.addresses.any { it in form.ips }
    val ips = if (chosen) form.ips.filter { it !in adapter.addresses } else form.ips + adapter.addresses.filter { it !in form.ips }
    return form.copy(ips = ips)
}

/** The addresses chosen on an adapter that isn't here now (a VPN that is off). */
fun addressesElsewhere(form: NetworkForm, adapters: List<NetworkAdapter>): List<String> {
    val known = adapters.flatMap { it.addresses }.toSet()
    return form.ips.filter { it !in known }
}

/**
 * The adapters Bonjour can be announced on, by name: only those the server
 * listens on (announced elsewhere, it couldn't be reached), and [elsewhere]
 * the ones chosen before that it doesn't listen on now.
 */
data class BonjourChoices(val adapters: List<NetworkAdapter>, val elsewhere: List<String>)

fun bonjourChoices(form: NetworkForm, adapters: List<NetworkAdapter>): BonjourChoices {
    val offered = if (form.mode == ListenMode.Networks) adapters.filter { a -> a.addresses.any { it in form.ips } } else adapters
    val byName = offered.distinctBy { it.name }
    val names = byName.map { it.name }.toSet()
    return BonjourChoices(byName, form.bonjourAdapters.filter { it !in names })
}

/** This address's host as the server's addresses write it, or `null` for a name (a reverse proxy's, say), which says nothing. */
private fun hostOf(url: URI): String? {
    val host = url.host?.removeSurrounding("[", "]") ?: return null
    if (host.equals("localhost", ignoreCase = true)) return "127.0.0.1"
    return host.takeIf { Regex("^\\d{1,3}(\\.\\d{1,3}){3}$").matches(it) || ':' in it }
}

private fun reaches(ip: String, host: String): Boolean = isAny(ip) || ip == host || (isLoopback(host) && isLoopback(ip))

/**
 * Whether the server, listening on [next] instead of [now], still answers
 * at [server] (the address this phone reaches it at): `null` when it
 * doesn't answer there directly now (a reverse proxy does, or it is a
 * name), so can't tell.
 */
fun stillServes(now: List<String>, next: List<String>, server: String): Boolean? {
    val url = runCatching { URI(server) }.getOrNull() ?: return null
    val host = hostOf(url) ?: return null
    val port = if (url.port >= 0) url.port else if (url.scheme.equals("https", ignoreCase = true)) 443 else 80
    fun serves(list: List<String>) = list.map(::splitAddress).any { it.port == port && reaches(it.ip, host) }
    return if (serves(now)) serves(next) else null
}
