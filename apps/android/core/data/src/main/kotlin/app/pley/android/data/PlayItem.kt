package app.pley.android.data

import app.pley.android.model.ItemSummary

/** What pressing play on an item means. */
sealed interface PlayTarget {
    /** A video: [itemId] from [startMs] (0 for the start). */
    data class Video(val itemId: Long, val startMs: Long) : PlayTarget

    /** Music: the tracks of an album or artist, or one track. */
    data class Music(val itemIds: List<Long>, val startIndex: Int) : PlayTarget

    /** A show or season with nothing to play yet. */
    data object NothingToPlay : PlayTarget
}

/**
 * Pressing play, as the web does it (apps/web/src/lib/usePlayItem.ts):
 * movies, episodes and extras resume unless watched; shows and seasons
 * play their next episode; albums, artists and tracks play music.
 */
fun interface PlayItem {
    suspend fun resolve(item: ItemSummary, fromStart: Boolean): PlayTarget
}
