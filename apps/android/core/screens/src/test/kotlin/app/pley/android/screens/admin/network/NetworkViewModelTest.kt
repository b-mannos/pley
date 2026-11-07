package app.pley.android.screens.admin.network

import app.pley.android.model.PleyJson
import app.pley.android.model.admin.ListenChoice
import app.pley.android.model.admin.NetworkListener
import app.pley.android.network.ApiException
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.testing.MainDispatcherRule
import app.pley.android.testing.data.FakeServerAdmin
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.putJsonArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Network's page: it reads where the server listens and the proxy
 * addresses, edits the choice and sends it whole with Save, says before a
 * change that would lose this phone the server, and sends each proxy
 * address as it is added or removed.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class NetworkViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val network = """
        {
          "web": ["127.0.0.1:7539"], "jellyfin": [], "jellyfin_on_web": false, "plex": [], "plex_on_web": true,
          "discovery": true, "bonjour": false, "bonjour_adapters": [], "bonjour_host_name": "",
          "bonjour_host_default": "pley-0123456789ab",
          "fixed": { "web": false, "jellyfin": false, "plex": false, "discovery": false, "bonjour": false },
          "listeners": [{ "kind": "web", "address": "127.0.0.1:7539", "error": null }],
          "adapters": [
            { "name": "Ethernet", "description": "A network card", "up": true, "addresses": ["192.168.1.10"],
              "network": { "name": "Home", "kind": "private" } },
            { "name": "Wi-Fi", "description": null, "up": false, "addresses": ["10.0.0.5"], "network": null }
          ],
          "config_file": "/config/pley.toml", "windows": true
        }
    """

    private fun base(changes: (JsonObject) -> JsonObject = { it }): JsonObject = changes(PleyJson.parseToJsonElement(network).jsonObject)

    private fun JsonObject.with(vararg fields: Pair<String, String>): JsonObject =
        JsonObject(this + fields.associate { (k, v) -> k to PleyJson.parseToJsonElement(v) })

    /** A server whose PUT /api/network answers what it was sent, with the listeners [listeners] makes of it. */
    private fun server(start: JsonObject = base(), listeners: (ListenChoice) -> String? = { null }): FakeServerAdmin {
        val server = FakeServerAdmin()
        var now = start
        server.answer("GET /api/network") { now }
        server.answer("PUT /api/network") { body ->
            val choice = PleyJson.decodeFromJsonElement(ListenChoice.serializer(), body!!)
            now = JsonObject(now + body.jsonObject)
            listeners(choice)?.let { now = now.with("listeners" to it) }
            now
        }
        var proxies: JsonElement = PleyJson.parseToJsonElement("""{ "server_name": "Den", "proxy_addresses": [] }""")
        server.answer("GET /api/settings") { proxies }
        server.answer("PUT /api/settings") { body ->
            proxies = JsonObject(proxies.jsonObject + body!!.jsonObject)
            proxies
        }
        return server
    }

    private fun page(model: NetworkViewModel) = model.page.value.valueOrNull!!

    private fun sent(server: FakeServerAdmin): ListenChoice =
        PleyJson.decodeFromJsonElement(ListenChoice.serializer(), server.callsTo("PUT /api/network").last().body!!)

    private fun choice(
        web: List<String>,
        jellyfin: List<String> = emptyList(),
        jellyfinOnWeb: Boolean = false,
        plex: List<String> = emptyList(),
        plexOnWeb: Boolean = true,
        bonjour: Boolean = false,
        bonjourAdapters: List<String> = emptyList(),
        bonjourHostName: String = "",
    ) = ListenChoice(web, jellyfin, jellyfinOnWeb, plex, plexOnWeb, discovery = true, bonjour, bonjourAdapters, bonjourHostName)

    @Test
    fun listensOnTheNetworksChosen() = runTest {
        val server = server(listeners = { choice -> choice.web.joinToString(",", "[", "]") { """{ "kind": "web", "address": "$it" }""" } })
        val model = NetworkViewModel(server) { "http://localhost:7539" }
        val saved = mutableListOf<Boolean>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { model.saved.collect { saved += it } }
        assertEquals(ListenMode.Local, page(model).form.mode)
        assertFalse(page(model).dirty)
        assertNull(model.save())

        model.setMode(ListenMode.Networks)
        assertTrue(page(model).edited)
        assertEquals(NetworkProblem.NoNetwork, page(model).problem)
        assertNull(model.save())
        model.toggleAdapter(page(model).network.adapters[0])
        assertNull(page(model).problem)
        assertFalse(page(model).cutsOff)
        model.save()!!.join()

        assertEquals(choice(listOf("127.0.0.1:7539", "192.168.1.10:7539")), sent(server))
        assertEquals(listOf(false), saved)
        // The answer is shown, and the form starts from it.
        assertEquals(2, page(model).network.listeners.size)
        assertEquals(ListenMode.Networks, page(model).form.mode)
        assertFalse(page(model).edited)
        assertEquals(1, server.callsTo("GET /api/network").size)
    }

    @Test
    fun turnsTheJellyfinCompatibleApiOnOnPleysPortOrItsOwn() = runTest {
        val server = server(base { it.with("web" to """["0.0.0.0:7539"]""") })
        val model = NetworkViewModel(server) { null }
        model.setApiOn(CompatApi.Jellyfin, true)
        assertTrue(page(model).form.jellyfin.onWeb)
        model.save()!!.join()
        assertEquals(choice(listOf("0.0.0.0:7539"), jellyfinOnWeb = true), sent(server))

        // A port of its own: Jellyfin's, or another typed in.
        model.setApiOnWeb(CompatApi.Jellyfin, false)
        assertEquals(8096, page(model).form.jellyfin.port)
        model.setApiPort(CompatApi.Jellyfin, 7539)
        assertEquals(NetworkProblem.JellyfinSamePort, page(model).problem)
        model.setApiPort(CompatApi.Jellyfin, 32400)
        model.setApiOnWeb(CompatApi.Plex, false)
        assertEquals(NetworkProblem.SharedPort, page(model).problem)
        model.setApiPort(CompatApi.Jellyfin, 8920)
        model.save()!!.join()
        assertEquals(choice(listOf("0.0.0.0:7539"), jellyfin = listOf("0.0.0.0:8920"), plex = listOf("0.0.0.0:32400"), plexOnWeb = false), sent(server))

        // And off again, said as such rather than left out.
        model.setApiOn(CompatApi.Jellyfin, false)
        model.setApiOn(CompatApi.Plex, false)
        model.save()!!.join()
        assertEquals(choice(listOf("0.0.0.0:7539"), plexOnWeb = false), sent(server))
    }

    @Test
    fun leavesWhatTheCommandLineSetsAlone() = runTest {
        val fixed = base {
            it.with(
                "plex" to """["0.0.0.0:32400"]""",
                "plex_on_web" to "false",
                "fixed" to """{ "web": false, "jellyfin": false, "plex": true, "discovery": false, "bonjour": false }""",
            )
        }
        val server = server(fixed)
        val model = NetworkViewModel(server) { null }
        model.setApiOn(CompatApi.Plex, false)
        model.setApiOn(CompatApi.Jellyfin, true)
        model.save()!!.join()
        assertEquals(choice(listOf("127.0.0.1:7539"), jellyfinOnWeb = true, plex = listOf("0.0.0.0:32400"), plexOnWeb = false), sent(server))
    }

    @Test
    fun aRefusedChoiceIsSaidAndStaysToChange() = runTest {
        val server = server()
        val refused = ApiException(400, "bad_request", "the Plex-compatible API can't share port 32400 with the Jellyfin-compatible API")
        server.fail("PUT /api/network", refused)
        val model = NetworkViewModel(server) { null }
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { model.errors.collect { errors += it } }
        model.setApiOnWeb(CompatApi.Plex, false)
        model.save()!!.join()
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals(2, server.callsTo("GET /api/network").size)
        assertEquals(ApiForm(on = true, onWeb = false, port = 32400), page(model).form.plex)
        assertTrue(page(model).dirty)
        assertEquals(0, model.busy.value)
    }

    @Test
    fun announcesByBonjourOnTheNetworksChosen() = runTest {
        val server = server(base { it.with("web" to """["0.0.0.0:7539"]""") })
        val model = NetworkViewModel(server) { null }
        model.setBonjour(true)
        model.setBonjourHost("my media")
        assertEquals(NetworkProblem.HostName, page(model).problem)
        model.setBonjourHost("Media.local")
        model.toggleBonjourAdapter("Ethernet")
        model.save()!!.join()
        assertEquals(choice(listOf("0.0.0.0:7539"), bonjour = true, bonjourAdapters = listOf("Ethernet"), bonjourHostName = "media"), sent(server))

        // Every network again.
        model.announceEverywhere()
        model.save()!!.join()
        assertEquals(emptyList<String>(), sent(server).bonjourAdapters)
    }

    @Test
    fun saysBeforeAChangeThatWouldLoseThisPhoneTheServer() = runTest {
        val server = server(base { it.with("web" to """["0.0.0.0:7539"]""") })
        var at = "http://192.168.1.10:7539"
        val model = NetworkViewModel(server) { at }
        model.setMode(ListenMode.Local)
        assertTrue(page(model).cutsOff)
        model.setMode(ListenMode.Networks)
        model.toggleAdapter(page(model).network.adapters[0])
        assertFalse(page(model).cutsOff)
        model.reset()
        model.setPort(8000)
        assertTrue(page(model).cutsOff)
        // Reached at a name (a reverse proxy's, say): can't tell.
        at = "https://media.example.com"
        model.setPort(8001)
        assertFalse(page(model).cutsOff)
    }

    @Test
    fun saysWhenSomeAddressesDontListenYetAndReadsAgainWhileTheyWait() = runTest {
        val waiting = """[{ "kind": "web", "address": "0.0.0.0:7539" }, { "kind": "plex", "address": "0.0.0.0:32400", "error": "address in use" }]"""
        val server = server(listeners = { waiting })
        val model = NetworkViewModel(server) { null }
        val saved = mutableListOf<Boolean>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { model.saved.collect { saved += it } }
        val watching = launch { model.watchListeners() }
        advanceTimeBy(NetworkViewModel.RETRY_READ_MS + 1)
        // Nothing waits: nothing is read again.
        assertEquals(1, server.callsTo("GET /api/network").size)

        model.setMode(ListenMode.All)
        model.setApiOnWeb(CompatApi.Plex, false)
        model.save()!!.join()
        assertEquals(listOf(true), saved)
        advanceTimeBy(NetworkViewModel.RETRY_READ_MS + 1)
        runCurrent()
        assertEquals(2, server.callsTo("GET /api/network").size)
        watching.cancel()
    }

    @Test
    fun addsAndRemovesProxyAddresses() = runTest {
        val server = server()
        val model = NetworkViewModel(server) { null }
        val done = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { model.confirmations.collect { done += it } }
        assertEquals(emptyList<String>(), model.proxy.state.value.valueOrNull?.proxyAddresses)

        model.addProxy(" https://media.example.com:9999 ")!!.join()
        assertEquals(
            buildJsonObject { putJsonArray("proxy_addresses") { add(JsonPrimitive("https://media.example.com:9999")) } },
            server.callsTo("PUT /api/settings").last().body,
        )
        assertEquals(listOf("https://media.example.com:9999"), model.proxy.state.value.valueOrNull?.proxyAddresses)
        assertEquals(listOf(AdminDone(NetworkViewModel.PROXY_SAVED)), done)

        model.removeProxy("https://media.example.com:9999")!!.join()
        assertEquals(0, server.callsTo("PUT /api/settings").last().body!!.jsonObject.getValue("proxy_addresses").jsonArray.size)
        // Answered, nothing is read again.
        assertEquals(1, server.callsTo("GET /api/settings").size)
    }

    @Test
    fun aRefusedProxyAddressIsSaidAndTheAddressesReadAgain() = runTest {
        val server = server()
        val refused = ApiException(400, "bad_request", "an address with a port")
        server.fail("PUT /api/settings", refused)
        val model = NetworkViewModel(server) { null }
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { model.errors.collect { errors += it } }
        model.addProxy("https://media.example.com")!!.join()
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals(2, server.callsTo("GET /api/settings").size)
        assertEquals("https://media.example.com", server.callsTo("PUT /api/settings").last().body!!.jsonObject.getValue("proxy_addresses").jsonArray[0].jsonPrimitive.content)
    }

    @Test
    fun aServerFromBeforeProxyAddressesHasNoneToChange() = runTest {
        val server = server()
        server.answer("GET /api/settings", """{ "server_name": "Den" }""")
        val model = NetworkViewModel(server) { null }
        assertNull(model.proxy.state.value.valueOrNull?.proxyAddresses)
        assertNull(model.addProxy("https://media.example.com:9999"))
        assertTrue(server.callsTo("PUT /api/settings").isEmpty())
        // The network page reads on regardless.
        assertEquals(listOf(NetworkListener(address = "127.0.0.1:7539")), page(model).network.listeners)
    }
}
