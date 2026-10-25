package app.pley.android.designsystem.image

import android.os.Build
import androidx.tracing.Trace
import app.pley.android.common.TraceNames
import java.util.concurrent.atomic.AtomicLong

/**
 * The image budget's counters (performance_plan.md, "Images"): every tile's
 * first drawn frame counts in [TraceNames.IMAGE_ENTER], and those drawn
 * before their picture was ready also in [TraceNames.IMAGE_MISS_ON_ENTER].
 * At least 98 % must have it during steady flings with a warm disk cache.
 * Counted always (two atomic adds); written to the trace only while
 * tracing runs.
 */
object ArtworkTrace {
    private val entered = AtomicLong()
    private val missed = AtomicLong()

    /** Tiles drawn for the first time so far. */
    val enteredCount: Long get() = entered.get()

    /** Of those, tiles drawn without their picture. */
    val missedCount: Long get() = missed.get()

    /** A tile's first frame, [loaded] or not. */
    fun entered(loaded: Boolean) {
        val enter = entered.incrementAndGet()
        val miss = if (loaded) missed.get() else missed.incrementAndGet()
        // Counters reach the trace from Android 10; Fire OS 7 still counts them.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && Trace.isEnabled()) {
            Trace.setCounter(TraceNames.IMAGE_ENTER, enter)
            if (!loaded) Trace.setCounter(TraceNames.IMAGE_MISS_ON_ENTER, miss)
        }
    }
}
