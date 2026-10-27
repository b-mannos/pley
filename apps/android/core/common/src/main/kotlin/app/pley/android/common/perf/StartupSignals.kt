package app.pley.android.common.perf

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The moment a cold start is measured by: the shells report the app fully
 * drawn once [homeDrawn] (ReportDrawnWhen), so the benchmarks' time to
 * full display is Home with its first row.
 */
object StartupSignals {
    private val home = MutableStateFlow(false)

    /** Home shows its first row with pictures, from the snapshot or the server. */
    val homeDrawn: StateFlow<Boolean> = home.asStateFlow()

    fun markHomeDrawn() {
        home.value = true
    }
}
