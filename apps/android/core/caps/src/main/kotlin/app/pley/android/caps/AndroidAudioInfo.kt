package app.pley.android.caps

import android.content.Context
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.AudioCapabilities

/**
 * The audio output in use, as Media3 decides passthrough: from the direct
 * playback profiles on TVs with Android 13 and up, isDirectPlaybackSupported
 * on Android 10 to 12, Amazon's surround setting on Fire TV, else the HDMI
 * plug. The route (HDMI, Bluetooth, speaker…) comes from AudioManager.
 */
@OptIn(markerClass = [UnstableApi::class])
class AndroidAudioInfo(context: Context) : AudioInfo {
    private val context = context.applicationContext
    private val audio = this.context.getSystemService(AudioManager::class.java)

    override fun read(): AudioState = from(AudioCapabilities.getCapabilities(context, AudioAttributes.DEFAULT, null))

    /** The state for Media3's [capabilities], with the route in use now. */
    fun from(capabilities: AudioCapabilities): AudioState = AudioState(
        passthroughEncodings = ENCODINGS.filter { capabilities.supportsEncoding(it) },
        maxChannels = capabilities.maxChannelCount,
        route = route(),
    )

    private fun route(): String {
        val manager = audio ?: return AudioState.ROUTE_OTHER
        val types = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val media = android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_MEDIA).build()
            manager.getAudioDevicesForAttributes(media).map { it.type }
        } else {
            manager.getDevices(AudioManager.GET_DEVICES_OUTPUTS).map { it.type }
        }
        return routeOf(types)
    }

    companion object {
        /** The encodings asked about. */
        val ENCODINGS: List<Int> = listOf(
            C.ENCODING_AC3,
            C.ENCODING_E_AC3,
            C.ENCODING_E_AC3_JOC,
            C.ENCODING_AC4,
            C.ENCODING_DOLBY_TRUEHD,
            C.ENCODING_DTS,
            C.ENCODING_DTS_HD,
            C.ENCODING_DTS_UHD_P2,
        )

        /**
         * The route of the output devices in use: Bluetooth first, then
         * headphones, USB and HDMI, else the speaker. On Android 13 and up
         * [types] are the devices media plays to; before, every output
         * connected.
         */
        fun routeOf(types: List<Int>): String = when {
            types.any { it in BLUETOOTH } -> AudioState.ROUTE_BLUETOOTH
            types.any { it in WIRED } -> AudioState.ROUTE_WIRED
            types.any { it in USB } -> AudioState.ROUTE_USB
            types.any { it in HDMI } -> AudioState.ROUTE_HDMI
            types.any { it == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER } -> AudioState.ROUTE_SPEAKER
            types.isEmpty() -> AudioState.ROUTE_SPEAKER
            else -> AudioState.ROUTE_OTHER
        }

        // AudioDeviceInfo types; the later ones are plain numbers so older SDK levels read them too.
        private val BLUETOOTH = setOf(AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, AudioDeviceInfo.TYPE_BLUETOOTH_SCO, 26, 27, 30)
        private val WIRED = setOf(AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES)
        private val USB = setOf(AudioDeviceInfo.TYPE_USB_DEVICE, AudioDeviceInfo.TYPE_USB_ACCESSORY, 22)
        private val HDMI = setOf(AudioDeviceInfo.TYPE_HDMI, AudioDeviceInfo.TYPE_HDMI_ARC, 29, AudioDeviceInfo.TYPE_LINE_DIGITAL)
    }
}
