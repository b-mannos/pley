package app.pley.android.network

import app.pley.android.model.SpeedSample
import java.io.IOException
import okio.Buffer
import okio.BufferedSource

/**
 * Reads a speed test's body as it comes (GET /api/playback/speedtest):
 * timed from its first byte, stopped [readMs] after it or when the
 * [expected] bytes all came. A read that fails after the first byte (the
 * call's time ran out) ends the test with what it measured; one that fails
 * before says nothing came.
 */
internal class SpeedReader(
    private val expected: Long,
    private val readMs: Long,
    private val nanos: () -> Long = System::nanoTime,
) {
    fun read(source: BufferedSource): SpeedSample {
        val buffer = Buffer()
        var startedAt = -1L
        var lastAt = 0L
        var total = 0L
        while (true) {
            val n = try {
                source.read(buffer, CHUNK)
            } catch (e: IOException) {
                if (startedAt < 0) throw e
                break
            }
            if (n < 0) break
            buffer.clear()
            lastAt = nanos()
            if (startedAt < 0) startedAt = lastAt
            total += n
            if (total >= expected || lastAt - startedAt >= readMs * NS_PER_MS) break
        }
        if (startedAt < 0) return SpeedSample(0, 0, complete = false)
        // At least a millisecond: a body that came in one read is fast, not infinitely so.
        val ms = ((lastAt - startedAt) / NS_PER_MS).coerceAtLeast(1)
        return SpeedSample(bytes = total, ms = ms, complete = total >= expected && ms < readMs)
    }

    private companion object {
        const val CHUNK = 64L * 1024
        const val NS_PER_MS = 1_000_000L
    }
}
