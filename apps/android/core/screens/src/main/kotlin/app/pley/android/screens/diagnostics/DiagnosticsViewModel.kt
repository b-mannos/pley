package app.pley.android.screens.diagnostics

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.DeviceCapabilities
import app.pley.android.common.DeviceProfile
import app.pley.android.common.FormFactor
import app.pley.android.model.PleyJson
import app.pley.android.model.caps.ClientCaps
import app.pley.android.player.engine.BufferPlan
import app.pley.android.screens.screensGraph
import java.util.Locale
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

/** A labelled fact of Diagnostics: "Memory tier", "T1 (2.0 GB)". */
data class Fact(val label: String, val value: String)

/** Diagnostics' state. */
data class DiagnosticsUiState(
    /** The device: model, kind, memory and its tier, the player's buffers. */
    val device: List<Fact> = emptyList(),
    /**
     * What it plays, one line each: decoders (hardware or software, their
     * profiles and limits), HDR types, Dolby Vision profiles, the screen's
     * modes, passthrough formats, quirks. Empty until probed.
     */
    val lines: List<String> = emptyList(),
    /** The report sent with every playback request, as JSON (copyable on phones). */
    val capsJson: String = "",
    val probing: Boolean = true,
    /** The probe failed: its sentence. */
    val error: Throwable? = null,
)

/**
 * Diagnostics (both interfaces): what this device decodes and shows, and
 * why, for when something doesn't play. The capability probe runs if it
 * hasn't (off the main thread).
 */
class DiagnosticsViewModel(
    private val capabilities: DeviceCapabilities,
    profile: DeviceProfile,
    private val compute: CoroutineDispatcher = Dispatchers.Default,
) : ViewModel() {
    private val probing = MutableStateFlow(capabilities.snapshot.value == null)
    private val failure = MutableStateFlow<Throwable?>(null)
    private val device = deviceFacts(profile)

    val state: StateFlow<DiagnosticsUiState> = combine(capabilities.snapshot, probing, failure) { snapshot, busy, error ->
        DiagnosticsUiState(
            device = device,
            lines = snapshot?.lines.orEmpty(),
            capsJson = snapshot?.let(::capsJson).orEmpty(),
            probing = busy && snapshot == null,
            error = error,
        )
    }.flowOn(compute).stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), DiagnosticsUiState(device = device, probing = probing.value))

    init {
        probe()
    }

    /** Probes now unless a probe of this screen and output is saved. */
    fun probe() {
        probing.value = true
        failure.value = null
        viewModelScope.launch(compute) {
            try {
                capabilities.warmUp()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                failure.value = e
            } finally {
                probing.value = false
            }
        }
    }

    companion object {
        private val pretty = Json(from = PleyJson) { prettyPrint = true }

        /** The report of [snapshot] as the server receives it, indented. */
        fun capsJson(snapshot: CapsSnapshot): String = pretty.encodeToString(ClientCaps.serializer(), snapshot.report)

        /** What Diagnostics says of the device [profile]. */
        fun deviceFacts(profile: DeviceProfile, locale: Locale = Locale.getDefault()): List<Fact> {
            val kind = when {
                profile.fireTv -> "Fire TV"
                profile.form == FormFactor.Tv -> "TV"
                else -> "Phone"
            }
            val ram = String.format(locale, "%.1f GB", profile.totalRamBytes / GB)
            val plan = BufferPlan.of(profile)
            val buffer = buildString {
                append("${plan.minMs / 1000}-${plan.maxMs / 1000} s ahead")
                if (plan.targetBytes > 0) append(", ${plan.targetBytes / MIB} MiB")
                if (plan.backBufferMs > 0) append(", ${plan.backBufferMs / 1000} s behind")
            }
            return listOf(
                Fact("Model", profile.model),
                Fact("Kind", kind),
                Fact("Memory", if (profile.lowRam) "$ram (low-memory device)" else ram),
                Fact("Memory tier", profile.tier.name),
                Fact("Player buffer", buffer),
            )
        }

        private const val GB = 1_000_000_000.0
        private const val MIB = 1024 * 1024

        val factory: ViewModelProvider.Factory = viewModelFactory {
            initializer {
                val g = screensGraph()
                DiagnosticsViewModel(g.caps.capabilities, g.device)
            }
        }
    }
}
