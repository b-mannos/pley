// The profile constants are plain numbers compared with what decoders list,
// so using newer ones on older versions is safe.
@file:SuppressLint("InlinedApi")

package app.pley.android.caps

import android.annotation.SuppressLint
import android.media.MediaCodecInfo.CodecProfileLevel

/**
 * Dolby Vision profiles and levels as decoders list them
 * (CodecProfileLevel.DolbyVisionProfile*, DolbyVisionLevel*), and as the
 * server numbers them (profile 5, 7, 8…; levels 1 to 13).
 */
object DolbyVision {
    /** Android's profile constant to its profile number. */
    val PROFILES: Map<Int, Int> = linkedMapOf(
        CodecProfileLevel.DolbyVisionProfileDvavPer to 0,
        CodecProfileLevel.DolbyVisionProfileDvavPen to 1,
        CodecProfileLevel.DolbyVisionProfileDvheDer to 2,
        CodecProfileLevel.DolbyVisionProfileDvheDen to 3,
        CodecProfileLevel.DolbyVisionProfileDvheDtr to 4,
        CodecProfileLevel.DolbyVisionProfileDvheStn to 5,
        CodecProfileLevel.DolbyVisionProfileDvheDth to 6,
        CodecProfileLevel.DolbyVisionProfileDvheDtb to 7,
        CodecProfileLevel.DolbyVisionProfileDvheSt to 8,
        CodecProfileLevel.DolbyVisionProfileDvavSe to 9,
        CodecProfileLevel.DolbyVisionProfileDvav110 to 10,
    )

    /** The profile number of a decoder's profile constant; `null` for one this app doesn't know. */
    fun profileOf(constant: Int): Int? = PROFILES[constant]

    /**
     * The level number of a decoder's level constant: the constants are bits,
     * HD 24 fps (0x1) being level 1 up to 8K 60 fps (0x800) being level 12.
     * `null` for anything that isn't a single known bit.
     */
    fun levelOf(constant: Int): Int? {
        if (constant <= 0 || constant and (constant - 1) != 0) return null
        val level = Integer.numberOfTrailingZeros(constant) + 1
        return level.takeIf { it <= 13 }
    }

    /**
     * Profile numbers to their highest level, over hardware Dolby Vision
     * decoders only: a software one isn't a real Dolby Vision decoder.
     */
    fun profiles(decoders: List<DecoderInfo>): Map<Int, Int?> {
        val out = sortedMapOf<Int, Int?>()
        decoders.filter { it.mime == Codecs.DOLBY_VISION && it.hardware }.forEach { decoder ->
            decoder.profileLevels.forEach { pl ->
                val profile = profileOf(pl.profile) ?: return@forEach
                val level = levelOf(pl.level)
                val known = out[profile]
                out[profile] = listOfNotNull(known, level).maxOrNull()
            }
        }
        return out
    }
}
