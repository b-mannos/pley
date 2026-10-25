@file:OptIn(ExperimentalCoroutinesApi::class)

package app.pley.android.data

import app.pley.android.model.CollectionDetail
import app.pley.android.model.Hubs
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemList
import app.pley.android.model.Library
import app.pley.android.model.MaybeItem
import app.pley.android.model.PlaylistDetail
import app.pley.android.model.ServerEvent
import app.pley.android.network.LiveEvent
import app.pley.android.network.ServerEvents
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/**
 * The server's events, while the app is in the [foreground] and signed in:
 *
 * - `library_changed`, held [DEBOUNCE_MS] per library (a scan sends many),
 *   asks again for what shows that library: its rows, listings and
 *   collections, Home, the libraries, and the pages of its items;
 * - `libraries_changed` asks again for the libraries and Home;
 * - a new connection (the server replays nothing) and every return to the
 *   foreground ask again for what is older than [STALE_MS];
 * - scans fill [scans] (GET /api/activity refuses apps' tokens).
 *
 * The stream ends for good when the server refuses the app (401, 403):
 * until the next return to the foreground or sign-in.
 */
internal class ServerLiveUpdates(
    private val session: SessionManager,
    private val foreground: Flow<Boolean>,
    private val eventsOf: (issuer: String) -> ServerEvents,
    private val cache: ResourceCache,
    private val app: CoroutineScope,
) : LiveUpdates {
    private val _scans = MutableStateFlow<List<ScanState>>(emptyList())
    private val readScans = _scans.asStateFlow()

    // Nothing else starts it in the app: reading the scans does (LiveUpdates.scans).
    override val scans: StateFlow<List<ScanState>>
        get() = readScans.also { start() }

    private val started = AtomicBoolean(false)

    override fun start() {
        if (!started.compareAndSet(false, true)) return
        app.launch {
            val signedIn = session.state.map { (it as? SessionState.SignedIn)?.session?.issuer }.distinctUntilChanged()
            combine(foreground.distinctUntilChanged(), signedIn) { inFront, issuer -> issuer.takeIf { inFront } }
                .distinctUntilChanged()
                .collectLatest { issuer -> if (issuer != null) follow(issuer) else _scans.value = emptyList() }
        }
    }

    /** A fresh list of libraries: which are scanning now. */
    fun heard(libraries: List<Library>) {
        val scanning = libraries.filter { it.scanning }.mapTo(HashSet()) { it.id }
        _scans.update { scans ->
            val kept = scans.filter { it.libraryId == null || it.libraryId in scanning }
            kept + scanning.filter { id -> kept.none { it.libraryId == id } }.map { ScanState(it, null) }
        }
    }

    private suspend fun follow(issuer: String) = coroutineScope {
        // Back in the foreground (or just signed in): what is old is asked again.
        cache.revalidate(STALE_MS) { _, _ -> true }
        val pending = ConcurrentHashMap<Long, Job>()
        try {
            eventsOf(issuer).events().collect { live ->
                when (live) {
                    LiveEvent.Reconnected -> {
                        cache.revalidate(STALE_MS) { _, _ -> true }
                        if (session.meState.value?.fresh != true) launch { session.refresh() }
                    }
                    is LiveEvent.Event -> handle(live.event, pending)
                }
            }
        } catch (e: CancellationException) {
            throw e
        } catch (_: IOException) {
            // Refused (signed out, not allowed): stopped until the next start.
        } finally {
            // Changes heard but not yet acted on: asked again when next shown.
            for ((libraryId, job) in pending) {
                job.cancel()
                cache.markStale { key, value -> affects(libraryId, key, value) }
            }
            _scans.value = emptyList()
        }
    }

    private fun CoroutineScope.handle(event: ServerEvent, pending: MutableMap<Long, Job>) {
        when (event) {
            is ServerEvent.ScanStarted -> scan(event.libraryId, null)
            is ServerEvent.ScanProgress ->
                scan(event.libraryId, if (event.total > 0) (event.processed.toFloat() / event.total).coerceIn(0f, 1f) else null)
            is ServerEvent.ScanFinished -> _scans.update { scans -> scans.filterNot { it.libraryId == event.libraryId } }
            is ServerEvent.LibraryChanged -> {
                val libraryId = event.libraryId
                pending.remove(libraryId)?.cancel()
                pending[libraryId] = launch {
                    delay(DEBOUNCE_MS)
                    pending.remove(libraryId)
                    cache.revalidate { key, value -> affects(libraryId, key, value) }
                }
            }
            ServerEvent.LibrariesChanged -> cache.revalidate { key, _ -> key == SnapshotKeys.LIBRARIES || key == SnapshotKeys.HOME }
            ServerEvent.SessionsChanged, is ServerEvent.Unknown -> Unit
        }
    }

    private fun scan(libraryId: Long, progress: Float?) {
        _scans.update { scans ->
            val others = scans.filterNot { it.libraryId == libraryId }
            others + ScanState(libraryId, progress)
        }
    }

    companion object {
        /** How long a library's changes are gathered before asking again. */
        const val DEBOUNCE_MS = 1_500L

        /** What is older than this is asked again on reconnecting or coming back. */
        const val STALE_MS = 30_000L

        /** Whether the answer under [key], holding [value], shows something of [libraryId]. */
        fun affects(libraryId: Long, key: String, value: Any?): Boolean {
            if (SnapshotKeys.libraryOf(key) == libraryId) return true
            if (key == SnapshotKeys.HOME || key == SnapshotKeys.LIBRARIES) return true
            return when (value) {
                is ItemDetail -> value.libraryId == libraryId
                is ItemList -> value.items.any { it.libraryId == libraryId }
                is MaybeItem -> value.item?.libraryId == libraryId
                is CollectionDetail -> value.libraryId == libraryId
                is PlaylistDetail -> value.entries.any { it.item.libraryId == libraryId }
                is Hubs -> value.hubs.any { h -> h.libraryId == libraryId || h.items.any { it.libraryId == libraryId } }
                else -> false
            }
        }
    }
}
