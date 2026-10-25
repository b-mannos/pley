package app.pley.android.testing.caps

import app.pley.android.caps.CapsBuilder
import app.pley.android.caps.CapsGraph
import app.pley.android.caps.CapsSettings
import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.DefaultPlaybackCapsPolicy
import app.pley.android.caps.DeviceCapabilities
import app.pley.android.caps.PlaybackCapsPolicy
import app.pley.android.caps.ProbeInput
import app.pley.android.caps.QuirkTable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** The snapshot the app would build for [input], with the shipped quirks. */
fun capsOf(input: ProbeInput, settings: CapsSettings = CapsSettings()): CapsSnapshot =
    CapsBuilder.build(input, TestQuirks.shipped, settings)

/**
 * Device capabilities for tests: [warmUp] hands out [probe]'s snapshot
 * (an SDR phone unless said otherwise), [emit] stands for the screen or
 * output changing, and what the app asked for is recorded.
 */
class FakeDeviceCapabilities(
    private val probe: CapsSnapshot = capsOf(TestDevices.sdrPhone),
    initial: CapsSnapshot? = null,
) : DeviceCapabilities {
    private val state = MutableStateFlow(initial)
    override val snapshot: StateFlow<CapsSnapshot?> = state.asStateFlow()

    var warmUps = 0
        private set
    val displays = mutableListOf<Int>()
    val settings = mutableListOf<CapsSettings>()
    val quirkTables = mutableListOf<QuirkTable>()

    override suspend fun warmUp(): CapsSnapshot {
        warmUps++
        return state.value ?: probe.also { state.value = it }
    }

    override fun setDisplay(displayId: Int) {
        displays += displayId
    }

    override fun updateSettings(settings: CapsSettings) {
        this.settings += settings
    }

    override fun updateQuirks(table: QuirkTable) {
        quirkTables += table
    }

    /** A new probe result, as after a screen or audio change. */
    fun emit(snapshot: CapsSnapshot) {
        state.value = snapshot
    }
}

/** The capability graph for tests: a fake probe and the real policy. */
class FakeCapsGraph(
    override val capabilities: FakeDeviceCapabilities = FakeDeviceCapabilities(),
    override val policy: PlaybackCapsPolicy = DefaultPlaybackCapsPolicy,
) : CapsGraph
