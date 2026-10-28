package app.pley.android.caps

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import app.pley.android.testing.caps.TestDevices
import app.pley.android.testing.caps.TestQuirks
import app.pley.android.testing.caps.capsOf
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** What needs the app's resources: Diagnostics' lines and the shipped quirk table. */
@RunWith(RobolectricTestRunner::class)
class CapsDescriptionTest {
    private val res = ApplicationProvider.getApplicationContext<Context>().resources

    @Test
    fun theShippedQuirkTableIsAnAndroidResource() {
        assertEquals(TestQuirks.shipped, QuirkTable.load(res))
    }

    @Test
    fun bothQuirkReadersAgree() {
        val json = """
            {"version": 2, "quirks": [
              {"id": "a", "why": "w", "models": ["M1", "M2"], "manufacturers": ["X"], "tv_only": true, "disabled": false,
               "add": {"dolby_vision_profiles": [7], "passthrough": ["truehd", "atmos"], "hdr_types": ["hdr10plus"], "video_codecs": ["av1"]},
               "remove": {"passthrough": ["dolby_digital", "dolby_digital_plus", "dts", "dts_hd"], "hdr_types": ["dolby_vision", "hlg", "hdr10"]},
               "flags": {"dv_hdr10_plus_hybrids_broken": true, "async_queueing": false, "tunneling": true, "dv_mode_switch": false}},
              {"id": "b", "disabled": true}
            ]}
        """.trimIndent()
        assertEquals(QuirkTable.parse(json), QuirkTableReader.read(json))
    }

    @Test
    fun diagnosticsSayWhatTheStreamerPlays() {
        val lines = CapsDescription.lines(capsOf(RealDevices.googleTvStreamer), res)
        val expected = listOf(
            "Device: Google TV Streamer by Google, Android API 34, TV, tier T2",
            "Video decoders: hardware only",
            "hevc: hardware, Main, Main 10, Main Still Picture, up to 3840×2160 at 60 fps, 10-bit, level 153",
            "av1: hardware, Main, up to 3840×2160 at 60 fps, 10-bit, level 13",
            "Decoder c2.mtk.av1.decoder for av1 (hardware)",
            "Decoder c2.android.av1.decoder for av1 (software)",
            "Screen HDR: HDR10",
            "HDR played directly: HDR10",
            "Dolby Vision profiles: 4 (level 9), 5 (level 9), 8 (level 9), 9 (level 9), 10 (level 9), unused: the screen doesn't show Dolby Vision",
            "Screen: 3840×2160 at 60 Hz, rates it switches to: 23.976, 24, 25, 29.97, 30, 50, 59.94, 60",
            "Match content frame rate: never",
            "Passthrough: none; channels: 2; output: hdmi",
            "Quirks: google-tv-streamer-dv-hdr10plus-hybrids, google-tv-lossless-passthrough",
            "Player: async queueing off, tunneling off, Dolby Vision+HDR10+ stripped of HDR10+",
        )
        expected.forEach { assertTrue("missing: $it\n${lines.joinToString("\n")}", it in lines) }
        assertTrue(lines.any { it.startsWith("Screen modes: 720×480@60 HDR10, ") })
    }

    @Test
    fun diagnosticsForAPhoneAndAFireTvStick() {
        val phone = CapsDescription.lines(capsOf(TestDevices.sdrPhone), res)
        assertTrue(phone.joinToString("\n"), "Video decoders: hardware, or software up to 1080p at 30 fps, 8-bit SDR" in phone)
        assertTrue("av1: software, Main, up to 1920×1080 at 30 fps, 8-bit, level 15" in phone)
        assertTrue("Screen HDR: none" in phone)
        assertTrue("Dolby Vision profiles: none" in phone)
        val lite = CapsDescription.lines(capsOf(TestDevices.fireTvStickLite), res)
        assertEquals("Device: AFTSS by Amazon, Android API 28, Fire TV, tier T0", lite.first())
        assertTrue(lite.joinToString("\n"), "Player: async queueing on, tunneling off, Dolby Vision+HDR10+ as they are" in lite)
        assertTrue("Match content frame rate: not said (Android 11 or older)" in lite)
    }
}
