package app.pley.android.player.music

import app.pley.android.model.ItemSummary
import app.pley.android.player.MusicQueueState
import kotlin.random.Random

/**
 * The tracks to play and which one plays (apps/web/src/audio/AudioPlayer.tsx).
 * The queue keeps the order it was given in; shuffled, it plays a
 * Fisher-Yates permutation of it that starts with the current track, and
 * unshuffled it goes back to the order given, at the same track. The same
 * track may be in it twice: positions, not ids, say which one plays.
 */
class MusicQueue(private val random: Random = Random.Default) {
    private var given: List<ItemSummary> = emptyList()

    /** Play position → position in [given]. */
    private var order: IntArray = IntArray(0)

    /** Where in [order] the current track is. */
    var index: Int = 0
        private set

    var shuffle: Boolean = false
        private set

    val size: Int get() = order.size

    val isEmpty: Boolean get() = order.isEmpty()

    /** The tracks in the order they play. */
    val tracks: List<ItemSummary> get() = order.map { given[it] }

    val current: ItemSummary? get() = order.getOrNull(index)?.let { given[it] }

    val hasNext: Boolean get() = index + 1 < order.size

    val state: MusicQueueState get() = MusicQueueState(tracks, index, shuffle)

    /**
     * Starts over with [tracks] at [startIndex]; with [shuffle], in a
     * random order that begins with that track.
     */
    fun start(tracks: List<ItemSummary>, startIndex: Int = 0, shuffle: Boolean = false) {
        given = tracks.toList()
        val start = if (tracks.isEmpty()) 0 else startIndex.coerceIn(0, tracks.lastIndex)
        order = IntArray(given.size) { it }
        index = start
        this.shuffle = false
        if (shuffle) setShuffle(true)
    }

    /** Forgets every track. */
    fun clear() = start(emptyList())

    /**
     * Shuffles: the current track first, the rest after it in a random
     * order. Unshuffles: the order given, at the same track.
     */
    fun setShuffle(on: Boolean) {
        if (on == shuffle || order.isEmpty()) {
            shuffle = on && order.isNotEmpty()
            return
        }
        val playing = order[index]
        if (on) {
            val rest = order.filterIndexed { i, _ -> i != index }.toIntArray()
            fisherYates(rest)
            order = intArrayOf(playing) + rest
            index = 0
        } else {
            order = IntArray(given.size) { it }
            index = playing
        }
        shuffle = on
    }

    /** Moves to the next track; `false` at the end. */
    fun next(): Boolean {
        if (!hasNext) return false
        index++
        return true
    }

    /**
     * What Previous does (the web's rule): after [RESTART_AFTER_MS] of the
     * track, or on the first track, it starts again; otherwise the track
     * before plays.
     */
    fun previous(positionMs: Long): Previous {
        if (positionMs > RESTART_AFTER_MS || index == 0) return Previous.Restart
        index--
        return Previous.Moved
    }

    /** Moves to the track at [position] of the play order; `false` when there is none. */
    fun jumpTo(position: Int): Boolean {
        if (position !in order.indices) return false
        index = position
        return true
    }

    private fun fisherYates(values: IntArray) {
        for (i in values.lastIndex downTo 1) {
            val j = random.nextInt(i + 1)
            val t = values[i]
            values[i] = values[j]
            values[j] = t
        }
    }

    /** What [previous] did. */
    enum class Previous { Restart, Moved }

    companion object {
        /** Previous restarts a track that has played longer than this. */
        const val RESTART_AFTER_MS = 3_000L
    }
}
