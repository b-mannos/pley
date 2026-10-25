package app.pley.android.caps

import android.media.MediaCodecInfo
import android.media.MediaCodecInfo.CodecCapabilities
import android.media.MediaCodecList
import android.os.Build
import androidx.annotation.RequiresApi
import kotlin.math.ceil

/**
 * The device's decoders, from MediaCodecList. Aliases are skipped (the
 * same decoder under an old name), and so are decoders that only play
 * protected or tunneled video. Each video decoder is asked about 1080p and
 * 2160p at the usual rates, and, on Android 10 and up, which of those its
 * performance points cover.
 */
class AndroidCodecInventory : CodecInventory {
    override fun decoders(): List<DecoderInfo> =
        MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos
            .filter { !it.isEncoder && !isAlias(it) }
            .flatMap { info ->
                info.supportedTypes
                    .map { it.lowercase() }
                    .filter { it in Codecs.PROBED_MIMES }
                    .mapNotNull { describe(info, it) }
            }

    private fun isAlias(info: MediaCodecInfo): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && info.isAlias

    private fun isHardware(info: MediaCodecInfo): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) info.isHardwareAccelerated else !Codecs.isSoftwareByName(info.name)

    private fun isVendor(info: MediaCodecInfo): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) info.isVendor else !Codecs.isSoftwareByName(info.name)

    private fun describe(info: MediaCodecInfo, mime: String): DecoderInfo? {
        val caps = try {
            info.getCapabilitiesForType(mime)
        } catch (_: IllegalArgumentException) {
            return null
        }
        if (caps.isFeatureRequired(CodecCapabilities.FEATURE_SecurePlayback) ||
            caps.isFeatureRequired(CodecCapabilities.FEATURE_TunneledPlayback)
        ) {
            return null
        }
        val levels = caps.profileLevels.orEmpty().map { ProfileLevel(it.profile, it.level) }
        val video = caps.videoCapabilities
        return DecoderInfo(
            name = info.name,
            mime = mime,
            hardware = isHardware(info),
            vendor = isVendor(info),
            profileLevels = levels,
            maxWidth = video?.supportedWidths?.upper,
            maxHeight = video?.supportedHeights?.upper,
            maxFps = video?.supportedFrameRates?.upper?.toDouble(),
            sizeRates = video?.let { v -> SizeRate.PROBED.filter { sizeAndRate(v, it) } }.orEmpty(),
            performancePoints = if (video != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) performancePoints(video) else emptyList(),
            maxChannels = caps.audioCapabilities?.maxInputChannelCount,
        )
    }

    private fun sizeAndRate(video: MediaCodecInfo.VideoCapabilities, at: SizeRate): Boolean = try {
        video.areSizeAndRateSupported(at.width, at.height, at.fps)
    } catch (_: IllegalArgumentException) {
        false
    }

    @RequiresApi(Build.VERSION_CODES.Q)
    private fun performancePoints(video: MediaCodecInfo.VideoCapabilities): List<SizeRate> {
        val points = video.supportedPerformancePoints.orEmpty()
        if (points.isEmpty()) return emptyList()
        return SizeRate.PROBED.filter { at ->
            val wanted = MediaCodecInfo.VideoCapabilities.PerformancePoint(at.width, at.height, ceil(at.fps).toInt())
            points.any { it.covers(wanted) }
        }
    }
}
