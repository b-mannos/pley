package app.pley.android.common.perf

import android.app.Activity

/**
 * Frame timing per screen (JankStats) in debug and benchmark builds, for
 * Diagnostics and the benchmarks. Not built yet: attaching does nothing, so
 * the activities can call it from the start.
 */
object PerfMonitor {
    /** Starts watching [activity]'s frames while it is shown. */
    @Suppress("UNUSED_PARAMETER")
    fun attach(activity: Activity) = Unit
}
