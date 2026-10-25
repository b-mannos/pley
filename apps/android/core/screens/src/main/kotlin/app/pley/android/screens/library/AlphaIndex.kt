package app.pley.android.screens.library

import app.pley.android.model.ItemSummary
import java.text.Normalizer

/**
 * The A to Z of a listing sorted by title (the web's jump bar,
 * apps/web/src/components/ItemGrid.tsx): for "#" and each letter, where its
 * first title is. Titles go under the first letter of their sort title
 * (else title), accents taken off ("Élan" under E); anything else, digits
 * and other alphabets, under "#". Letters without titles are kept, so
 * screens can show them dimmed.
 */
class AlphaIndex private constructor(
    /** For each of [LETTERS], the position of its first title, or -1. */
    private val firsts: IntArray,
    /** Sections in listing order: positions and their letters, for [letterAt]. */
    private val starts: IntArray,
    private val startLetters: IntArray,
) {
    /** Whether [letter] (an index into [LETTERS]) has titles. */
    fun has(letter: Int): Boolean = firsts.getOrElse(letter) { -1 } >= 0

    /** Where [letter]'s first title is; -1 without one. */
    fun positionOf(letter: Int): Int = firsts.getOrElse(letter) { -1 }

    /**
     * Where to go for [letter]: its first title, else the next letter's
     * that has one, else the last letter's before it.
     */
    fun jumpTarget(letter: Int): Int {
        for (l in letter until LETTERS.size) if (firsts[l] >= 0) return firsts[l]
        for (l in letter downTo 0) if (firsts[l] >= 0) return firsts[l]
        return 0
    }

    /** The letter (an index into [LETTERS]) of the section [position] is in. */
    fun letterAt(position: Int): Int {
        if (starts.isEmpty()) return 0
        var lo = 0
        var hi = starts.size - 1
        if (position < starts[0]) return startLetters[0]
        while (lo < hi) {
            val mid = (lo + hi + 1) / 2
            if (starts[mid] <= position) lo = mid else hi = mid - 1
        }
        return startLetters[lo]
    }

    /** The letters that have titles. */
    val present: List<Int> get() = LETTERS.indices.filter(::has)

    override fun equals(other: Any?): Boolean = other is AlphaIndex && firsts.contentEquals(other.firsts)

    override fun hashCode(): Int = firsts.contentHashCode()

    companion object {
        /** "#" then A to Z. */
        val LETTERS: List<String> = listOf("#") + ('A'..'Z').map { it.toString() }

        /** Listings this short don't need one (the web's 30). */
        const val MIN_ITEMS = 30

        /** [items]' index, in their order. */
        fun of(items: List<ItemSummary>): AlphaIndex {
            val firsts = IntArray(LETTERS.size) { -1 }
            val starts = ArrayList<Int>()
            val letters = ArrayList<Int>()
            var previous = -1
            items.forEachIndexed { position, item ->
                val letter = letterOf(item.sortTitle.ifBlank { item.title })
                if (firsts[letter] < 0) firsts[letter] = position
                if (letter != previous) {
                    starts += position
                    letters += letter
                    previous = letter
                }
            }
            return AlphaIndex(firsts, starts.toIntArray(), letters.toIntArray())
        }

        /** The letter [title] goes under: 0 for "#", 1 to 26 for A to Z. */
        fun letterOf(title: String): Int {
            val first = title.trim().firstOrNull() ?: return 0
            val plain = Normalizer.normalize(first.uppercaseChar().toString(), Normalizer.Form.NFD)
                .filterNot { Character.getType(it) == Character.NON_SPACING_MARK.toInt() }
                .firstOrNull() ?: return 0
            return if (plain in 'A'..'Z') plain - 'A' + 1 else 0
        }
    }
}
