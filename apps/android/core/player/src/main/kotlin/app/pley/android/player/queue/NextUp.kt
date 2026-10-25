package app.pley.android.player.queue

import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.format.episodeLabel
import app.pley.android.model.format.isPlayableVideo
import app.pley.android.network.PleyApi
import app.pley.android.player.PlayQueue

/**
 * What plays after an item, fetched fresh so its watch state is today's
 * (apps/web/src/lib/videoQueue.ts, nextInQueue; fetchNext): the queue's
 * next video when playing through a playlist or collection, else the next
 * episode; movies and extras have none.
 */
class NextUp(private val api: PleyApi) {
    /** Whether [kind] played from [queue] can have something next, before asking. */
    fun hasNext(kind: ItemKind, queue: PlayQueue?): Boolean = queue != null || kind == ItemKind.Episode

    /** What follows [itemId] (of [kind]); `null` at the end. */
    suspend fun after(itemId: Long, kind: ItemKind, queue: PlayQueue?): ItemSummary? = when (queue) {
        is PlayQueue.Playlist -> nextIn(api.playlist(queue.playlistId).entries.map { it.item }, itemId)
        is PlayQueue.Collection -> nextIn(api.collection(queue.collectionId).items, itemId)
        null -> if (kind == ItemKind.Episode) api.next(itemId).item else null
    }

    companion object {
        /** The first video after [itemId] in [items]; `null` when it is the last, or isn't there. */
        fun nextIn(items: List<ItemSummary>, itemId: Long): ItemSummary? {
            val videos = items.filter { isPlayableVideo(it.kind) }
            val at = videos.indexOfFirst { it.id == itemId }
            return if (at < 0) null else videos.getOrNull(at + 1)
        }

        /** Where [next] starts: from the start when watched, else its resume point. */
        fun startOf(next: ItemSummary): Long = if (next.userState.watched) 0 else next.userState.positionMs

        /** "S1 E2" for an episode, `null` otherwise. */
        fun labelOf(next: ItemSummary): String? = if (next.kind == ItemKind.Episode) episodeLabel(next).ifEmpty { null } else null
    }
}
