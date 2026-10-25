package app.pley.android.caps

import android.content.Context
import android.media.MediaCodecInfo.CodecCapabilities
import android.media.MediaCodecInfo.CodecProfileLevel
import android.media.MediaFormat
import android.view.Display
import androidx.test.core.app.ApplicationProvider
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.shadows.MediaCodecInfoBuilder
import org.robolectric.shadows.ShadowDisplay
import org.robolectric.shadows.ShadowDisplayManager
import org.robolectric.shadows.ShadowMediaCodecList
import org.robolectric.util.ReflectionHelpers
import org.robolectric.util.ReflectionHelpers.ClassParameter

/** The Android sources over Robolectric's MediaCodecList and Display. */
@RunWith(RobolectricTestRunner::class)
class AndroidSourcesTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()

    @After
    fun reset() = ShadowMediaCodecList.reset()

    private fun videoFormat(mime: String, sizeRange: String = "64x64-3840x2160", rates: String = "1-60", blocksPerSecond: String = "1-1958400") =
        MediaFormat().apply {
            setString(MediaFormat.KEY_MIME, mime)
            setString("size-range", sizeRange)
            setString("block-size", "16x16")
            setString("alignment", "2x2")
            setString("frame-rate-range", rates)
            setString("blocks-per-second-range", blocksPerSecond)
        }

    private fun addDecoder(
        name: String,
        format: MediaFormat,
        profiles: List<Pair<Int, Int>>,
        hardware: Boolean,
        required: Array<String> = emptyArray(),
        encoder: Boolean = false,
    ) {
        val levels = profiles.map { (p, l) -> CodecProfileLevel().apply { profile = p; level = l } }.toTypedArray()
        ShadowMediaCodecList.addCodec(
            MediaCodecInfoBuilder.newBuilder()
                .setName(name)
                .setIsEncoder(encoder)
                .setIsHardwareAccelerated(hardware)
                .setIsSoftwareOnly(!hardware)
                .setIsVendor(hardware)
                .setCapabilities(
                    MediaCodecInfoBuilder.CodecCapabilitiesBuilder.newBuilder()
                        .setMediaFormat(format)
                        .setIsEncoder(encoder)
                        .setRequiredFeatures(required)
                        .setProfileLevels(levels)
                        .apply { if (format.getString(MediaFormat.KEY_MIME)!!.startsWith("video/")) setColorFormats(intArrayOf(CodecCapabilities.COLOR_FormatYUV420Flexible)) }
                        .build(),
                )
                .build(),
        )
    }

    @Test
    fun decodersComeFromMediaCodecList() {
        addDecoder(
            "c2.vendor.hevc.decoder",
            videoFormat(MediaFormat.MIMETYPE_VIDEO_HEVC),
            listOf(CodecProfileLevel.HEVCProfileMain10 to CodecProfileLevel.HEVCMainTierLevel51),
            hardware = true,
        )
        addDecoder(
            "c2.vendor.hevc.decoder.secure",
            videoFormat(MediaFormat.MIMETYPE_VIDEO_HEVC),
            listOf(CodecProfileLevel.HEVCProfileMain10 to CodecProfileLevel.HEVCMainTierLevel51),
            hardware = true,
            required = arrayOf(CodecCapabilities.FEATURE_SecurePlayback),
        )
        addDecoder(
            "c2.android.av1.decoder",
            videoFormat(MediaFormat.MIMETYPE_VIDEO_AV1, sizeRange = "2x2-1920x1088", rates = "1-30"),
            listOf(CodecProfileLevel.AV1ProfileMain8 to CodecProfileLevel.AV1Level41),
            hardware = false,
        )
        addDecoder(
            "c2.vendor.dv.decoder",
            videoFormat(MediaFormat.MIMETYPE_VIDEO_DOLBY_VISION),
            listOf(
                CodecProfileLevel.DolbyVisionProfileDvheStn to CodecProfileLevel.DolbyVisionLevelUhd60,
                CodecProfileLevel.DolbyVisionProfileDvheSt to CodecProfileLevel.DolbyVisionLevelUhd60,
            ),
            hardware = true,
        )
        addDecoder("c2.vendor.hevc.encoder", videoFormat(MediaFormat.MIMETYPE_VIDEO_HEVC), emptyList(), hardware = true, encoder = true)
        addDecoder(
            "c2.android.aac.decoder",
            MediaFormat().apply {
                setString(MediaFormat.KEY_MIME, MediaFormat.MIMETYPE_AUDIO_AAC)
                setString("max-channel-count", "8")
            },
            emptyList(),
            hardware = false,
        )
        addDecoder("c2.vendor.h263.decoder", videoFormat(MediaFormat.MIMETYPE_VIDEO_H263), emptyList(), hardware = true)

        val decoders = AndroidCodecInventory().decoders()
        assertEquals(
            "encoders, decoders that need protected video and unasked types are left out",
            listOf("c2.vendor.hevc.decoder", "c2.android.av1.decoder", "c2.vendor.dv.decoder", "c2.android.aac.decoder"),
            decoders.map { it.name },
        )
        val hevc = decoders.first()
        assertTrue(hevc.hardware)
        assertEquals(Codecs.HEVC, hevc.mime)
        assertEquals(listOf(ProfileLevel(CodecProfileLevel.HEVCProfileMain10, CodecProfileLevel.HEVCMainTierLevel51)), hevc.profileLevels)
        assertTrue(SizeRate(3840, 2160, 60.0) in hevc.sizeRates)
        assertTrue(SizeRate(1920, 1080, 23.976) in hevc.sizeRates)
        val av1 = decoders[1]
        assertFalse(av1.hardware)
        assertTrue(SizeRate(1920, 1080, 30.0) in av1.sizeRates)
        assertFalse("beyond its size", av1.sizeRates.any { it.width == 3840 })
        assertFalse("beyond its rate", SizeRate(1920, 1080, 50.0) in av1.sizeRates)
        assertEquals(8, decoders.last().maxChannels)
        assertEquals(mapOf(5 to 9, 8 to 9), DolbyVision.profiles(decoders))
    }

    @Test
    fun theScreenWithItsModesAndHdrTypes() {
        val shadow = shadowOf(ShadowDisplay.getDefaultDisplay())
        shadow.setDisplayHdrCapabilities(Display.DEFAULT_DISPLAY, 1000f, 500f, 0.1f, 2, 3)
        ShadowDisplayManager.setSupportedModes(Display.DEFAULT_DISPLAY, mode(7, 1920, 1080, 60f), mode(8, 1920, 1080, 50f))
        val display = AndroidDisplayInfo(context).read()!!
        assertEquals(listOf(7, 8), display.modes.map { it.id })
        assertEquals(1920, display.activeMode!!.width)
        assertEquals(listOf(2, 3), display.hdrCapabilities)
        assertTrue("Android 14 lists types per mode", display.modes.all { it.hdrTypes != null })
    }

    @Test
    fun thePlayersDisplayIsRead() {
        val id = ShadowDisplayManager.addDisplay("w960dp-h540dp")
        ShadowDisplayManager.setSupportedModes(id, mode(1, 3840, 2160, 60f), mode(2, 3840, 2160, 23.976f))
        val display = AndroidDisplayInfo(context) { id }.read()!!
        assertEquals(listOf(3840 to 2160, 3840 to 2160), display.modes.map { it.width to it.height })
        assertEquals(listOf(60f, 23.976f), display.modes.map { it.refreshRate })
        assertTrue(display.matchContent != MatchContent.Unknown)
        assertEquals("a display that's gone falls back to the default one", Display.DEFAULT_DISPLAY.let { AndroidDisplayInfo(context).read()!!.activeModeId }, AndroidDisplayInfo(context) { 99 }.read()!!.activeModeId)
    }

    /** Display.Mode's constructor is hidden. */
    private fun mode(id: Int, width: Int, height: Int, rate: Float): Display.Mode = ReflectionHelpers.callConstructor(
        Display.Mode::class.java,
        ClassParameter.from(Int::class.javaPrimitiveType, id),
        ClassParameter.from(Int::class.javaPrimitiveType, width),
        ClassParameter.from(Int::class.javaPrimitiveType, height),
        ClassParameter.from(Float::class.javaPrimitiveType, rate),
    )

    @Test
    fun routesAreReadFromTheOutputs() {
        assertEquals(AudioState.ROUTE_HDMI, AndroidAudioInfo.routeOf(listOf(android.media.AudioDeviceInfo.TYPE_HDMI)))
        assertEquals(AudioState.ROUTE_BLUETOOTH, AndroidAudioInfo.routeOf(listOf(android.media.AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, android.media.AudioDeviceInfo.TYPE_BLUETOOTH_A2DP)))
        assertEquals(AudioState.ROUTE_SPEAKER, AndroidAudioInfo.routeOf(listOf(android.media.AudioDeviceInfo.TYPE_BUILTIN_SPEAKER, android.media.AudioDeviceInfo.TYPE_BUILTIN_EARPIECE)))
        assertEquals(AudioState.ROUTE_SPEAKER, AndroidAudioInfo.routeOf(emptyList()))
        val state = AndroidAudioInfo(context).read()
        assertEquals("Robolectric's output passes nothing through", emptyList<Int>(), state.passthroughEncodings)
    }
}
