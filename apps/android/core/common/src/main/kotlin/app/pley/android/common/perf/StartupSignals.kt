package app.pley.android.common.perf

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The moments a cold start is measured by. The splash screen stays only
 * until [startupRead] (the one disk read before the first frame), and the
 * shells report the app fully drawn once [homeDrawn] (ReportDrawnWhen), so
 * the benchmarks' time to full display is Home with its first row.
 */
object StartupSignals {
    private val read = MutableStateFlow(false)
    private val home = MutableStateFlow(false)

    /** The startup read finished (or gave up): the first frame may show. */
    val startupRead: StateFlow<Boolean> = read.asStateFlow()

    /** Home shows its first row with pictures, from the snapshot or the server. */
    val homeDrawn: StateFlow<Boolean> = home.asStateFlow()

    fun markStartupRead() {
        read.value = true
    }

    fun markHomeDrawn() {
        home.value = true
    }
}
