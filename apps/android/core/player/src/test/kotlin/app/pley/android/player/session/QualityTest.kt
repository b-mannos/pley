package app.pley.android.player.session

import app.pley.android.model.MediaFile
import app.pley.android.model.PlaybackMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

/** The cases of apps/web/src/player/quality.test.ts. */
class QualityTest {
    private fun ids(bitrate: Long?, height: Int?) =
        Qualities.forFile(MediaFile(id = 1, path = "a", bitrate = bitrate, height = height)).map { it.id }

    @Test
    fun `finds them by id, falling back to the original`() {
        assertEquals(4_000, Qualities.byId("4m").maxBitrateKbps)
        assertEquals(720, Qualities.byId("4m").maxHeight)
        assertSame(Qualities.ORIGINAL, Qualities.byId("nope"))
        assertEquals("original", Qualities.byId(null).id)
        // The settings' default says "Original".
        assertSame(Qualities.ORIGINAL, Qualities.byIdOrLabel("Original"))
    }

    @Test
    fun `lists qualities from best to worst`() {
        val bitrates = Qualities.ALL.drop(1).map { it.maxBitrateKbps ?: 0 }
        assertEquals(bitrates.sortedDescending(), bitrates)
    }

    @Test
    fun `transcodes every quality but the original`() {
        assertEquals(PlaybackMode.Auto, Qualities.byId("original").mode)
        assertEquals(PlaybackMode.Transcode, Qualities.byId("8m").mode)
    }

    @Test
    fun `offers only the qualities that make a file smaller`() {
        assertEquals(listOf("original", "8m", "4m", "2m"), ids(9_000_000, 1080))
        assertEquals(listOf("original", "2m"), ids(3_000_000, 720))
        // A 4K file at a low bitrate still gets smaller at 1080p.
        assertEquals(listOf("original", "20m", "12m", "8m", "4m", "2m"), ids(6_000_000, 2160))
        // A 4K remux: 4K at 30 Mbps too.
        assertEquals(listOf("original", "30m", "20m", "15m", "12m", "8m", "4m", "2m"), ids(60_000_000, 2160))
        // 4K at 20 Mbps: 4K at 15 is smaller, 4K at 30 isn't.
        assertEquals(listOf("original", "20m", "15m", "12m", "8m", "4m", "2m"), ids(20_000_000, 2160))
        assertEquals(Qualities.ALL.size, ids(null, null).size)
        assertEquals(Qualities.ALL.size, Qualities.forFile(null).size)
    }

    @Test
    fun `a remembered quality the file isn't offered plays the original`() {
        val file = MediaFile(id = 1, path = "a", bitrate = 5_000_000, height = 1080)
        assertEquals("4m", Qualities.choose(file, "4m").id)
        assertSame(Qualities.ORIGINAL, Qualities.choose(file, "8m"))
        assertSame(Qualities.ORIGINAL, Qualities.choose(file, null))
    }
}
