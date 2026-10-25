package app.pley.android.player.subtitles

import androidx.media3.common.text.Cue

/** What shows from [startMs] to [endMs]. */
data class TimedCues(
    val startMs: Long,
    val endMs: Long,
    val cues: List<Cue>,
)

/**
 * The cues of one subtitle track, by time, for SubtitleView. A track still
 * being read out of its file grows: each answer holds every cue found so
 * far, and [merge] keeps whichever reaches further, so an answer that
 * arrives late never takes cues away. Lines showing stay as they were
 * (cues compare equal), so nothing flickers when more arrives.
 */
class SubtitleIndex(entries: List<TimedCues>) {
    private val entries: List<TimedCues> = entries.sortedWith(compareBy<TimedCues> { it.startMs }.thenBy { it.endMs })

    /** Cue groups in all. */
    val size: Int get() = entries.size

    /** Where the last cue ends; 0 without cues. */
    val endMs: Long get() = entries.maxOfOrNull { it.endMs } ?: 0

    /** The cues showing at [positionMs], earliest first. */
    fun at(positionMs: Long): List<Cue> {
        val out = mutableListOf<Cue>()
        for (entry in entries) {
            if (entry.startMs > positionMs) break
            if (positionMs < entry.endMs) out += entry.cues
        }
        return out
    }

    /** The next time after [positionMs] at which what shows changes; `null` when nothing changes any more. */
    fun nextChangeAfter(positionMs: Long): Long? {
        var next: Long? = null
        for (entry in entries) {
            if (entry.startMs > positionMs) {
                next = minOf(next ?: entry.startMs, entry.startMs)
                break
            }
            if (entry.endMs > positionMs) next = minOf(next ?: entry.endMs, entry.endMs)
        }
        return next
    }

    /** [more], a later answer of the same track, unless it reaches less far than this one. */
    fun merge(more: SubtitleIndex): SubtitleIndex =
        if (more.endMs > endMs || (more.endMs == endMs && more.size > size)) more else this

    companion object {
        val EMPTY = SubtitleIndex(emptyList())
    }
}
