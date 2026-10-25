package app.pley.android.caps

import kotlinx.serialization.Serializable

/**
 * Which video decoders the player may use. TVs use hardware decoders only:
 * their Cortex-A35 and A53 cores can't decode 10-bit HEVC at 1080p in
 * software. Phones use hardware first, and software only up to 1920×1080
 * at 30 frames per second, 8-bit and SDR. Debug builds can allow software
 * at any size, for tests on emulators. Audio decoders are never limited.
 */
@Serializable
data class DecoderPolicy(
    val tv: Boolean,
    val allowSoftware: Boolean = false,
) {
    /** Whether a video decoder may play a picture this big, this fast, this deep, HDR or not. */
    fun allows(hardware: Boolean, width: Int, height: Int, fps: Double, bitDepth: Int, hdr: Boolean): Boolean = when {
        hardware || allowSoftware -> true
        tv -> false
        else -> withinSoftwareLimit(width, height, fps, bitDepth, hdr)
    }

    /** Whether software decoders may be used at all (with their limit on phones). */
    val softwareAllowed: Boolean get() = allowSoftware || !tv

    /** Whether software decoders are held to [SOFTWARE_MAX_WIDTH]×[SOFTWARE_MAX_HEIGHT] at [SOFTWARE_MAX_FPS], 8-bit SDR. */
    val softwareLimited: Boolean get() = !allowSoftware && !tv

    companion object {
        const val SOFTWARE_MAX_WIDTH = 1920
        const val SOFTWARE_MAX_HEIGHT = 1080
        const val SOFTWARE_MAX_FPS = 30.0
        const val SOFTWARE_MAX_BIT_DEPTH = 8

        /** Either orientation fits; 29.97 and 30 fps count as 30. */
        fun withinSoftwareLimit(width: Int, height: Int, fps: Double, bitDepth: Int, hdr: Boolean): Boolean {
            val long = maxOf(width, height)
            val short = minOf(width, height)
            return long <= SOFTWARE_MAX_WIDTH && short <= SOFTWARE_MAX_HEIGHT &&
                fps <= SOFTWARE_MAX_FPS + 0.01 && bitDepth <= SOFTWARE_MAX_BIT_DEPTH && !hdr
        }
    }
}
