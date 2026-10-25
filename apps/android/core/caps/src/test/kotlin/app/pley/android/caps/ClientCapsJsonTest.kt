package app.pley.android.caps

import app.pley.android.model.PleyJson
import app.pley.android.model.caps.ClientCaps
import app.pley.android.testing.caps.TestDevices
import app.pley.android.testing.caps.capsOf
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

/**
 * The report's JSON, field for field, against snapshots in
 * src/test/resources/snapshots. When one differs, the new JSON is written
 * to build/caps-snapshots/ to compare and, if right, copy over.
 */
class ClientCapsJsonTest {
    private val pretty = kotlinx.serialization.json.Json(from = PleyJson) { prettyPrint = true }

    private fun check(name: String, report: ClientCaps) {
        val actual = PleyJson.encodeToJsonElement(ClientCaps.serializer(), report).jsonObject
        val expectedText = javaClass.classLoader!!.getResourceAsStream("snapshots/$name.json")?.bufferedReader()?.use { it.readText() }
        val expected = expectedText?.let { PleyJson.parseToJsonElement(it).jsonObject }
        if (expected != actual) {
            File("build/caps-snapshots").mkdirs()
            File("build/caps-snapshots/$name.json").writeText(pretty.encodeToString(JsonObject.serializer(), actual) + "\n")
        }
        assertEquals("snapshots/$name.json (new one in build/caps-snapshots)", expected, actual)
    }

    /** Only the five fields today's server reads (apps/server/src/playback.rs, `Caps`). */
    private fun v1(report: ClientCaps) = ClientCaps(report.containers, report.videoCodecs, report.audioCodecs, report.hdr, report.dolbyVision)

    @Test
    fun fireTvStickLite() {
        val report = capsOf(TestDevices.fireTvStickLite).report
        check("fire_tv_stick_lite_v1", v1(report))
        check("fire_tv_stick_lite_v2", report)
    }

    @Test
    fun fireTvStick4kMax2023() = check("fire_tv_stick_4k_max_2023_v2", capsOf(TestDevices.fireTvStick4kMax2023).report)

    @Test
    fun googleTvStreamer() {
        val report = capsOf(RealDevices.googleTvStreamer).report
        check("google_tv_streamer_v1", v1(report))
        check("google_tv_streamer_v2", report)
    }

    @Test
    fun galaxyNote20Ultra() {
        val report = capsOf(RealDevices.galaxyNote20Ultra).report
        check("galaxy_note20_ultra_v1", v1(report))
        check("galaxy_note20_ultra_v2", report)
    }
}
