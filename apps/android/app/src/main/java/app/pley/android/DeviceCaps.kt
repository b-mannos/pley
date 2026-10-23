package app.pley.android

import android.content.Context
import android.hardware.display.DisplayManager
import android.media.MediaCodecList
import android.os.Build
import android.view.Display

/** What this device decodes, asked of its codecs and its screen. */
object DeviceCaps {
    /** The server's codec names for the MIME types decoders announce. */
    private val VIDEO = mapOf(
        "video/avc" to "h264",
        "video/hevc" to "hevc",
        "video/x-vnd.on2.vp9" to "vp9",
        "video/x-vnd.on2.vp8" to "vp8",
        "video/av01" to "av1",
        "video/mpeg2" to "mpeg2video",
        "video/mp4v-es" to "mpeg4",
    )
    private val AUDIO = mapOf(
        "audio/mp4a-latm" to listOf("aac"),
        "audio/mpeg" to listOf("mp3"),
        "audio/opus" to listOf("opus"),
        "audio/vorbis" to listOf("vorbis"),
        "audio/flac" to listOf("flac"),
        "audio/ac3" to listOf("ac3"),
        "audio/eac3" to listOf("eac3"),
        "audio/raw" to listOf("pcm_s16le", "pcm_s24le"),
        "audio/alac" to listOf("alac"),
    )

    /** Containers Media3 reads. */
    private val CONTAINERS = listOf("mkv", "mp4", "m4v", "mov", "webm", "ts", "avi")

    fun detect(context: Context): Caps {
        val decoders = MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos.filter { !it.isEncoder }
        val types = decoders.flatMap { it.supportedTypes.map { type -> type.lowercase() } }.toSet()
        val display = context.getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY)
        val hdrTypes: Set<Int> = when {
            display == null -> emptySet()
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE -> display.mode.supportedHdrTypes.toSet()
            else -> @Suppress("DEPRECATION") display.hdrCapabilities?.supportedHdrTypes?.toSet().orEmpty()
        }
        return Caps(
            containers = CONTAINERS,
            videoCodecs = VIDEO.filterKeys { it in types }.values.toList(),
            audioCodecs = AUDIO.filterKeys { it in types }.values.flatten(),
            hdr = Display.HdrCapabilities.HDR_TYPE_HDR10 in hdrTypes || Display.HdrCapabilities.HDR_TYPE_HLG in hdrTypes,
            dolbyVision = "video/dolby-vision" in types && Display.HdrCapabilities.HDR_TYPE_DOLBY_VISION in hdrTypes,
        )
    }
}
