package app.pley.android.screens.admin.transcoder

import app.pley.android.model.PleyJson
import app.pley.android.model.admin.HwAccel
import app.pley.android.model.admin.ToneMapping
import app.pley.android.network.ApiException
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.testing.MainDispatcherRule
import app.pley.android.testing.data.FakeServerAdmin
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/** Settings → Transcoder: what it reads, each choice as it is sent, testing, and the transcode folder. */
@OptIn(ExperimentalCoroutinesApi::class)
class TranscoderAdminViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val system = """{"os":"linux","transcode_dir":{"in_use":"/data/transcode","next":null,"default":"/data/transcode","problem":null,"fixed":false},
        "transcoder":{"available":true,"managed":true,"encoders":["libx264"],"accels":[],"hardware_decoding":false,"with_accel":{},
        "tonemapping":["libplacebo","zscale"],"throttling":true,"memory":{"available":true,"budget_bytes":1024},"max_jobs":8}}"""

    /** A server whose settings take what is sent, as PUT /api/settings answers the whole settings. */
    private fun server(): FakeServerAdmin {
        var settings: JsonObject = PleyJson.parseToJsonElement(
            """{"server_name":"Den","hwaccel":"auto","hardware_decoding":false,"tonemapping":"zscale","max_transcodes":4,
            "transcode_preset":"auto","throttle_transcodes":true,"transcode_in_memory":true}""",
        ).jsonObject
        return FakeServerAdmin().apply {
            answer("GET /api/settings") { settings }
            answer("GET /api/system", system)
            answer("PUT /api/settings") { body: JsonElement? ->
                settings = JsonObject(settings + body!!.jsonObject)
                settings
            }
        }
    }

    @Test
    fun itReadsTheSettingsAndWhatTheTranscoderCanDo() = runTest {
        val page = TranscoderAdminViewModel(server()).page.state.value.valueOrNull!!
        assertEquals(ToneMapping.Zscale, page.settings.tonemapping)
        assertEquals(listOf("libplacebo", "zscale"), page.system?.transcoder?.tonemapping)
        assertEquals("/data/transcode", page.system?.transcodeDir?.inUse)
    }

    @Test
    fun withoutTheSystemTheSettingsStillShow() = runTest {
        val server = server()
        server.fail("GET /api/system", ApiException(500, "internal", "it broke"))
        val page = TranscoderAdminViewModel(server).page.state.value.valueOrNull!!
        assertEquals(4, page.settings.maxTranscodes)
        assertNull(page.system)
    }

    @Test
    fun eachChoiceIsSentAloneAsItIsMade() = runTest {
        val server = server()
        val page = TranscoderAdminViewModel(server)
        page.setMaxTranscodes(6)
        page.setPreset("veryfast")
        page.setThrottle(false)
        page.setInMemory(false)
        page.setHardwareDecoding(true)
        page.setTonemapping(ToneMapping.Libplacebo)
        assertEquals(
            listOf(
                """{"max_transcodes":6}""",
                """{"transcode_preset":"veryfast"}""",
                """{"throttle_transcodes":false}""",
                """{"transcode_in_memory":false}""",
                """{"hardware_decoding":true}""",
                """{"tonemapping":"libplacebo"}""",
            ),
            server.callsTo("PUT /api/settings").map { it.body.toString() },
        )
        val settings = page.page.state.value.valueOrNull!!.settings
        assertEquals(6, settings.maxTranscodes)
        assertEquals("veryfast", settings.transcodePreset)
        assertEquals(ToneMapping.Libplacebo, settings.tonemapping)
    }

    @Test
    fun choosingVulkanSendsWhatGoesWithItAndSaysWhatChanged() = runTest {
        val server = server()
        val page = TranscoderAdminViewModel(server)
        val done = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.confirmations.collect { done += it } }
        page.chooseAccel(HwAccel.Vulkan)
        assertEquals(
            """{"hwaccel":"vulkan","tonemapping":"auto","hardware_decoding":true}""",
            server.callsTo("PUT /api/settings").single().body.toString(),
        )
        assertEquals(
            listOf(AdminDone(TranscoderAdminViewModel.FITTED, listOf(FitNote.LibplaceboOnly(HwAccel.Vulkan), FitNote.Decoding(HwAccel.Vulkan)))),
            done,
        )
        val settings = page.page.state.value.valueOrNull!!.settings
        assertEquals(HwAccel.Vulkan, settings.hwaccel)
        assertEquals(ToneMapping.Auto, settings.tonemapping)
        // The same choice again sends nothing.
        page.chooseAccel(HwAccel.Vulkan)
        assertEquals(1, server.callsTo("PUT /api/settings").size)
    }

    @Test
    fun aRefusedChoiceSaysWhyAndShowsWhatTheServerHas() = runTest {
        val server = server()
        val refused = ApiException(400, "bad_request", "NVIDIA NVENC isn't available here")
        server.fail("PUT /api/settings", refused)
        val page = TranscoderAdminViewModel(server)
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.errors.collect { errors += it } }
        page.chooseAccel(HwAccel.Nvenc)
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals(HwAccel.Auto, page.page.state.value.valueOrNull!!.settings.hwaccel)
        assertEquals(2, server.callsTo("GET /api/settings").size)
    }

    @Test
    fun testingTheTranscoderSaysHowItWent() = runTest {
        val server = server()
        server.answer("POST /api/settings/test", """{"ok":true,"message":"Transcoder answered: pley-transcoder 0.1.6.","ms":9}""")
        val page = TranscoderAdminViewModel(server)
        assertEquals(TranscoderTest.Idle, page.test.value)
        page.testTranscoder()
        assertEquals("""{"service":"transcoder"}""", server.callsTo("POST /api/settings/test").single().body.toString())
        val result = (page.test.value as TranscoderTest.Done).result
        assertTrue(result.ok)
        assertEquals(9L, result.ms)

        server.fail("POST /api/settings/test", ApiException(503, "unavailable", "busy"))
        page.testTranscoder()
        assertTrue(page.test.value is TranscoderTest.Failed)
    }

    @Test
    fun theTranscodeFolderIsChosenForTheNextStartOrPutBack() = runTest {
        val server = server()
        server.answer("PUT /api/system/transcode-dir") { body ->
            val next = if (body!!.jsonObject["path"].toString() == "null") "null" else "\"/fast/pley\""
            PleyJson.parseToJsonElement("""{"in_use":"/data/transcode","next":$next,"default":"/data/transcode","fixed":false}""")
        }
        val page = TranscoderAdminViewModel(server)
        val said = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.folderNotices.collect { said += it } }
        page.chooseFolder("/fast/pley").join()
        assertEquals("""{"path":"/fast/pley"}""", server.callsTo("PUT /api/system/transcode-dir").last().body.toString())
        assertEquals("/fast/pley", page.page.state.value.valueOrNull?.system?.transcodeDir?.next)
        page.chooseFolder(null).join()
        // The default is asked for with an explicit null.
        assertEquals(buildJsonObject { put("path", JsonNull) }, server.callsTo("PUT /api/system/transcode-dir").last().body)
        assertNull(page.page.state.value.valueOrNull?.system?.transcodeDir?.next)
        assertEquals(
            listOf(AdminDone(TranscoderAdminViewModel.FOLDER_NEXT_START), AdminDone(TranscoderAdminViewModel.FOLDER_SAVED)),
            said,
        )
    }
}
