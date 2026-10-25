package app.pley.android.caps

import app.pley.android.model.PleyJson

/**
 * Devices probed for real over adb, read-only (getprop, dumpsys
 * media.player, dumpsys display, dumpsys media.audio_policy, settings get):
 * their decoders, screen modes and HDR types as the system lists them. The
 * dumps became fixtures through scratchpad tooling that emulates
 * areSizeAndRateSupported from each decoder's advertised limits.
 */
object RealDevices {
    /**
     * Google TV Streamer, Android 14, on a Samsung C49RG9x monitor: HDR10
     * only, stereo PCM over HDMI, match content frame rate set to never.
     */
    val googleTvStreamer: ProbeInput by lazy { load("google_tv_streamer.json") }

    /** Galaxy Note20 Ultra (SM-N986W), Android 13: HDR10, HLG and HDR10+ screen; no Dolby Vision; AV1 in software only. */
    val galaxyNote20Ultra: ProbeInput by lazy { load("galaxy_note20_ultra.json") }

    private fun load(name: String): ProbeInput {
        val text = RealDevices::class.java.classLoader!!.getResourceAsStream("devices/$name")!!
            .bufferedReader().use { it.readText() }
        return PleyJson.decodeFromString(ProbeInput.serializer(), text)
    }
}
