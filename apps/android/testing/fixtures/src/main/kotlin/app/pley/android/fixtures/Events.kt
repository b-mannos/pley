package app.pley.android.fixtures

import app.pley.android.model.PleyJson
import app.pley.android.model.ScanPhase
import app.pley.android.model.ServerEvent
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArraySet
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.LinkedBlockingQueue
import mockwebserver3.MockResponse
import mockwebserver3.MockResponseBody
import okio.BufferedSink

/**
 * GET /api/events: the server's events as Server-Sent Events, to every
 * open stream. Each stream gets a keep-alive comment every
 * [FixtureServer.Options.keepAliveMs] (the server's 15 s) and ends when its
 * access token does, as the server's. Every
 * [FixtureServer.Options.scanEveryMs] (30 s) one library is "scanned":
 * scan_started, its progress, scan_finished and library_changed. Writes
 * add their own (library_changed, sessions_changed), and tests can send
 * any with [FixtureServer.emit].
 */
internal class EventHub(private val state: State) {
    private val streams = CopyOnWriteArraySet<LinkedBlockingQueue<String>>()

    /** The profile each stream is for. */
    private val owners = ConcurrentHashMap<LinkedBlockingQueue<String>, Long>()
    private var scheduler: ScheduledExecutorService? = null
    private var scans = 0

    fun start() {
        val every = state.options.scanEveryMs
        if (every <= 0) return
        scheduler = Executors.newSingleThreadScheduledExecutor { task ->
            Thread(task, "FixtureServer scans").apply { isDaemon = true }
        }.also { it.scheduleWithFixedDelay(::scan, every, every, TimeUnit.MILLISECONDS) }
    }

    /** Stops the scans and ends every stream. */
    fun stop() {
        scheduler?.shutdownNow()
        scheduler = null
        drop()
    }

    /** Ends every stream: the server stops listening. */
    fun drop() {
        streams.forEach { it.offer(END) }
    }

    /** Open streams: tests wait for one before sending. */
    val listeners: Int get() = streams.size

    fun emit(event: ServerEvent) {
        val text = PleyJson.encodeToString(ServerEvent.Serializer, event)
        streams.forEach { it.offer(text) }
    }

    /** Sends [event] to the streams of profile [profileId] only. */
    fun emitTo(profileId: Long, event: ServerEvent) {
        val text = PleyJson.encodeToString(ServerEvent.Serializer, event)
        streams.forEach { if (owners[it] == profileId) it.offer(text) }
    }

    private fun scan() {
        val library = state.catalog.libraries[scans++ % state.catalog.libraries.size]
        val total = library.itemCount.toLong()
        emit(ServerEvent.ScanStarted(library.id))
        emit(ServerEvent.ScanProgress(library.id, ScanPhase.Walking, 0, total))
        emit(ServerEvent.ScanProgress(library.id, ScanPhase.Probing, total / 2, total))
        emit(ServerEvent.ScanProgress(library.id, ScanPhase.Metadata, total, total))
        emit(ServerEvent.ScanFinished(library.id, added = 0, updated = 0, removed = 0))
        emit(ServerEvent.LibraryChanged(library.id))
    }

    /** The stream for [caller], until its token ends, it goes away or the server stops. */
    fun response(caller: Caller): MockResponse {
        val queue = LinkedBlockingQueue<String>()
        val body = object : MockResponseBody {
            override val contentLength: Long = -1L

            override fun writeTo(sink: BufferedSink) {
                owners[queue] = caller.profile.id
                streams += queue
                try {
                    // The headers go out with the first comment.
                    chunk(sink, ": connected\n\n")
                    while (true) {
                        val left = caller.expiresAtMs?.let { it - state.nowMs() } ?: Long.MAX_VALUE
                        if (left <= 0) break
                        val next = queue.poll(minOf(left, state.options.keepAliveMs), TimeUnit.MILLISECONDS)
                        when {
                            next === END -> break
                            next != null -> chunk(sink, "data: $next\n\n")
                            else -> chunk(sink, ": keep-alive\n\n")
                        }
                    }
                    sink.writeUtf8("0\r\n\r\n")
                    sink.flush()
                } catch (_: IOException) {
                    // The app went away.
                } catch (_: InterruptedException) {
                    Thread.currentThread().interrupt()
                } finally {
                    streams -= queue
                    owners -= queue
                }
            }
        }
        return MockResponse.Builder()
            .code(200)
            .body(body)
            // Chunked, written here, so the stream can end cleanly.
            .removeHeader("Content-Length")
            .setHeader("Transfer-Encoding", "chunked")
            .setHeader("Content-Type", "text/event-stream")
            .setHeader("Cache-Control", "no-cache")
            .build()
    }

    private fun chunk(sink: BufferedSink, text: String) {
        val bytes = text.toByteArray(Charsets.UTF_8)
        sink.writeUtf8(Integer.toHexString(bytes.size)).writeUtf8("\r\n").write(bytes).writeUtf8("\r\n")
        sink.flush()
    }

    private companion object {
        /** Ends a stream. Compared by identity. */
        val END = String(charArrayOf('e', 'n', 'd'))
    }
}
