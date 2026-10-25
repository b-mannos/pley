package app.pley.android.caps

import app.pley.android.testing.caps.TestQuirks
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class QuirkTableTest {
    private fun tv(model: String, manufacturer: String = "") =
        DeviceFacts(model = model, manufacturer = manufacturer, sdk = 34, form = DeviceFacts.Form.Tv)

    @Test
    fun theShippedTableReads() {
        val table = TestQuirks.shipped
        assertEquals(1, table.version)
        assertEquals(
            listOf("shield-dv-p7", "fire-tv-dv-hdr10plus-hybrids", "google-tv-streamer-dv-hdr10plus-hybrids", "tcl-tv-dv-hdr10plus-hybrids", "google-tv-lossless-passthrough"),
            table.quirks.map { it.id },
        )
        assertTrue("every quirk says why", table.quirks.all { it.why.isNotBlank() })
        assertTrue("no tunneling allowlist yet", table.quirks.none { it.flags.tunneling == true })
        assertTrue("no async queueing kill switch yet", table.quirks.none { it.flags.asyncQueueing == false })
    }

    @Test
    fun quirksMatchByModelOrMaker() {
        val table = TestQuirks.shipped
        assertEquals(listOf("shield-dv-p7"), table.matching(tv("SHIELD Android TV")).map { it.id })
        assertEquals("case doesn't matter", listOf("shield-dv-p7"), table.matching(tv("shield android tv")).map { it.id })
        listOf("AFTMM", "AFTKA", "AFTKM", "AFTKRT", "AFTMA08C15").forEach {
            assertTrue(it, table.matching(tv(it)).single().flags.dvHdr10PlusHybridsBroken == true)
        }
        assertEquals("the 1080p sticks have no Dolby Vision to break", emptyList<Quirk>(), table.matching(tv("AFTSS")))
        assertEquals(listOf("tcl-tv-dv-hdr10plus-hybrids"), table.matching(tv("Smart TV Pro", "TCL")).map { it.id })
        assertEquals("a TCL phone isn't a TCL television", emptyList<Quirk>(), table.matching(DeviceFacts("T Phone", "TCL", 34, form = DeviceFacts.Form.Phone)))
        assertEquals(listOf("google-tv-lossless-passthrough"), table.matching(tv("Chromecast")).map { it.id })
        assertEquals("the HD has no lossless passthrough to remove", emptyList<Quirk>(), table.matching(tv("Chromecast HD")))
        val passthrough = table.quirks.single { it.id == "google-tv-lossless-passthrough" }.remove.passthrough
        assertEquals(listOf(PassthroughFormat.TrueHd, PassthroughFormat.DtsHd), passthrough)
    }

    @Test
    fun aServerTableWinsQuirkByQuirk() {
        val server = QuirkTable.parse(
            """
            {"version": 3, "quirks": [
              {"id": "shield-dv-p7", "models": ["SHIELD Android TV"], "add": {"dolby_vision_profiles": [7, 4]}},
              {"id": "tcl-tv-dv-hdr10plus-hybrids", "disabled": true},
              {"id": "aftkrt-tunnel", "models": ["AFTKRT"], "flags": {"tunneling": true, "async_queueing": false, "dv_mode_switch": true},
               "unknown_field": 1}
            ]}
            """.trimIndent(),
        )
        val table = TestQuirks.shipped.overriddenBy(server)
        assertEquals(3, table.version)
        assertEquals(listOf(7, 4), table.matching(tv("SHIELD Android TV")).single().add.dolbyVisionProfiles)
        assertEquals(emptyList<Quirk>(), table.matching(tv("X", "TCL")))
        val aftkrt = table.matching(tv("AFTKRT")).map { it.id }
        assertEquals(listOf("fire-tv-dv-hdr10plus-hybrids", "aftkrt-tunnel"), aftkrt)
        val flags = table.matching(tv("AFTKRT")).last().flags
        assertEquals(QuirkFlags(asyncQueueing = false, tunneling = true, dvModeSwitch = true), flags)
    }

    @Test
    fun anUnreadableServerTableIsIgnored() {
        assertNull(QuirkTable.parseOrNull("not json"))
        assertNull(QuirkTable.parseOrNull("""{"quirks": [{"why": "no id"}]}"""))
        assertNull(QuirkTable.parseOrNull("""{"quirks": [{"id": "x", "remove": {"passthrough": ["mp3"]}}]}"""))
        assertFalse(QuirkTable.parseOrNull("""{"quirks": []}""")!!.quirks.any())
    }
}
