package app.pley.android.designsystem.image

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlaceholderTitleTest {
    @Test
    fun aLineEndingInsideAWordBrokeIt() {
        assertTrue("Casablan/ca", brokeAWord("Casablanca", listOf(8)))
        assertFalse("at spaces", brokeAWord("Back to the Future", listOf(8, 12)))
        assertFalse("after a hyphen", brokeAWord("Spider-Man", listOf(7)))
        assertFalse("one line", brokeAWord("Alien", emptyList()))
        assertFalse("between two Chinese characters", brokeAWord("千と千尋の神隠し", listOf(4)))
    }

    @Test
    fun theWidestWordSetsHowMuchSmaller() {
        // Eight px a letter, 72 px of room: a poster's.
        val width = { word: String -> word.length * 8 }
        assertEquals(1f, wholeWordScale("12 Angry Men", 72, width))
        assertEquals(0.9f, wholeWordScale("Casablanca", 72, width), 0.001f)
        // Only the words count, not the punctuation between them.
        assertEquals(1f, wholeWordScale("Spider-Man: No Way Home", 72, width))
        assertEquals(MIN_TITLE_SCALE, wholeWordScale("Supercalifragilisticexpialidocious", 72, width))
        assertEquals(1f, wholeWordScale("", 72, width))
        assertEquals("lines break between any two", 1f, wholeWordScale("千と千尋の神隠し", 72, width))
    }
}
