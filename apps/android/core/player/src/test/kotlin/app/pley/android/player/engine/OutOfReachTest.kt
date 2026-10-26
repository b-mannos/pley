package app.pley.android.player.engine

import androidx.media3.common.C
import androidx.media3.common.ParserException
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.HttpDataSource
import androidx.media3.exoplayer.source.LoadEventInfo
import androidx.media3.exoplayer.source.MediaLoadData
import androidx.media3.exoplayer.upstream.LoadErrorHandlingPolicy
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import java.io.IOException
import java.net.ConnectException
import java.net.SocketTimeoutException

/** A server out of reach mid-playback: the loads' retries, and the engine's reconnecting. */
@RunWith(RobolectricTestRunner::class)
class OutOfReachTest {
    private val spec = DataSpec.Builder().setUri("http://nas:7539/api/files/1/stream").build()

    /** A failure of load [task], Media3's count for it [count], after reading [bytes] in all. */
    private fun failure(error: IOException, count: Int = 1, task: Long = 1, bytes: Long = 0) = LoadErrorHandlingPolicy.LoadErrorInfo(
        LoadEventInfo(task, spec, spec.uri, emptyMap(), 0, 0, bytes),
        MediaLoadData(C.DATA_TYPE_MEDIA),
        error,
        count,
    )

    /** What OkHttpDataSource throws when no answer comes. */
    private fun noAnswer(cause: IOException = ConnectException("Failed to connect to nas/192.168.1.20:7539")) =
        HttpDataSource.HttpDataSourceException.createForIOException(cause, spec, HttpDataSource.HttpDataSourceException.TYPE_OPEN)

    private fun http(code: Int) = HttpDataSource.InvalidResponseCodeException(code, null, null, emptyMap(), spec, ByteArray(0))

    @Test
    fun outOfReachIsNoAnswerOrAGatewaySayingSo() {
        assertTrue(MediaRetries.isOutOfReach(noAnswer()))
        assertTrue(MediaRetries.isOutOfReach(noAnswer(SocketTimeoutException("timeout"))))
        assertTrue(MediaRetries.isOutOfReach(IOException(noAnswer())))
        assertTrue(MediaRetries.isOutOfReach(http(502)))
        assertTrue(MediaRetries.isOutOfReach(http(503)))
        assertTrue(MediaRetries.isOutOfReach(http(504)))
        assertFalse(MediaRetries.isOutOfReach(http(500)))
        assertFalse(MediaRetries.isOutOfReach(http(404)))
        assertFalse(MediaRetries.isOutOfReach(IOException("reset")))
        assertFalse(MediaRetries.isOutOfReach(ParserException.createForMalformedContainer("bad box", null)))
    }

    @Test
    fun aServerOutOfReachIsRetriedForNinetySeconds() {
        var now = 0L
        val retries = MediaRetries { now }
        val delays = mutableListOf<Long>()
        var count = 0
        while (true) {
            val delay = retries.getRetryDelayMsFor(failure(noAnswer(), ++count))
            if (delay == C.TIME_UNSET) break
            delays += delay
            // Each try waits, then fails in the 3 s a home server's connection is given.
            now += delay + 3_000
        }
        // 2 s apart at most: playback goes on soon after the server is back.
        assertEquals(listOf(500L, 1_000L, 2_000L, 2_000L, 2_000L), delays.take(5))
        assertTrue("past five tries: ${delays.size}", delays.size > MediaRetries.RETRIES)
        assertTrue("gave up at $now ms", now in MediaRetries.OUT_OF_REACH_MS..MediaRetries.OUT_OF_REACH_MS + 5_000)
    }

    @Test
    fun aGatewaySayingTheServerIsAwayCountsAsOutOfReach() {
        var now = 0L
        val retries = MediaRetries { now }
        repeat(10) { i ->
            assertTrue(retries.getRetryDelayMsFor(failure(http(502), i + 1)) != C.TIME_UNSET)
            now += 5_000
        }
        now = MediaRetries.OUT_OF_REACH_MS
        assertEquals(C.TIME_UNSET, retries.getRetryDelayMsFor(failure(http(502), 11)))
    }

    @Test
    fun aLoadThatReadsOnStartsOver() {
        var now = 0L
        val retries = MediaRetries { now }
        // A file played as it is is one load: out of reach for 20 s, then it reads on…
        assertEquals(500L, retries.getRetryDelayMsFor(failure(noAnswer(), count = 1, bytes = 1_000)))
        now = 20_000
        assertEquals(1_000L, retries.getRetryDelayMsFor(failure(noAnswer(), count = 2, bytes = 1_000)))
        // …and when it fails again ten minutes later, it has 90 s again and starts at 0.5 s, though
        // Media3's count goes on from before.
        now = 620_000
        assertEquals(500L, retries.getRetryDelayMsFor(failure(noAnswer(), count = 3, bytes = 900_000_000)))
        now = 620_000 + MediaRetries.OUT_OF_REACH_MS - 1
        assertEquals(1_000L, retries.getRetryDelayMsFor(failure(noAnswer(), count = 2, bytes = 900_000_000)))
        now = 620_000 + MediaRetries.OUT_OF_REACH_MS
        assertEquals(C.TIME_UNSET, retries.getRetryDelayMsFor(failure(noAnswer(), count = 3, bytes = 900_000_000)))
    }

    @Test
    fun errorAnswersCountApartFromNoAnswer() {
        val retries = MediaRetries { 0L }
        // The server restarting: refused three times, then answering 500 while it starts.
        repeat(3) { retries.getRetryDelayMsFor(failure(noAnswer(), it + 1)) }
        val answers = (4..9).map { retries.getRetryDelayMsFor(failure(http(500), it)) }
        assertEquals(C.TIME_UNSET, answers.last())
        assertTrue(answers.dropLast(1).none { it == C.TIME_UNSET })
    }

    @Test
    fun loadsCountApartAndAreForgottenWhenTheyConclude() {
        val retries = MediaRetries { 0L }
        repeat(5) { retries.getRetryDelayMsFor(failure(http(500), it + 1, task = 1)) }
        assertEquals(500L, retries.getRetryDelayMsFor(failure(http(500), task = 2)))
        retries.onLoadTaskConcluded(1)
        assertEquals(500L, retries.getRetryDelayMsFor(failure(http(500), task = 1)))
    }

    @Test
    fun reconnectingLastsUntilMediaComesIn() {
        val watcher = ReconnectWatcher()
        watcher.failed(noAnswer(), gaveUp = false, bufferedMs = 60_000)
        assertTrue(watcher.reconnecting)
        watcher.buffered(60_000)
        assertTrue("nothing came in", watcher.reconnecting)
        // A file played as it is reads on: its buffer grows.
        watcher.buffered(60_400)
        assertFalse(watcher.reconnecting)
        // A stream's segment completes.
        watcher.failed(noAnswer(), gaveUp = false, bufferedMs = 61_000)
        watcher.completed()
        assertFalse(watcher.reconnecting)
    }

    @Test
    fun anAnswerOrGivingUpIsNotReconnecting() {
        val watcher = ReconnectWatcher()
        watcher.failed(http(500), gaveUp = false, bufferedMs = 0)
        assertFalse(watcher.reconnecting)
        watcher.failed(noAnswer(), gaveUp = true, bufferedMs = 0)
        assertFalse("Media3 gave up: the error comes next", watcher.reconnecting)
        watcher.failed(noAnswer(), gaveUp = false, bufferedMs = 0)
        watcher.failed(http(500), gaveUp = false, bufferedMs = 0)
        assertFalse("the server answers", watcher.reconnecting)
    }

    @Test
    fun aSeekMeasuresTheBufferFromWhereItLands() {
        val watcher = ReconnectWatcher()
        watcher.failed(noAnswer(), gaveUp = false, bufferedMs = 60_000)
        watcher.sought()
        // Ten minutes on: the buffer is measured from there, not taken for media coming in.
        watcher.buffered(660_000)
        assertTrue(watcher.reconnecting)
        watcher.buffered(660_500)
        assertFalse(watcher.reconnecting)
    }
}
