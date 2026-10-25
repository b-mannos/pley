package app.pley.android.caps

import android.media.MediaCodecInfo.CodecProfileLevel as PL
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CodecsTest {
    @Test
    fun softwareDecodersAreToldByNameBeforeAndroid10() {
        listOf("OMX.google.h264.decoder", "c2.android.av1.decoder", "c2.google.av1.decoder", "OMX.ffmpeg.hevc.decoder", "OMX.SEC.vp9.sw.dec", "OMX.qcom.video.decoder.hevcswvdec", "libdav1d")
            .forEach { assertTrue(it, Codecs.isSoftwareByName(it)) }
        listOf("OMX.MTK.VIDEO.DECODER.HEVC", "OMX.amlogic.hevc.decoder.awesome2", "c2.mtk.av1.decoder", "OMX.Nvidia.h265.decode", "arc.hevc.decoder", "OMX.qcom.video.decoder.avc")
            .forEach { assertFalse(it, Codecs.isSoftwareByName(it)) }
    }

    @Test
    fun profilesAreNamedAsFfprobeNamesThem() {
        assertEquals(listOf("Main", "Main 10"), Codecs.profileNames(Codecs.HEVC, listOf(PL.HEVCProfileMain, PL.HEVCProfileMain10HDR10)))
        assertEquals(listOf("Main"), Codecs.profileNames(Codecs.AV1, listOf(PL.AV1ProfileMain10)))
        assertEquals(listOf("Profile 0", "Profile 2"), Codecs.profileNames(Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile2HDR)))
        assertEquals(listOf("Constrained Baseline", "Main", "High"), Codecs.profileNames(Codecs.AVC, listOf(PL.AVCProfileConstrainedBaseline, PL.AVCProfileMain, PL.AVCProfileConstrainedHigh)))
        assertEquals("MPEG-2 profiles are numbers", listOf("Main"), Codecs.profileNames(Codecs.MPEG2, listOf(PL.MPEG2ProfileMain)))
        assertEquals(listOf("Simple"), Codecs.profileNames(Codecs.MPEG2, listOf(PL.MPEG2ProfileSimple)))
    }

    @Test
    fun bitDepthAndHdr() {
        assertEquals(10, Codecs.maxBitDepth(Codecs.HEVC, listOf(PL.HEVCProfileMain10)))
        assertEquals(8, Codecs.maxBitDepth(Codecs.HEVC, listOf(PL.HEVCProfileMain, PL.HEVCProfileMainStill)))
        assertEquals(10, Codecs.maxBitDepth(Codecs.VP9, listOf(PL.VP9Profile2)))
        assertEquals(8, Codecs.maxBitDepth(Codecs.AV1, listOf(PL.AV1ProfileMain8)))
        assertTrue(Codecs.decodesHdr(Codecs.AV1, listOf(PL.AV1ProfileMain10)))
        assertFalse("H.264 High 10 isn't an HDR codec here", Codecs.decodesHdr(Codecs.AVC, listOf(PL.AVCProfileHigh10)))
        assertTrue(Codecs.decodesHdr10Plus(Codecs.HEVC, listOf(PL.HEVCProfileMain10HDR10Plus)))
        assertFalse(Codecs.decodesHdr10Plus(Codecs.HEVC, listOf(PL.HEVCProfileMain10HDR10)))
    }

    @Test
    fun levelsAreNumberedAsFfprobeNumbersThem() {
        assertEquals(123, Codecs.ffprobeLevel(Codecs.HEVC, listOf(PL.HEVCMainTierLevel41)))
        assertEquals(153, Codecs.ffprobeLevel(Codecs.HEVC, listOf(PL.HEVCHighTierLevel51, PL.HEVCMainTierLevel4)))
        assertEquals(41, Codecs.ffprobeLevel(Codecs.AVC, listOf(PL.AVCLevel41)))
        assertEquals(13, Codecs.ffprobeLevel(Codecs.AV1, listOf(PL.AV1Level51)))
        assertEquals(51, Codecs.ffprobeLevel(Codecs.VP9, listOf(PL.VP9Level51)))
        assertNull(Codecs.ffprobeLevel(Codecs.VP8, listOf(1)))
        assertNull(Codecs.ffprobeLevel(Codecs.HEVC, listOf(3)))
    }

    @Test
    fun dolbyVisionProfilesAndLevels() {
        assertEquals(4, DolbyVision.profileOf(PL.DolbyVisionProfileDvheDtr))
        assertEquals(5, DolbyVision.profileOf(PL.DolbyVisionProfileDvheStn))
        assertEquals(7, DolbyVision.profileOf(PL.DolbyVisionProfileDvheDtb))
        assertEquals(8, DolbyVision.profileOf(PL.DolbyVisionProfileDvheSt))
        assertEquals(9, DolbyVision.profileOf(PL.DolbyVisionProfileDvavSe))
        assertEquals(10, DolbyVision.profileOf(PL.DolbyVisionProfileDvav110))
        assertEquals(0x10, PL.DolbyVisionProfileDvheDtr)
        assertEquals(0x400, PL.DolbyVisionProfileDvav110)
        assertEquals(1, DolbyVision.levelOf(PL.DolbyVisionLevelHd24))
        assertEquals(9, DolbyVision.levelOf(PL.DolbyVisionLevelUhd60))
        assertEquals(12, DolbyVision.levelOf(PL.DolbyVisionLevel8k60))
        assertNull(DolbyVision.levelOf(0))
        assertNull(DolbyVision.levelOf(0x300))

        val software = DecoderInfo("c2.android.dv", Codecs.DOLBY_VISION, hardware = false, profileLevels = listOf(ProfileLevel(PL.DolbyVisionProfileDvheStn, PL.DolbyVisionLevelUhd60)))
        val hardware = DecoderInfo(
            "c2.vendor.dv",
            Codecs.DOLBY_VISION,
            hardware = true,
            profileLevels = listOf(ProfileLevel(PL.DolbyVisionProfileDvheSt, PL.DolbyVisionLevelUhd30), ProfileLevel(PL.DolbyVisionProfileDvheSt, PL.DolbyVisionLevelUhd60)),
        )
        assertEquals("hardware only, highest level", mapOf(8 to 9), DolbyVision.profiles(listOf(software, hardware)))
    }
}
