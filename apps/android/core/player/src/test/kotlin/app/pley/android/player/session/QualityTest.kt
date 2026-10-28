package app.pley.android.player.session

import app.pley.android.model.MediaFile
import app.pley.android.model.PlaybackMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/** The menu of docs/QUALITY.md, and the cases of apps/web/src/player/quality.test.ts. */
class QualityTest {
    private fun ids(bitrate: Long?, height: Int?) =
        Qualities.forFile(MediaFile(id = 1, path = "a", bitrate = bitrate, height = height)).map { it.id }

    @Test
    fun `finds them by id, falling back to the original`() {
        assertEquals(4_000, Qualities.byId("4m").maxBitrateKbps)
        assertEquals(720, Qualities.byId("4m").maxHeight)
        assertSame(Qualities.ORIGINAL, Qualities.byId("nope"))
        assertEquals("original", Qualities.byId(null).id)
        assertSame(Qualities.AUTOMATIC, Qualities.byId("auto"))
        // The settings say "Original" for the original.
        assertSame(Qualities.ORIGINAL, Qualities.byIdOrLabel("Original"))
        assertSame(Qualities.AUTOMATIC, Qualities.byIdOrLabel("auto"))
    }

    @Test
    fun `the menu's order is the same on every app`() {
        assertEquals(listOf("original", "auto", "30m", "15m", "20m", "12m", "8m", "4m", "2m"), Qualities.ALL.map { it.id })
        // 15 Mbps 4K under 30 Mbps 4K, then 1080p and below.
        assertEquals(listOf(2160, 2160, 1080, 1080, 1080, 720, 480), Qualities.TRANSCODES.map { it.maxHeight })
        assertSame(Qualities.byId("2m"), Qualities.LOWEST)
    }

    @Test
    fun `Automatic is a choice that never plays as itself`() {
        assertTrue(Qualities.AUTOMATIC.automatic)
        assertFalse(Qualities.AUTOMATIC.original)
        assertFalse(Qualities.ORIGINAL.automatic)
        assertEquals(null, Qualities.AUTOMATIC.maxBitrateKbps)
    }

    @Test
    fun `transcodes every quality but the original and Automatic`() {
        assertEquals(PlaybackMode.Auto, Qualities.byId("original").mode)
        assertEquals(PlaybackMode.Auto, Qualities.byId("auto").mode)
        assertEquals(PlaybackMode.Transcode, Qualities.byId("8m").mode)
    }

    @Test
    fun `offers only the qualities that make a file smaller, the original and Automatic always`() {
        assertEquals(listOf("original", "auto", "8m", "4m", "2m"), ids(9_000_000, 1080))
        assertEquals(listOf("original", "auto", "2m"), ids(3_000_000, 720))
        assertEquals(listOf("original", "auto"), ids(1_500_000, 480))
        // A 4K file at a low bitrate still gets smaller at 1080p.
        assertEquals(listOf("original", "auto", "20m", "12m", "8m", "4m", "2m"), ids(6_000_000, 2160))
        // A 4K remux: 4K at 30 Mbps too.
        assertEquals(listOf("original", "auto", "30m", "15m", "20m", "12m", "8m", "4m", "2m"), ids(60_000_000, 2160))
        // 4K at 20 Mbps: 4K at 15 is smaller, 4K at 30 isn't.
        assertEquals(listOf("original", "auto", "15m", "20m", "12m", "8m", "4m", "2m"), ids(20_000_000, 2160))
        assertEquals(Qualities.ALL.size, ids(null, null).size)
        assertEquals(Qualities.ALL.size, Qualities.forFile(null).size)
    }

    @Test
    fun `nothing chosen is Automatic, a remembered quality the file isn't offered plays the original`() {
        val file = MediaFile(id = 1, path = "a", bitrate = 5_000_000, height = 1080)
        assertEquals("4m", Qualities.choose(file, "4m").id)
        assertSame(Qualities.ORIGINAL, Qualities.choose(file, "8m"))
        assertSame(Qualities.AUTOMATIC, Qualities.choose(file, null))
        assertSame(Qualities.AUTOMATIC, Qualities.choose(file, "auto"))
        // The original chosen before stays chosen.
        assertSame(Qualities.ORIGINAL, Qualities.choose(file, "original"))
        assertSame(Qualities.ORIGINAL, Qualities.choose(file, "Original"))
    }
}
