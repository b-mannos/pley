package app.pley.android.screens.admin.transcoder

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.admin.HwAccel
import app.pley.android.model.admin.ToneMapping
import app.pley.android.model.admin.TranscodeFolder
import app.pley.android.model.admin.TranscoderSettings
import app.pley.android.model.admin.TranscoderSystem
import app.pley.android.model.admin.TranscoderTestResult
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.AdminViewModel
import app.pley.android.screens.admin.folders.FolderBrowser
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** What Settings → Transcoder shows: its settings, and the server's system (`null` when it couldn't be read: nothing is greyed out). */
data class TranscoderPage(val settings: TranscoderSettings, val system: TranscoderSystem?)

/** Testing the transcoder: not yet, under way, its answer, or why it couldn't be asked. */
sealed interface TranscoderTest {
    data object Idle : TranscoderTest

    data object Testing : TranscoderTest

    data class Done(val result: TranscoderTestResult) : TranscoderTest

    data class Failed(val error: Throwable) : TranscoderTest
}

/**
 * Settings → Transcoder (apps/web/src/pages/settings/TranscoderSettings.tsx):
 * the transcoder's settings, each saved as it is chosen, with what its
 * FFmpeg can do (GET /api/system) deciding what is offered; testing it;
 * and the folder it works in, picked on the server.
 */
class TranscoderAdminViewModel(admin: ServerAdmin) : AdminViewModel(admin) {
    val page = resource {
        coroutineScope {
            val settings = async { get(TranscoderSettings.serializer(), SETTINGS) }
            // Without the system, the settings still show, as on the web: nothing is greyed out.
            val system = async {
                try {
                    get(TranscoderSystem.serializer(), SYSTEM)
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    null
                }
            }
            TranscoderPage(settings.await(), system.await())
        }
    }

    /** Picking the transcode folder on the server. */
    val folders = FolderBrowser(viewModelScope, admin::api)

    private val testNow = MutableStateFlow<TranscoderTest>(TranscoderTest.Idle)
    val test: StateFlow<TranscoderTest> = testNow.asStateFlow()

    private val folderSaved = Channel<AdminDone>(Channel.BUFFERED)

    /** The transcode folder was saved: from now ([FOLDER_SAVED]), or from Pley's next start ([FOLDER_NEXT_START]). */
    val folderNotices: Flow<AdminDone> = folderSaved.receiveAsFlow()

    /**
     * Chooses the hardware acceleration, with what goes with it, as the web
     * sends it: a tone mapper it can't use gives way to Automatic, and
     * Vulkan turns hardware decoding on. What changed is said ([FITTED],
     * with the [FitNote]s).
     */
    fun chooseAccel(accel: HwAccel) {
        val page = page.state.value.valueOrNull ?: return
        if (accel == page.settings.hwaccel) return
        val fit = TranscodeChoices.fitToAccel(page.settings, accel, page.system)
        val body = buildJsonObject {
            put("hwaccel", accel.wire)
            fit.tonemapping?.let { put("tonemapping", it.wire) }
            fit.hardwareDecoding?.let { put("hardware_decoding", it) }
        }
        val said = fit.notes.takeIf { it.isNotEmpty() }?.let { AdminDone(FITTED, it) }
        save(body, said) {
            it.copy(
                hwaccel = accel,
                tonemapping = fit.tonemapping ?: it.tonemapping,
                hardwareDecoding = fit.hardwareDecoding ?: it.hardwareDecoding,
            )
        }
    }

    fun setHardwareDecoding(on: Boolean) = save(buildJsonObject { put("hardware_decoding", on) }) { it.copy(hardwareDecoding = on) }

    fun setPreset(preset: String) = save(buildJsonObject { put("transcode_preset", preset) }) { it.copy(transcodePreset = preset) }

    fun setTonemapping(mode: ToneMapping) = save(buildJsonObject { put("tonemapping", mode.wire) }) { it.copy(tonemapping = mode) }

    fun setMaxTranscodes(count: Int) = save(buildJsonObject { put("max_transcodes", count) }) { it.copy(maxTranscodes = count) }

    fun setThrottle(on: Boolean) = save(buildJsonObject { put("throttle_transcodes", on) }) { it.copy(throttleTranscodes = on) }

    fun setInMemory(on: Boolean) = save(buildJsonObject { put("transcode_in_memory", on) }) { it.copy(transcodeInMemory = on) }

    /** Asks the transcoder to answer; [test] says how it went, in the server's words. */
    fun testTranscoder() {
        if (testNow.value == TranscoderTest.Testing) return
        testNow.value = TranscoderTest.Testing
        viewModelScope.launch {
            testNow.value = try {
                val body = buildJsonObject { put("service", "transcoder") }
                TranscoderTest.Done(admin().send("POST", TranscoderTestResult.serializer(), TEST, body))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                TranscoderTest.Failed(e)
            }
        }
    }

    /** Chooses the folder the transcoder works in from Pley's next start; `null`: the default one. */
    fun chooseFolder(path: String?) = change {
        val body = buildJsonObject { if (path == null) put("path", JsonNull) else put("path", path) }
        val folder = send("PUT", TranscodeFolder.serializer(), TRANSCODE_DIR, body)
        val now = page.state.value.valueOrNull
        if (now?.system != null) page.set(now.copy(system = now.system.copy(transcodeDir = folder)))
        folderSaved.send(AdminDone(if (folder.next != null) FOLDER_NEXT_START else FOLDER_SAVED))
    }

    /**
     * Sends [body], a part of the settings, showing what it changes at once
     * ([shown]), then the settings as the server saved them; a refusal
     * puts back what the server has.
     */
    private fun save(body: JsonObject, said: AdminDone? = null, shown: (TranscoderSettings) -> TranscoderSettings) {
        page.state.value.valueOrNull?.let { page.set(it.copy(settings = shown(it.settings))) }
        var saved = false
        change(said, after = { if (!saved) page.reload() }) {
            val answer = send("PUT", TranscoderSettings.serializer(), SETTINGS, body)
            saved = true
            page.state.value.valueOrNull?.let { page.set(it.copy(settings = answer)) }
        }
    }

    companion object {
        /** What a hardware acceleration chosen changed; its args are [FitNote]s. */
        const val FITTED = "transcoder.fitted"
        const val FOLDER_SAVED = "transcoder.folder.saved"
        const val FOLDER_NEXT_START = "transcoder.folder.savedNextStart"

        private val SETTINGS = listOf("api", "settings")
        private val SYSTEM = listOf("api", "system")
        private val TEST = listOf("api", "settings", "test")
        private val TRANSCODE_DIR = listOf("api", "system", "transcode-dir")

        val factory: ViewModelProvider.Factory = viewModelFactory {
            initializer { TranscoderAdminViewModel(screensGraph().data.admin) }
        }
    }
}
