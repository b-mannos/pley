package app.pley.android.player.session

/**
 * Whether the video decoder keeps up: it struggles when it drops over 1 %
 * of its frames in 10 s of playback, or 10 frames in a row. Fed the
 * engine's counts every few seconds while playing.
 */
class StruggleMeter(
    private val windowMs: Long = WINDOW_MS,
    private val maxShare: Double = MAX_SHARE,
    private val maxRun: Int = MAX_RUN,
) {
    private val samples = ArrayDeque<Pair<Long, FrameCounts>>()

    /** Takes the counts at [atMs] (ms of playback); true when the decoder struggles. */
    fun sample(atMs: Long, counts: FrameCounts): Boolean {
        val last = samples.lastOrNull()?.second
        // The counts start over with each decoder.
        if (last != null && (counts.rendered < last.rendered || counts.dropped < last.dropped)) samples.clear()
        val baseline = samples.firstOrNull()?.second
        samples.addLast(atMs to counts)
        if (baseline != null && counts.maxConsecutiveDropped >= maxRun && counts.maxConsecutiveDropped > baseline.maxConsecutiveDropped) {
            return true
        }
        while (samples.size > 2 && samples[1].first <= atMs - windowMs) samples.removeFirst()
        val (fromMs, from) = samples.first()
        if (atMs - fromMs < windowMs) return false
        val dropped = counts.dropped - from.dropped
        val shown = counts.rendered - from.rendered
        val total = dropped + shown
        return total > 0 && dropped.toDouble() / total > maxShare
    }

    /** A new decoder, a seek or a pause: the next window starts afresh. */
    fun reset() = samples.clear()

    companion object {
        const val WINDOW_MS = 10_000L
        const val MAX_SHARE = 0.01
        const val MAX_RUN = 10
    }
}
