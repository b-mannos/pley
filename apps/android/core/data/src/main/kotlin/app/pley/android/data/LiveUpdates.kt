package app.pley.android.data

import kotlinx.coroutines.flow.StateFlow

/** A library scan the server is running. */
data class ScanState(
    val libraryId: Long?,
    /** From 0 to 1 when the server says how far it is. */
    val progress: Float?,
)

/**
 * Keeps what the screens show in step with the server while the app is in
 * the foreground and signed in: a changed library refreshes what shows it,
 * and coming back refreshes what is old.
 */
interface LiveUpdates {
    /** Scans running now; empty when none. */
    val scans: StateFlow<List<ScanState>>

    /**
     * Starts following the server, once, after the first frame: from then
     * on the event stream is open whenever the app is in the foreground and
     * signed in, and closed otherwise. Calling it again does nothing.
     */
    fun start() {}
}
