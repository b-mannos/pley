package app.pley.android.screens.library

import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The A to Z of a listing by title, as the web's jump bar works it out. */
class AlphaIndexTest {
    private fun item(id: Long, title: String, sortTitle: String = "") = ItemSummary(id, 1, ItemKind.Movie, title, sortTitle)

    private fun letter(l: String) = AlphaIndex.LETTERS.indexOf(l)

    @Test
    fun lettersComeFromTheSortTitleWithoutAccents() {
        assertEquals(letter("E"), AlphaIndex.letterOf("Élan"))
        assertEquals(letter("O"), AlphaIndex.letterOf("  Öresund"))
        assertEquals(letter("A"), AlphaIndex.letterOf("ångström"))
        assertEquals(letter("C"), AlphaIndex.letterOf("Ça ira"))
        assertEquals(letter("#"), AlphaIndex.letterOf("2001: A Space Odyssey"))
        assertEquals(letter("#"), AlphaIndex.letterOf("Ωmega"))
        assertEquals(letter("#"), AlphaIndex.letterOf("!Women Art Revolution"))
        assertEquals(letter("#"), AlphaIndex.letterOf(""))
        // "The Matrix" sorts as "Matrix".
        val index = AlphaIndex.of(listOf(item(1, "The Matrix", "Matrix")))
        assertTrue(index.has(letter("M")))
        assertFalse(index.has(letter("T")))
    }

    @Test
    fun eachLetterKnowsItsFirstTitle() {
        val items = listOf(
            item(1, "12 Monkeys"),
            item(2, "Alien"),
            item(3, "Aliens"),
            item(4, "Élite Squad"),
            item(5, "Heat"),
            item(6, "Zodiac"),
        )
        val index = AlphaIndex.of(items)
        assertEquals(0, index.positionOf(letter("#")))
        assertEquals(1, index.positionOf(letter("A")))
        assertEquals(3, index.positionOf(letter("E")))
        assertEquals(5, index.positionOf(letter("Z")))
        // Empty letters are kept, to be shown dimmed.
        assertFalse(index.has(letter("B")))
        assertEquals(-1, index.positionOf(letter("B")))
        assertEquals(listOf("#", "A", "E", "H", "Z"), index.present.map { AlphaIndex.LETTERS[it] })
    }

    @Test
    fun anEmptyLetterJumpsToTheNextThatHasTitles() {
        val index = AlphaIndex.of(listOf(item(1, "Alien"), item(2, "Heat"), item(3, "Up")))
        assertEquals(1, index.jumpTarget(letter("B")))
        assertEquals(2, index.jumpTarget(letter("U")))
        // Past the last: the last letter's.
        assertEquals(2, index.jumpTarget(letter("Z")))
    }

    @Test
    fun aPositionIsInItsLettersSection() {
        val index = AlphaIndex.of(listOf(item(1, "#1"), item(2, "Alien"), item(3, "Aliens"), item(4, "Heat"), item(5, "Hero")))
        assertEquals(letter("#"), index.letterAt(0))
        assertEquals(letter("A"), index.letterAt(1))
        assertEquals(letter("A"), index.letterAt(2))
        assertEquals(letter("H"), index.letterAt(4))
        assertEquals(letter("H"), index.letterAt(400))
    }
}
