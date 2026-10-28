package app.pley.android.network

import java.io.IOException
import okio.Buffer
import okio.BufferedSource
import okio.ForwardingSource
import okio.buffer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The speed test's reading (GET /api/playback/speedtest): from the first byte, for 2 s at most. */
class SpeedReaderTest {
    /** A clock that moves [stepMs] each time it is read: one read of the body. */
    private class Clock(private val stepMs: Long) {
        var ms = 0L
        fun nanos(): Long {
            ms += stepMs
            return ms * 1_000_000
        }
    }

    private fun body(bytes: Int): BufferedSource = Buffer().write(ByteArray(bytes))

    @Test
    fun `it stops 2 s after the first byte, and says what came`() {
        val clock = Clock(100)
        val sample = SpeedReader(expected = 32L * 1024 * 1024, readMs = 2_000, nanos = clock::nanos).read(body(4 * 1024 * 1024))
        // 64 KiB a read, a read every 100 ms: 21 reads to cover 2 s.
        assertEquals(21L * 64 * 1024, sample.bytes)
        assertEquals(2_000, sample.ms)
        assertFalse(sample.complete)
        assertEquals((21L * 64 * 1024 * 8 / 2_000).toInt(), sample.kbps)
    }

    @Test
    fun `all of it within the time is complete`() {
        val clock = Clock(100)
        val sample = SpeedReader(expected = 256L * 1024, readMs = 2_000, nanos = clock::nanos).read(body(256 * 1024))
        assertEquals(256L * 1024, sample.bytes)
        assertEquals(300, sample.ms)
        assertTrue(sample.complete)
    }

    @Test
    fun `a body cut short after its first byte keeps what it measured`() {
        val clock = Clock(100)
        var reads = 0
        val failing = object : ForwardingSource(body(1024 * 1024)) {
            override fun read(sink: Buffer, byteCount: Long): Long {
                if (++reads > 5) throw IOException("timeout")
                return super.read(sink, byteCount)
            }
        }.buffer()
        val sample = SpeedReader(expected = 32L * 1024 * 1024, readMs = 2_000, nanos = clock::nanos).read(failing)
        assertTrue(sample.bytes > 0)
        assertFalse(sample.complete)
    }

    @Test(expected = IOException::class)
    fun `one that fails before any byte fails`() {
        val failing = object : ForwardingSource(Buffer()) {
            override fun read(sink: Buffer, byteCount: Long): Long = throw IOException("timeout")
        }.buffer()
        SpeedReader(expected = 1024, readMs = 2_000).read(failing)
    }
}
