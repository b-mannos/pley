package app.pley.android.common

/**
 * The names of the app's trace sections and counters, which the benchmarks
 * measure by (docs/ANDROID.md). One place, so a rename can't silently break
 * a budget.
 */
object TraceNames {
    /** Application.onCreate: at most 15 ms. */
    const val APP_ON_CREATE = "pley.app.onCreate"

    /** The one disk read before the first frame: active server, tokens, home snapshot. */
    const val STARTUP_READ = "pley.startup.read"

    /** A card's picture drawn on the frame the card first shows. */
    const val IMAGE_ENTER = "pley.img.enter"

    /** A card shown before its picture was ready. */
    const val IMAGE_MISS_ON_ENTER = "pley.img.missOnEnter"

    /** Opening an item page until its first frame. */
    const val NAV_ITEM = "pley.nav.item"

    /** From pressing play to the first video frame. */
    const val PLAY_TAP_TO_FIRST_FRAME = "pley.play.tapToFirstFrame"

    /** From a seek to the first video frame at the new place. */
    const val PLAY_SEEK_TO_FRAME = "pley.play.seekToFrame"
}
