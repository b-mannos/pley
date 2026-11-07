package app.pley.android.model.admin

import app.pley.android.model.PleyJson
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The JSON of Settings → Transcoder, as settings.rs and admin.rs write it. */
class TranscoderAdminTest {
    @Test
    fun theTranscodersSettingsAreReadFromTheWholeSettings() {
        val json = """{"server_name":"Den","hwaccel":"vulkan","hardware_decoding":true,"tonemapping":"libplacebo",
            "max_transcodes":6,"transcode_preset":"veryfast","throttle_transcodes":false,"transcode_in_memory":true,
            "scan_interval_minutes":60,"proxy_addresses":[]}"""
        val settings = PleyJson.decodeFromString(TranscoderSettings.serializer(), json)
        assertEquals(
            TranscoderSettings(HwAccel.Vulkan, true, ToneMapping.Libplacebo, 6, "veryfast", throttleTranscodes = false, transcodeInMemory = true),
            settings,
        )
        // A choice a newer server adds reads as unknown, not as a failure.
        val newer = PleyJson.decodeFromString(TranscoderSettings.serializer(), """{"hwaccel":"metal","tonemapping":"bt2446"}""")
        assertEquals(HwAccel.Unknown, newer.hwaccel)
        assertEquals(ToneMapping.Unknown, newer.tonemapping)
    }

    @Test
    fun theSystemSaysWhatTheTranscodersFfmpegCanDo() {
        val json = """{"name":"Den","version":"0.1.6","os":"linux","arch":"x86_64","uptime_secs":5,"data_dir":"/data",
            "transcode_dir":{"in_use":"/data/transcode","next":"/fast/pley","default":"/data/transcode",
              "problem":{"folder":"/gone","reason":"not found"},"fixed":false},
            "transcoder":{"available":true,"managed":true,"url":null,"version":"0.1.6","commit":"abc","license":"AGPL-3.0-only",
              "ffmpeg_version":"7.1","encoders":["h264_vaapi","libx264"],"active_encoder":"h264_vaapi","hwaccels":["vaapi"],
              "accels":["vaapi"],"hardware_decoding":true,
              "with_accel":{"vaapi":{"hardware_decoding":true,"software":["hevc"],"tonemappers":["zscale"]},
                            "none":{"hardware_decoding":false,"software":["h264","hevc"],"tonemappers":null}},
              "tonemapping":["opencl","zscale"],"dolby_vision":false,"chromaprint":true,"throttling":true,
              "memory":{"available":true,"path":"/dev/shm","budget_bytes":2147483648},"max_jobs":8,"active_jobs":0,"error":null},
            "counts":{"libraries":2}}"""
        val system = PleyJson.decodeFromString(TranscoderSystem.serializer(), json)
        assertEquals("linux", system.os)
        val folder = system.transcodeDir!!
        assertEquals("/fast/pley", folder.chosen)
        assertEquals(TranscodeFolderProblem("/gone", "not found"), folder.problem)
        val tc = system.transcoder
        assertEquals(listOf(HwAccel.Vaapi), tc.accels)
        assertEquals(listOf("zscale"), tc.with(HwAccel.Vaapi)?.tonemappers)
        assertNull(tc.with(HwAccel.None)?.tonemappers)
        assertFalse(tc.with(HwAccel.None)!!.hardwareDecoding)
        assertNull(tc.with(HwAccel.Nvenc))
        assertEquals(2_147_483_648L, tc.memory?.budgetBytes)
        assertEquals(8, tc.maxJobs)
    }

    @Test
    fun aServerBeforeTheCapabilitiesSaysNothingOfThem() {
        val system = PleyJson.decodeFromString(TranscoderSystem.serializer(), """{"os":"windows","transcoder":{"available":false,"error":"down"}}""")
        assertNull(system.transcoder.accels)
        assertNull(system.transcoder.memory)
        assertNull(system.transcodeDir)
        assertEquals("down", system.transcoder.error)
    }

    @Test
    fun aFolderWithoutAChangeIsTheOneInUse() {
        val folder = TranscodeFolder(inUse = "/data/transcode", default = "/data/transcode")
        assertEquals("/data/transcode", folder.chosen)
        assertEquals("/gone", folder.copy(problem = TranscodeFolderProblem("/gone", "x")).chosen)
    }

    @Test
    fun aTestSaysHowItWent() {
        val result = PleyJson.decodeFromString(TranscoderTestResult.serializer(), """{"ok":true,"message":"Transcoder answered.","ms":12}""")
        assertTrue(result.ok)
        assertEquals(12L, result.ms)
    }
}
