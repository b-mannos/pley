package app.pley.android.player.engine

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import app.pley.android.caps.DecoderPolicy

/**
 * The caps' decoder policy applied to Media3's decoder lists: TVs keep
 * hardware video decoders only; phones put hardware first and keep software
 * ones for pictures up to 1080p30, 8-bit and SDR; the debug switch allows
 * software at any size. Dolby Vision decoders are left out when the screen
 * can't show Dolby Vision, so Media3 falls back to the HEVC (or AVC, AV1)
 * decoder of the base layer. Audio decoders are never limited.
 */
@OptIn(markerClass = [UnstableApi::class])
class DecoderRules(
    private val policy: () -> DecoderPolicy,
    /** Whether the screen shows Dolby Vision; `null` when unknown (Media3 then decides). */
    private val dolbyVisionScreen: () -> Boolean? = { null },
) {
    /** A selector for a track of [format]: [base]'s decoders that may play it, hardware first. */
    fun selectorFor(base: MediaCodecSelector, format: Format?): MediaCodecSelector =
        MediaCodecSelector { mimeType, secure, tunneling ->
            val listed = base.getDecoderInfos(mimeType, secure, tunneling)
            if (!MimeTypes.isVideo(mimeType)) return@MediaCodecSelector listed
            val picture = format?.let(Picture::of) ?: Picture.UNKNOWN
            val allowed = listed.filter { allows(mimeType, it.hardwareAccelerated, picture) }
            hardwareFirst(allowed) { it.hardwareAccelerated }
        }

    /** The selector for renderers that don't know the format (audio, and the factory's default). */
    fun selector(base: MediaCodecSelector = MediaCodecSelector.DEFAULT): MediaCodecSelector = selectorFor(base, null)

    /** Whether a [hardware] (or software) decoder of [mimeType] may play [picture] here. */
    fun allows(mimeType: String, hardware: Boolean, picture: Picture): Boolean {
        if (!MimeTypes.isVideo(mimeType)) return true
        if (mimeType == MimeTypes.VIDEO_DOLBY_VISION && dolbyVisionScreen() == false) return false
        return policy().allows(hardware, picture.width, picture.height, picture.fps, picture.bitDepth, picture.hdr)
    }

    /**
     * A video picture as the policy weighs it. Unknown values count as
     * small: a format without its size is judged again with the size when
     * its decoder starts.
     */
    data class Picture(val width: Int, val height: Int, val fps: Double, val bitDepth: Int, val hdr: Boolean) {
        companion object {
            val UNKNOWN = Picture(0, 0, 0.0, 8, false)

            fun of(format: Format): Picture {
                val color = format.colorInfo
                val transfer = color?.colorTransfer ?: Format.NO_VALUE
                val hdr = format.sampleMimeType == MimeTypes.VIDEO_DOLBY_VISION ||
                    transfer == C.COLOR_TRANSFER_ST2084 || transfer == C.COLOR_TRANSFER_HLG
                val depth = color?.lumaBitdepth?.takeIf { it != Format.NO_VALUE } ?: bitDepthOf(format.codecs)
                return Picture(
                    width = format.width.coerceAtLeast(0),
                    height = format.height.coerceAtLeast(0),
                    fps = format.frameRate.toDouble().coerceAtLeast(0.0),
                    bitDepth = depth,
                    hdr = hdr,
                )
            }

            /**
             * The bit depth an RFC 6381 codec string tells: HEVC Main 10
             * (`hvc1.2…`), H.264 High 10 (`avc1.6E…`), VP9 profile 2 or 3
             * (`vp09.02…`), AV1 with 10 bits (`av01.0.08M.10`), Dolby Vision
             * (always 10). 8 otherwise.
             */
            fun bitDepthOf(codecs: String?): Int {
                val c = codecs?.substringBefore(',')?.trim()?.lowercase() ?: return 8
                val parts = c.split('.')
                return when (parts.firstOrNull()) {
                    "hvc1", "hev1" -> if (parts.getOrNull(1)?.trimStart('a', 'b', 'c') == "2") 10 else 8
                    "avc1", "avc3" -> if (parts.getOrNull(1)?.take(2) == "6e") 10 else 8
                    "vp09" -> if (parts.getOrNull(1) == "02" || parts.getOrNull(1) == "03") 10 else parts.getOrNull(3)?.toIntOrNull() ?: 8
                    "av01" -> parts.getOrNull(3)?.toIntOrNull() ?: 8
                    "dvh1", "dvhe", "dav1", "dva1", "dvav" -> 10
                    else -> 8
                }
            }
        }
    }

    companion object {
        /** [list] with its hardware entries first, each kind in Media3's order. */
        fun <T> hardwareFirst(list: List<T>, hardware: (T) -> Boolean): List<T> {
            val (hw, sw) = list.partition(hardware)
            return hw + sw
        }
    }
}
