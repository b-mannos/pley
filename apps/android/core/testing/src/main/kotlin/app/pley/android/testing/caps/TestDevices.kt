// Android's profile and level constants are plain numbers compared with what
// decoders list, so using newer ones on older versions is safe.
@file:SuppressLint("InlinedApi")

package app.pley.android.testing.caps

import android.annotation.SuppressLint
import android.media.MediaCodecInfo.CodecProfileLevel as PL
import androidx.media3.common.C
import app.pley.android.caps.Codecs
import app.pley.android.caps.DeviceFacts
import app.pley.android.caps.ProbeInput
import app.pley.android.testing.caps.TestDecoders.aospAudio
import app.pley.android.testing.caps.TestDecoders.aospVideo
import app.pley.android.testing.caps.TestDecoders.audio
import app.pley.android.testing.caps.TestDecoders.dolbyVision
import app.pley.android.testing.caps.TestDecoders.video
import app.pley.android.testing.caps.TestDisplays.DV
import app.pley.android.testing.caps.TestDisplays.HDR10
import app.pley.android.testing.caps.TestDisplays.HDR10_PLUS
import app.pley.android.testing.caps.TestDisplays.HLG

/**
 * Devices written by hand from their published specs (TV addendum §5.4,
 * §5.6, §6.1), as the probe would read them. The two real devices probed
 * over adb are JSON fixtures in :core:caps' tests.
 */
object TestDevices {
    private const val MIB = 1L shl 20

    private fun phone(model: String, sdk: Int = 34) =
        DeviceFacts(model = model, manufacturer = "pley", sdk = sdk, fingerprint = "pley/$model", form = DeviceFacts.Form.Phone, totalRamBytes = 6144 * MIB)

    private val hevcMain10 = listOf(PL.HEVCProfileMain, PL.HEVCProfileMain10, PL.HEVCProfileMain10HDR10)
    private val avcHigh = listOf(PL.AVCProfileBaseline, PL.AVCProfileMain, PL.AVCProfileHigh)

    /** A phone with an SDR screen: hardware AVC, HEVC and VP9; AV1 only in software. */
    val sdrPhone = ProbeInput(
        device = phone("pley SDR phone"),
        decoders = listOf(
            video("c2.qti.avc.decoder", Codecs.AVC, avcHigh, PL.AVCLevel52, maxFps = 30.0, fps1080 = 120.0),
            video("c2.qti.hevc.decoder", Codecs.HEVC, hevcMain10, PL.HEVCMainTierLevel51, fps1080 = 120.0),
            video("c2.qti.vp9.decoder", Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile2), PL.VP9Level51, fps1080 = 120.0),
        ) + aospVideo + aospAudio,
        display = TestDisplays.phone(),
        audio = TestAudio.speaker,
    )

    /** The same phone with an HDR10 and HLG screen. */
    val hdr10Phone = sdrPhone.copy(
        device = phone("pley HDR10 phone"),
        display = TestDisplays.phone(hdr = listOf(HDR10, HLG)),
    )

    /** A phone with a Dolby Vision screen and decoder (profiles 5 and 8), and HDR10+. */
    val dvPhone = ProbeInput(
        device = phone("pley DV phone"),
        decoders = listOf(
            video("c2.qti.avc.decoder", Codecs.AVC, avcHigh, PL.AVCLevel52, maxFps = 30.0, fps1080 = 120.0),
            video("c2.qti.hevc.decoder", Codecs.HEVC, hevcMain10 + PL.HEVCProfileMain10HDR10Plus, PL.HEVCMainTierLevel51, fps1080 = 120.0),
            dolbyVision("c2.qti.dv.decoder", mapOf(5 to 7, 8 to 7)),
        ) + aospVideo + aospAudio,
        display = TestDisplays.phone(hdr = listOf(DV, HDR10, HLG, HDR10_PLUS)),
        audio = TestAudio.speaker,
    )

    /**
     * Fire TV Stick Lite (T0): Fire OS 7 (Android 9), 1 GB. Up to 1080p60:
     * H.264, HEVC Main 10 at level 4.1, VP9 profile 2; no AV1, no Dolby
     * Vision. Below Android 10 hardware is told by name.
     */
    val fireTvStickLite = ProbeInput(
        device = DeviceFacts(
            model = "AFTSS", manufacturer = "Amazon", sdk = 28, fingerprint = "Amazon/sheldon/sheldon:9", form = DeviceFacts.Form.Tv,
            fireTv = true, totalRamBytes = 900 * MIB, lowRam = true,
        ),
        decoders = listOf(
            video("OMX.MTK.VIDEO.DECODER.AVC", Codecs.AVC, avcHigh, PL.AVCLevel42, 1920, 1080),
            video("OMX.MTK.VIDEO.DECODER.HEVC", Codecs.HEVC, listOf(PL.HEVCProfileMain, PL.HEVCProfileMain10), PL.HEVCMainTierLevel41, 1920, 1080),
            video("OMX.MTK.VIDEO.DECODER.VP9", Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile2), PL.VP9Level41, 1920, 1080),
            video("OMX.google.h264.decoder", Codecs.AVC, avcHigh, PL.AVCLevel41, 1920, 1080, 30.0, hardware = false),
            audio("OMX.dolby.ac3.decoder", "audio/ac3", hardware = true),
            audio("OMX.dolby.eac3.decoder", "audio/eac3", hardware = true),
        ) + aospAudio,
        display = TestDisplays.tv(1920, 1080, hdr = listOf(HDR10, HLG, HDR10_PLUS)),
        audio = TestAudio.hdmi(C.ENCODING_AC3, C.ENCODING_E_AC3, C.ENCODING_E_AC3_JOC, C.ENCODING_DTS),
    )

    /**
     * Fire TV Stick 4K Max (2023, AFTKRT, T1): Fire OS 8 (Android 11), 2 GB.
     * 4K60 HEVC, VP9 and AV1; Dolby Vision profiles 5, 8, 9 and 10 but not
     * 7; Atmos, TrueHD and DTS-HD passthrough.
     */
    val fireTvStick4kMax2023 = ProbeInput(
        device = DeviceFacts(
            model = "AFTKRT", manufacturer = "Amazon", sdk = 30, fingerprint = "Amazon/kara/kara:11", form = DeviceFacts.Form.Tv,
            fireTv = true, totalRamBytes = 1950 * MIB,
        ),
        decoders = listOf(
            video("c2.amlogic.avc.decoder", Codecs.AVC, avcHigh, PL.AVCLevel52),
            video("c2.amlogic.hevc.decoder", Codecs.HEVC, hevcMain10 + PL.HEVCProfileMain10HDR10Plus, PL.HEVCMainTierLevel51),
            video("c2.amlogic.vp9.decoder", Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile2, PL.VP9Profile2HDR), PL.VP9Level51),
            video(
                "c2.amlogic.av1.decoder",
                Codecs.AV1,
                listOf(PL.AV1ProfileMain8, PL.AV1ProfileMain10, PL.AV1ProfileMain10HDR10, PL.AV1ProfileMain10HDR10Plus),
                PL.AV1Level51,
            ),
            dolbyVision("c2.amlogic.dolby.decoder", mapOf(5 to 9, 8 to 9, 9 to 5, 10 to 9)),
            audio("c2.dolby.ac3.decoder", "audio/ac3", hardware = true),
            audio("c2.dolby.eac3.decoder", "audio/eac3", hardware = true),
        ) + aospVideo + aospAudio,
        display = TestDisplays.tv(hdr = listOf(DV, HDR10, HLG, HDR10_PLUS)),
        audio = TestAudio.hdmi(
            C.ENCODING_AC3, C.ENCODING_E_AC3, C.ENCODING_E_AC3_JOC, C.ENCODING_DOLBY_TRUEHD, C.ENCODING_DTS, C.ENCODING_DTS_HD,
        ),
    )

    /**
     * NVIDIA Shield TV (2019, Android 11, 3 GB): 4K60 HEVC and VP9, no AV1;
     * Dolby Vision profiles 4, 5 and 8 listed, 7 played without being
     * listed (the quirk); HDR10 but no HDR10+; lossless passthrough.
     */
    val shield = ProbeInput(
        device = DeviceFacts(
            model = "SHIELD Android TV", manufacturer = "NVIDIA", sdk = 30, fingerprint = "NVIDIA/mdarcy/mdarcy:11", form = DeviceFacts.Form.Tv,
            totalRamBytes = 2900 * MIB,
        ),
        decoders = listOf(
            video("OMX.Nvidia.h264.decode", Codecs.AVC, avcHigh, PL.AVCLevel52, maxFps = 30.0, fps1080 = 60.0),
            video("OMX.Nvidia.h265.decode", Codecs.HEVC, hevcMain10, PL.HEVCMainTierLevel51),
            video("OMX.Nvidia.vp9.decode", Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile2), PL.VP9Level51),
            dolbyVision("OMX.Nvidia.h265.decode.dv", mapOf(4 to 9, 5 to 9, 8 to 9)),
            audio("OMX.Nvidia.eac3.decoder", "audio/eac3", hardware = true),
        ) + aospVideo + aospAudio,
        display = TestDisplays.tv(hdr = listOf(DV, HDR10, HLG)),
        audio = TestAudio.hdmi(
            C.ENCODING_AC3, C.ENCODING_E_AC3, C.ENCODING_E_AC3_JOC, C.ENCODING_DOLBY_TRUEHD, C.ENCODING_DTS, C.ENCODING_DTS_HD,
        ),
    )

    /** Chromecast with Google TV HD (T0): Android 12, 1.5 GB; up to 1080p60 HEVC, VP9 and AV1; no Dolby Vision. */
    val chromecastHd = ProbeInput(
        device = DeviceFacts(
            model = "Chromecast HD", manufacturer = "Google", sdk = 31, fingerprint = "google/boreal/boreal:12", form = DeviceFacts.Form.Tv,
            totalRamBytes = 1420 * MIB,
        ),
        decoders = listOf(
            video("c2.amlogic.avc.decoder", Codecs.AVC, avcHigh, PL.AVCLevel42, 1920, 1080),
            video("c2.amlogic.hevc.decoder", Codecs.HEVC, hevcMain10, PL.HEVCMainTierLevel41, 1920, 1080),
            video("c2.amlogic.vp9.decoder", Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile2), PL.VP9Level41, 1920, 1080),
            video("c2.amlogic.av1.decoder", Codecs.AV1, listOf(PL.AV1ProfileMain8, PL.AV1ProfileMain10), PL.AV1Level41, 1920, 1080),
        ) + aospVideo + aospAudio,
        display = TestDisplays.tv(1920, 1080, hdr = listOf(HDR10, HLG, HDR10_PLUS)),
        audio = TestAudio.hdmi(C.ENCODING_AC3, C.ENCODING_E_AC3, C.ENCODING_E_AC3_JOC),
    )

    /**
     * Google TV Streamer (Android 14, 4 GB) on a Dolby Vision TV with an
     * AVR: its real decoders (profiles 4, 5, 8, 9, 10), every mode listing
     * its HDR types, and TrueHD and DTS-HD advertised, which the quirk takes
     * away.
     */
    val googleTvStreamer = ProbeInput(
        device = DeviceFacts(
            model = "Google TV Streamer", manufacturer = "Google", sdk = 34, fingerprint = "google/kirkwood/kirkwood:14", form = DeviceFacts.Form.Tv,
            totalRamBytes = 3870 * MIB,
        ),
        decoders = listOf(
            video("c2.mtk.avc.decoder", Codecs.AVC, avcHigh + PL.AVCProfileHigh10, PL.AVCLevel52),
            video("c2.mtk.hevc.decoder", Codecs.HEVC, hevcMain10, PL.HEVCHighTierLevel51),
            video("c2.mtk.vp9.decoder", Codecs.VP9, listOf(PL.VP9Profile0, PL.VP9Profile1, PL.VP9Profile2, PL.VP9Profile2HDR), PL.VP9Level51),
            video(
                "c2.mtk.av1.decoder",
                Codecs.AV1,
                listOf(PL.AV1ProfileMain8, PL.AV1ProfileMain10, PL.AV1ProfileMain10HDR10, PL.AV1ProfileMain10HDR10Plus),
                PL.AV1Level51,
            ),
            dolbyVision("c2.mtk.dvhe.stn.decoder", mapOf(5 to 9)),
            dolbyVision("c2.mtk.dvhe.sth.decoder", mapOf(8 to 9)),
            dolbyVision("c2.mtk.dvhe.dtr.decoder", mapOf(4 to 9)),
            dolbyVision("c2.mtk.dvav.ser.decoder", mapOf(9 to 9)),
            dolbyVision("c2.mtk.dvav1.decoder", mapOf(10 to 9)),
            audio("c2.dolby.ac3.decoder", "audio/ac3", hardware = true, channels = 6),
            audio("c2.dolby.eac3.decoder", "audio/eac3", hardware = true),
        ) + aospVideo + aospAudio,
        display = TestDisplays.tv(hdr = listOf(DV, HDR10, HLG, HDR10_PLUS), perMode = true),
        audio = TestAudio.hdmi(
            C.ENCODING_AC3, C.ENCODING_E_AC3, C.ENCODING_E_AC3_JOC, C.ENCODING_DOLBY_TRUEHD, C.ENCODING_DTS, C.ENCODING_DTS_HD,
        ),
    )

    /** Every device above, by a short name. */
    val all: Map<String, ProbeInput> = linkedMapOf(
        "sdrPhone" to sdrPhone,
        "hdr10Phone" to hdr10Phone,
        "dvPhone" to dvPhone,
        "fireTvStickLite" to fireTvStickLite,
        "fireTvStick4kMax2023" to fireTvStick4kMax2023,
        "shield" to shield,
        "chromecastHd" to chromecastHd,
        "googleTvStreamer" to googleTvStreamer,
    )
}
