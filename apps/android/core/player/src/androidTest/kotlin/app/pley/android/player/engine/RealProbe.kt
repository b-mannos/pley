package app.pley.android.player.engine

import android.content.Context
import app.pley.android.caps.AndroidAudioInfo
import app.pley.android.caps.AndroidCodecInventory
import app.pley.android.caps.AndroidDisplayInfo
import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.ProbingDeviceCapabilities
import app.pley.android.caps.QuirkTable
import app.pley.android.common.DeviceProfile
import kotlinx.coroutines.runBlocking

/**
 * This device's capabilities, probed for real but without watching for
 * changes: ProbingDeviceCapabilities.create's change source registers
 * Media3's AudioCapabilitiesReceiver off the main thread, which throws on
 * Android 13 devices with a spatializer (reported to core/caps).
 */
fun realProbe(context: Context, profile: DeviceProfile): CapsSnapshot = runBlocking {
    ProbingDeviceCapabilities(
        device = { ProbingDeviceCapabilities.deviceFacts(context, profile) },
        codecs = AndroidCodecInventory(),
        display = AndroidDisplayInfo(context),
        audio = AndroidAudioInfo(context),
        builtInQuirks = { QuirkTable.load(context.resources) },
    ).warmUp()
}
