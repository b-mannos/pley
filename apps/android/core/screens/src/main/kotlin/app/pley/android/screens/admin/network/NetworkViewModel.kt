package app.pley.android.screens.admin.network

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.PleyJson
import app.pley.android.model.admin.ListenChoice
import app.pley.android.model.admin.NetworkAdapter
import app.pley.android.model.admin.ProxySettings
import app.pley.android.model.admin.ServerNetwork
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.AdminLoad
import app.pley.android.screens.admin.AdminViewModel
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.putJsonArray

/**
 * What Network's page shows: where the server listens ([network]), the
 * form as edited ([form]), and what saving it would do.
 */
data class NetworkPage(
    val network: ServerNetwork,
    val form: NetworkForm,
    /** What saving sends. */
    val next: ListenChoice,
    /** Changed on the page, if not yet in its addresses ("networks", none chosen). */
    val edited: Boolean,
    /** Saving would change where the server listens. */
    val dirty: Boolean,
    val problem: NetworkProblem?,
    /** Saving would stop the server listening where this phone reaches it. */
    val cutsOff: Boolean,
    /** The config file lists addresses the page doesn't show as they are: saving replaces them. */
    val unusual: Boolean,
) {
    /** Everything is set by the command line or the environment. */
    val fixedAll: Boolean
        get() = network.fixed.let { it.web && it.jellyfin && it.plex && it.discovery && it.bonjour }
}

/**
 * Network (the web's Settings → Network): where the server listens, the
 * Plex- and Jellyfin-compatible APIs, discovery and Bonjour, edited
 * together and saved with one button as the web's form is, since each
 * choice changes where the server listens at once; and the addresses it is
 * reached at through a reverse proxy, each sent as it is added or removed.
 * [server] is the address this phone reaches the server at, to say before
 * a change that it would lose it.
 */
class NetworkViewModel(admin: ServerAdmin, private val server: () -> String?) : AdminViewModel(admin) {
    val network = resource { get(ServerNetwork.serializer(), listOf("api", "network")) }

    /** The proxy addresses (in the server's settings); `null` addresses from a server from before them. */
    val proxy = resource { get(ProxySettings.serializer(), listOf("api", "settings")) }

    /** The form as edited, and the choice it started from: it starts again when where the server listens changes. */
    private data class Draft(val base: ListenChoice, val form: NetworkForm)

    private val draft = MutableStateFlow<Draft?>(null)

    val page: StateFlow<AdminLoad<NetworkPage>> = combine(network.state, draft) { load, edits ->
        when (load) {
            AdminLoad.Loading -> AdminLoad.Loading
            is AdminLoad.Ready -> AdminLoad.Ready(pageOf(load.value, edits))
            is AdminLoad.Failed -> AdminLoad.Failed(load.error, load.stale?.let { pageOf(it, edits) })
        }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, AdminLoad.Loading)

    private val outcomes = Channel<Boolean>(Channel.BUFFERED)

    /** A change the server took: `true` when some of its addresses don't listen yet. */
    val saved: Flow<Boolean> = outcomes.receiveAsFlow()

    private fun pageOf(network: ServerNetwork, edits: Draft?): NetworkPage {
        val start = formOf(network.choice)
        val form = edits?.takeIf { it.base == network.choice }?.form ?: start
        val saved = choiceOf(start, network)
        val next = choiceOf(form, network)
        val dirty = next != saved
        return NetworkPage(
            network = network,
            form = form,
            next = next,
            edited = form != start,
            dirty = dirty,
            problem = problemOf(form),
            cutsOff = dirty && server()?.let { stillServes(network.web, next.web, it) } == false,
            unusual = saved != network.choice,
        )
    }

    /** The page as it is now, from what was read and the edits, without waiting for [page] to follow. */
    private fun current(): NetworkPage? = network.state.value.valueOrNull?.let { pageOf(it, draft.value) }

    private fun edit(change: (NetworkForm) -> NetworkForm) {
        val now = current() ?: return
        draft.value = Draft(now.network.choice, change(now.form))
    }

    fun setMode(mode: ListenMode) = edit { it.copy(mode = mode) }

    fun toggleAdapter(adapter: NetworkAdapter) = edit { toggleAdapter(it, adapter) }

    /** Forgets an address chosen on an adapter that isn't here now. */
    fun dropAddress(ip: String) = edit { form -> form.copy(ips = form.ips - ip) }

    fun setPort(port: Int) = edit { it.copy(port = port) }

    fun setApiOn(api: CompatApi, on: Boolean) = edit { it.withApi(api, it.api(api).copy(on = on)) }

    fun setApiOnWeb(api: CompatApi, onWeb: Boolean) = edit { it.withApi(api, it.api(api).copy(onWeb = onWeb)) }

    fun setApiPort(api: CompatApi, port: Int) = edit { it.withApi(api, it.api(api).copy(port = port)) }

    fun setDiscovery(on: Boolean) = edit { it.copy(discovery = on) }

    fun setBonjour(on: Boolean) = edit { it.copy(bonjour = on) }

    fun setBonjourHost(typed: String) = edit { it.copy(bonjourHost = typed) }

    /** Bonjour announces on every network the server listens on. */
    fun announceEverywhere() = edit { it.copy(bonjourAdapters = emptyList()) }

    fun toggleBonjourAdapter(name: String) = edit { form ->
        form.copy(bonjourAdapters = if (name in form.bonjourAdapters) form.bonjourAdapters - name else form.bonjourAdapters + name)
    }

    /** Back to where the server listens. */
    fun reset() {
        draft.value = null
    }

    /**
     * Listens as the form says. The answer is where the server listens
     * then, shown at once; [saved] says whether every address listens yet.
     * A refusal keeps the form, to change.
     */
    fun save(): Job? {
        val now = current()?.takeIf { it.dirty && it.problem == null } ?: return null
        var took = false
        return change(after = { if (!took) network.reload() }) {
            val answer = send("PUT", ServerNetwork.serializer(), listOf("api", "network"), PleyJson.encodeToJsonElement(ListenChoice.serializer(), now.next))
            took = true
            draft.value = null
            network.set(answer)
            outcomes.send(answer.listeners.any { it.error != null })
        }
    }

    /** Adds an address the server is reached at through a reverse proxy. */
    fun addProxy(address: String): Job? {
        val addresses = proxy.state.value.valueOrNull?.proxyAddresses ?: return null
        return saveProxies(addresses + address.trim())
    }

    fun removeProxy(address: String): Job? {
        val addresses = proxy.state.value.valueOrNull?.proxyAddresses ?: return null
        return saveProxies(addresses - address)
    }

    private fun saveProxies(addresses: List<String>): Job {
        var took = false
        return change(said = AdminDone(PROXY_SAVED), after = { if (!took) proxy.reload() }) {
            val body = buildJsonObject { putJsonArray("proxy_addresses") { addresses.forEach { add(JsonPrimitive(it)) } } }
            val answer = send("PUT", ProxySettings.serializer(), listOf("api", "settings"), body)
            took = true
            proxy.set(answer)
        }
    }

    /**
     * While the page is on screen: where an address doesn't listen yet,
     * reads again every few seconds, as the server tries it again.
     */
    suspend fun watchListeners() {
        while (true) {
            delay(RETRY_READ_MS)
            val waiting = network.state.value.valueOrNull?.listeners?.any { it.error != null } == true
            if (waiting && busy.value == 0) network.reload()
        }
    }

    companion object {
        const val PROXY_SAVED = "network.proxy_saved"

        /** How often a page with addresses that don't listen yet reads again, as the web's does. */
        const val RETRY_READ_MS = 5_000L

        val factory: ViewModelProvider.Factory = viewModelFactory {
            initializer {
                val data = screensGraph().data
                NetworkViewModel(data.admin) { data.session.session.value?.issuer }
            }
        }
    }
}
