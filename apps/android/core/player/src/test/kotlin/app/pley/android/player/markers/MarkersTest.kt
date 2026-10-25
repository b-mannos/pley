package app.pley.android.player.markers

import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemKind
import app.pley.android.model.Marker
import app.pley.android.model.MarkerKind
import app.pley.android.model.MarkerSource
import app.pley.android.model.MediaFile
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

/** The cases of apps/web/src/player/SkipButton.test.tsx. */
class MarkersTest {
    private fun marker(kind: MarkerKind, start: Long, end: Long, file: Long = 1) = Marker(file, kind, start, end, MarkerSource.Chapter)

    private val intro = marker(MarkerKind.Intro, 10_000, 70_000)
    private val credits = marker(MarkerKind.Credits, 1_200_000, 1_300_000)

    @Test
    fun `takes the markers of the file being played, else the first file`() {
        val other = marker(MarkerKind.Intro, 0, 5_000, file = 2)
        val item = ItemDetail(
            id = 1, libraryId = 1, kind = ItemKind.Movie, title = "A",
            files = listOf(MediaFile(id = 1, path = "a.mkv"), MediaFile(id = 2, path = "b.mkv")),
            markers = listOf(intro, other),
        )
        assertEquals(listOf(other), Markers.forFile(item, 2))
        assertEquals(listOf(intro), Markers.forFile(item, null))
        assertEquals(emptyList<Marker>(), Markers.forFile(item.copy(markers = emptyList()), 1))
    }

    @Test
    fun `finds the segment the playhead is in, until a second before it ends`() {
        val markers = listOf(intro, credits)
        assertNull(Markers.active(markers, 9_900))
        assertSame(intro, Markers.active(markers, 10_000))
        assertSame(intro, Markers.active(markers, 68_900))
        assertNull(Markers.active(markers, 69_000))
        assertSame(credits, Markers.active(markers, 1_250_000))
    }

    @Test
    fun `lands just before an intro ends, so its last moment plays, and at the end of credits`() {
        assertEquals(69_250L, Markers.skipTarget(intro))
        assertEquals(1_300_000L, Markers.skipTarget(credits))
    }

    @Test
    fun `stays inside a very short intro`() {
        assertEquals(500L, Markers.skipTarget(marker(MarkerKind.Intro, 0, 1_000)))
    }

    @Test
    fun `announces the credits once, even when seeking back into them`() {
        val gate = CreditsGate()
        assertNull(gate.entered(null))
        assertNull(gate.entered(intro))
        assertSame(credits, gate.entered(credits))
        assertNull(gate.entered(credits))
        // Other credits (another file's, or another segment) are announced.
        val other = marker(MarkerKind.Credits, 1_200_000, 1_300_000, file = 2)
        assertSame(other, gate.entered(other))
        gate.reset()
        assertSame(credits, gate.entered(credits))
    }
}
