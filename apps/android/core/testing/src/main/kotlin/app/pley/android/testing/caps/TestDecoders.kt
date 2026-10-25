// Android's profile and level constants are plain numbers compared with what
// decoders list, so using newer ones on older versions is safe.
@file:SuppressLint("InlinedApi")

package app.pley.android.testing.caps

import android.annotation.SuppressLint
import android.media.MediaCodecInfo.CodecProfileLevel
import app.pley.android.caps.AudioState
import app.pley.android.caps.Codecs
import app.pley.android.caps.DecoderInfo
import app.pley.android.caps.DisplayModeInfo
import app.pley.android.caps.DisplayState
import app.pley.android.caps.DolbyVision
import app.pley.android.caps.MatchContent
import app.pley.android.caps.ProfileLevel
import app.pley.android.caps.SizeRate

/** Decoders written by hand for test devices, as the probe would list them. */
object TestDecoders {
    /**
     * A video decoder that plays up to [maxWidth]×[maxHeight] at [maxFps]
     * (and at [fps1080] at 1080p when that is faster), with [profiles] at
     * [level]. Hardware unless said otherwise.
     */
    fun video(
        name: String,
        mime: String,
        profiles: List<Int>,
        level: Int,
        maxWidth: Int = 3840,
        maxHeight: Int = 2160,
        maxFps: Double = 60.0,
        fps1080: Double = maxFps,
        hardware: Boolean = true,
    ): DecoderInfo = DecoderInfo(
        name = name,
        mime = mime,
        hardware = hardware,
        vendor = hardware,
        profileLevels = profiles.map { ProfileLevel(it, level) },
        maxWidth = maxWidth,
        maxHeight = maxHeight,
        maxFps = maxOf(maxFps, fps1080),
        sizeRates = SizeRate.PROBED.filter { at ->
            at.width <= maxWidth && at.height <= maxHeight &&
                at.fps <= if (at.height == maxHeight) maxFps else maxOf(maxFps, fps1080)
        },
    )

    /** A hardware Dolby Vision decoder for profile numbers (5, 8…) at level numbers (9 is 2160p60). */
    fun dolbyVision(name: String, profiles: Map<Int, Int>): DecoderInfo {
        val constants = DolbyVision.PROFILES.entries.associate { (constant, number) -> number to constant }
        return DecoderInfo(
            name = name,
            mime = Codecs.DOLBY_VISION,
            hardware = true,
            vendor = true,
            profileLevels = profiles.map { (profile, level) -> ProfileLevel(constants.getValue(profile), 1 shl (level - 1)) },
            maxWidth = 3840,
            maxHeight = 2160,
            maxFps = 60.0,
            sizeRates = SizeRate.PROBED,
        )
    }

    fun audio(name: String, mime: String, hardware: Boolean = false, channels: Int = 8): DecoderInfo =
        DecoderInfo(name = name, mime = mime, hardware = hardware, maxChannels = channels)

    /** AOSP's software audio decoders, on every device. */
    val aospAudio: List<DecoderInfo> = listOf(
        audio("c2.android.aac.decoder", "audio/mp4a-latm"),
        audio("c2.android.mp3.decoder", "audio/mpeg", channels = 2),
        audio("c2.android.opus.decoder", "audio/opus"),
        audio("c2.android.vorbis.decoder", "audio/vorbis"),
        audio("c2.android.flac.decoder", "audio/flac"),
        audio("c2.android.raw.decoder", "audio/raw"),
    )

    /** AOSP's software video decoders (Android 10 and up), AV1 among them. */
    val aospVideo: List<DecoderInfo> = listOf(
        video("c2.android.avc.decoder", Codecs.AVC, listOf(CodecProfileLevel.AVCProfileHigh), CodecProfileLevel.AVCLevel52, 1920, 1080, hardware = false),
        video("c2.android.hevc.decoder", Codecs.HEVC, listOf(CodecProfileLevel.HEVCProfileMain), CodecProfileLevel.HEVCMainTierLevel51, 1920, 1080, 30.0, hardware = false),
        video(
            "c2.android.vp9.decoder",
            Codecs.VP9,
            listOf(CodecProfileLevel.VP9Profile0, CodecProfileLevel.VP9Profile2),
            CodecProfileLevel.VP9Level5,
            1920,
            1080,
            30.0,
            hardware = false,
        ),
        video(
            "c2.android.av1.decoder",
            Codecs.AV1,
            listOf(CodecProfileLevel.AV1ProfileMain8, CodecProfileLevel.AV1ProfileMain10),
            CodecProfileLevel.AV1Level53,
            1920,
            1080,
            60.0,
            hardware = false,
        ),
    )
}

/** Screens for test devices. */
object TestDisplays {
    /** Display.HdrCapabilities types. */
    const val DV = 1
    const val HDR10 = 2
    const val HLG = 3
    const val HDR10_PLUS = 4

    /**
     * A TV at [width]×[height] with the usual film and TV rates, showing
     * [hdr]; on Android 14 and up ([perMode]) each mode lists the types too.
     */
    fun tv(
        width: Int = 3840,
        height: Int = 2160,
        hdr: List<Int> = listOf(HDR10, HLG),
        perMode: Boolean = false,
        activeRate: Float = 60f,
        match: MatchContent = MatchContent.Unknown,
    ): DisplayState {
        val sizes = listOf(width to height) + if (height > 1080) listOf(1920 to 1080) else emptyList()
        val rates = listOf(23.976f, 24f, 25f, 29.97f, 30f, 50f, 59.94f, 60f)
        var id = 1
        val modes = sizes.flatMap { (w, h) -> rates.map { DisplayModeInfo(id++, w, h, it, hdrTypes = if (perMode) hdr else null) } }
        val active = modes.first { it.width == width && it.refreshRate == activeRate }
        return DisplayState(modes = modes, activeModeId = active.id, hdrCapabilities = hdr, matchContent = match)
    }

    /** A phone screen at 60 Hz that also runs at [alternatives] without a blank. */
    fun phone(width: Int = 1080, height: Int = 2400, hdr: List<Int> = emptyList(), alternatives: List<Float> = listOf(90f, 120f)): DisplayState =
        DisplayState(
            modes = listOf(DisplayModeInfo(1, width, height, 60f, alternativeRefreshRates = alternatives)),
            activeModeId = 1,
            hdrCapabilities = hdr,
            matchContent = MatchContent.SeamlessOnly,
        )
}

/** Audio outputs for test devices. */
object TestAudio {
    val speaker = AudioState(passthroughEncodings = emptyList(), maxChannels = 2, route = AudioState.ROUTE_SPEAKER)

    fun hdmi(vararg encodings: Int, channels: Int = 8) =
        AudioState(passthroughEncodings = encodings.toList(), maxChannels = channels, route = AudioState.ROUTE_HDMI)
}
