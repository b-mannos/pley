package app.pley.android.caps

import app.pley.android.model.DynamicRange
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.MediaFile
import app.pley.android.model.MediaStream
import app.pley.android.model.StreamKind
import app.pley.android.testing.caps.TestDevices
import app.pley.android.testing.caps.capsOf
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PlaybackCapsPolicyTest {
    private val policy = DefaultPlaybackCapsPolicy

    private fun stream(
        codec: String = "hevc",
        width: Int = 3840,
        height: Int = 2160,
        fps: Double = 23.976,
        range: DynamicRange = DynamicRange.Sdr,
        depth: Int = if (range == DynamicRange.Sdr) 8 else 10,
        level: Int? = null,
        profile: String? = null,
        dv: Int? = null,
        blCompat: Int? = null,
    ) = MediaStream(
        index = 0, kind = StreamKind.Video, codec = codec, width = width, height = height, frameRate = fps, bitDepth = depth,
        dynamicRange = range, level = level, profile = profile, dvProfile = dv, dvBlCompat = blCompat, default = true,
    )

    private fun file(id: Long, video: MediaStream, bitrate: Long = 20_000_000) = MediaFile(
        id = id, path = "film-$id.mkv", container = "mkv", bitrate = bitrate, width = video.width, height = video.height,
        streams = listOf(video, MediaStream(index = 1, kind = StreamKind.Audio, codec = "eac3", channels = 6)),
    )

    private fun item(vararg files: MediaFile) = ItemDetail(id = 101, libraryId = 1, kind = ItemKind.Movie, title = "Arrival", files = files.toList())

    private fun decide(device: ProbeInput, video: MediaStream): CapsDecision = policy.decide(file(1, video), capsOf(device))

    @Test
    fun hdrTypesNeedTheirScreen() {
        val hdr10 = stream(range = DynamicRange.Hdr10)
        assertEquals(CapsDecision.ServerToneMaps, decide(TestDevices.sdrPhone, hdr10))
        assertEquals(CapsDecision.Direct, decide(TestDevices.hdr10Phone, hdr10))
        assertEquals("HDR10+ plays as HDR10", CapsDecision.Direct, decide(TestDevices.hdr10Phone, stream(range = DynamicRange.Hdr10Plus)))
        assertEquals(CapsDecision.Direct, decide(TestDevices.hdr10Phone, stream(range = DynamicRange.Hlg)))
        assertEquals("HLG needs HLG", CapsDecision.ServerToneMaps, decide(TestDevices.shield.copy(display = TestDevices.shield.display!!.copy(hdrCapabilities = listOf(2))), stream(range = DynamicRange.Hlg)))
        assertEquals(CapsDecision.Direct, decide(TestDevices.sdrPhone, stream()))
    }

    @Test
    fun dolbyVisionPlaysAsItIsOnlyWithItsProfileAndScreen() {
        val p5 = stream(range = DynamicRange.DolbyVision, dv = 5, blCompat = 0)
        val p81 = stream(range = DynamicRange.DolbyVision, dv = 8, blCompat = 1)
        val p84 = stream(range = DynamicRange.DolbyVision, dv = 8, blCompat = 4)
        val p7 = stream(range = DynamicRange.DolbyVision, dv = 7, blCompat = 6)
        val p10 = stream(codec = "av1", width = 1920, height = 1080, range = DynamicRange.DolbyVision, dv = 10, blCompat = 1)

        // Fire TV Stick 4K Max: profiles 5, 8, 9, 10 on a Dolby Vision TV, no 7.
        assertEquals(CapsDecision.Direct, decide(TestDevices.fireTvStick4kMax2023, p5))
        assertEquals(CapsDecision.Direct, decide(TestDevices.fireTvStick4kMax2023, p81))
        assertEquals(CapsDecision.Direct, decide(TestDevices.fireTvStick4kMax2023, p10))
        assertEquals("no profile 7 decoder: the HDR10 base layer", CapsDecision.RemuxBaseLayer, decide(TestDevices.fireTvStick4kMax2023, p7))
        // The Shield plays 7 through its quirk.
        assertEquals(CapsDecision.Direct, decide(TestDevices.shield, p7))

        // Galaxy Note20 Ultra: HDR10, HLG, HDR10+; no Dolby Vision.
        val note = RealDevices.galaxyNote20Ultra
        assertEquals("8.1 plays its HDR10 base layer", CapsDecision.Direct, decide(note, p81))
        assertEquals("8.4 plays its HLG base layer", CapsDecision.Direct, decide(note, p84))
        assertEquals("5 has no base layer", CapsDecision.ServerToneMaps, decide(note, p5))
        assertEquals(CapsDecision.RemuxBaseLayer, decide(note, p7))
        assertTrue("10 needs AV1 hardware", decide(note, p10) is CapsDecision.Unsupported)

        // Fire TV Stick Lite: no Dolby Vision, HEVC up to 1080p.
        val lite = TestDevices.fireTvStickLite
        assertEquals(CapsDecision.ServerToneMaps, decide(lite, p5))
        assertTrue("4K is beyond it", decide(lite, p81) is CapsDecision.Unsupported)
        assertEquals(CapsDecision.Direct, decide(lite, p81.copy(width = 1920, height = 1080)))

        // An SDR phone: everything HDR is tone-mapped.
        assertEquals(CapsDecision.ServerToneMaps, decide(TestDevices.sdrPhone, p81))
        assertEquals(CapsDecision.ServerToneMaps, decide(TestDevices.sdrPhone, p7))

        // Real Google TV Streamer on its HDR10 monitor: DV decoders, no DV screen.
        val streamer = RealDevices.googleTvStreamer
        assertEquals(CapsDecision.Direct, decide(streamer, p81))
        assertEquals(CapsDecision.ServerToneMaps, decide(streamer, p5))
        assertEquals(CapsDecision.RemuxBaseLayer, decide(streamer, p7))
    }

    @Test
    fun decoderLimitsAreKept() {
        val lite = TestDevices.fireTvStickLite
        val hevc1080 = stream(width = 1920, height = 1080, level = 120, profile = "Main 10", range = DynamicRange.Hdr10)
        assertEquals(CapsDecision.Direct, decide(lite, hevc1080))
        assertTrue("level 5.1 is above 4.1", decide(lite, hevc1080.copy(level = 153)) is CapsDecision.Unsupported)
        assertTrue("no AV1 at all", decide(lite, stream(codec = "av1", width = 1920, height = 1080)) is CapsDecision.Unsupported)
        assertTrue("Rext isn't listed", decide(lite, hevc1080.copy(profile = "Rext")) is CapsDecision.Unsupported)
        // Phones: software AV1 up to 1080p30 8-bit.
        val phone = TestDevices.sdrPhone
        assertEquals(CapsDecision.Direct, decide(phone, stream(codec = "av1", width = 1920, height = 1080, fps = 24.0)))
        assertTrue(decide(phone, stream(codec = "av1", width = 1920, height = 1080, fps = 60.0)) is CapsDecision.Unsupported)
        assertTrue(decide(phone, stream(codec = "av1", width = 1920, height = 1080, depth = 10)) is CapsDecision.Unsupported)
        assertTrue(decide(phone, stream(codec = "av1")) is CapsDecision.Unsupported)
        // 4K at 30 fps on the phone's AVC decoder, 1080p up to 60.
        assertEquals(CapsDecision.Direct, decide(phone, stream(codec = "h264", fps = 29.97)))
        assertTrue(decide(phone, stream(codec = "h264", fps = 59.94)) is CapsDecision.Unsupported)
        assertEquals(CapsDecision.Direct, decide(phone, stream(codec = "h264", width = 1920, height = 1080, fps = 59.94)))
        assertTrue(decide(phone, stream(codec = "mpeg4")) is CapsDecision.Unsupported)
    }

    @Test
    fun theBestVersionPlaysAsItIs() {
        val dv4k = file(1, stream(range = DynamicRange.DolbyVision, dv = 5, blCompat = 0), bitrate = 60_000_000)
        val hdr1080 = file(2, stream(width = 1920, height = 1080, range = DynamicRange.Hdr10, level = 123), bitrate = 10_000_000)
        val sdr720 = file(3, stream(codec = "h264", width = 1280, height = 720), bitrate = 4_000_000)
        val film = item(dv4k, hdr1080, sdr720)
        assertEquals("the 1080p stick plays the 1080p HDR10 version", 2L, policy.bestFile(film, capsOf(TestDevices.fireTvStickLite)))
        assertEquals("the 4K Max plays Dolby Vision", 1L, policy.bestFile(film, capsOf(TestDevices.fireTvStick4kMax2023)))
        assertEquals(CapsDecision.Direct, policy.decide(film, null, capsOf(TestDevices.fireTvStickLite)))
        assertEquals("a chosen file is decided as it is", CapsDecision.ServerToneMaps, policy.decide(film, 1, capsOf(TestDevices.fireTvStickLite)))
        assertEquals(null, policy.bestFile(item(), capsOf(TestDevices.sdrPhone)))
        assertTrue(policy.decide(item(), null, capsOf(TestDevices.sdrPhone)) is CapsDecision.Unsupported)
    }
}
