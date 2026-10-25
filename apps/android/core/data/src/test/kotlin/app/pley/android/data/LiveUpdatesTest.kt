package app.pley.android.data

import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.model.ScanPhase
import app.pley.android.model.ServerEvent
import app.pley.android.network.LiveEvent
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class LiveUpdatesTest {
    @get:Rule
    val folder = TemporaryFolder()

    private fun TestScope.time(ms: Long) {
        testScheduler.advanceTimeBy(ms)
        testScheduler.runCurrent()
    }

    @Test
    fun `the stream is open only in the foreground and signed in`() = runTest {
        val data = signedIn(folder.newFolder())
        data.foreground.value = false
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()
        assertEquals(0, data.events.connected.value)

        data.foreground.value = true
        testScheduler.runCurrent()
        assertEquals(1, data.events.connected.value)

        data.foreground.value = false
        testScheduler.runCurrent()
        assertEquals(0, data.events.connected.value)

        data.foreground.value = true
        testScheduler.runCurrent()
        data.graph.session.signOut()
        testScheduler.runCurrent()
        assertEquals(0, data.events.connected.value)
        assertEquals(2, data.events.connections)
    }

    @Test
    fun `reading the scans starts following the server, as the shells do`() = runTest {
        val data = signedIn(folder.newFolder())
        testScheduler.runCurrent()
        assertEquals(0, data.events.connected.value)

        data.graph.liveUpdates.scans
        testScheduler.runCurrent()
        assertEquals(1, data.events.connected.value)
    }

    @Test
    fun `starting twice opens one stream`() = runTest {
        val data = signedIn(folder.newFolder())
        data.graph.liveUpdates.start()
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()
        assertEquals(1, data.events.connected.value)
    }

    @Test
    fun `a library's changes are gathered for 1,5 s, then what shows it is asked again`() = runTest {
        val data = signedIn(folder.newFolder())
        collect(data.graph.hubs.library(1))
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()
        val before = data.api.calls("libraryHubs").size

        repeat(5) {
            data.events.send(LiveEvent.Event(ServerEvent.LibraryChanged(1)))
            time(500)
        }
        // Still within 1.5 s of the last change.
        assertEquals(before, data.api.calls("libraryHubs").size)

        time(1_500)
        val asked = data.api.calls("libraryHubs").drop(before).map { it.args.single() }
        assertEquals(listOf<Any?>(1L), asked)
    }

    @Test
    fun `changes to the list of libraries ask for the libraries and Home again`() = runTest {
        val data = signedIn(folder.newFolder())
        collect(data.graph.libraries.libraries())
        collect(data.graph.hubs.home())
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()

        data.events.send(LiveEvent.Event(ServerEvent.LibrariesChanged))
        testScheduler.runCurrent()

        assertEquals(2, data.api.calls("libraries").size)
        assertEquals(2, data.api.calls("homeHubs").size)
    }

    @Test
    fun `reconnecting asks again for what is older than 30 s`() = runTest {
        val data = signedIn(folder.newFolder())
        collect(data.graph.hubs.home())
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()
        data.events.send(LiveEvent.Reconnected)
        testScheduler.runCurrent()
        assertEquals(1, data.api.calls("homeHubs").size)

        time(31_000)
        data.events.send(LiveEvent.Reconnected)
        testScheduler.runCurrent()
        assertEquals(2, data.api.calls("homeHubs").size)
    }

    @Test
    fun `coming back to the foreground asks again for what is old`() = runTest {
        val data = signedIn(folder.newFolder())
        collect(data.graph.hubs.home())
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()
        data.foreground.value = false
        time(60_000)

        data.foreground.value = true
        testScheduler.runCurrent()

        assertEquals(2, data.api.calls("homeHubs").size)
    }

    @Test
    fun `a refused stream stays closed until the next start`() = runTest {
        val data = signedIn(folder.newFolder())
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()
        data.events.refuse(403)
        time(120_000)
        assertEquals(0, data.events.connected.value)
        assertEquals(1, data.events.connections)

        data.foreground.value = false
        testScheduler.runCurrent()
        data.foreground.value = true
        testScheduler.runCurrent()
        assertEquals(2, data.events.connections)
    }

    @Test
    fun `scans fill the scan state`() = runTest {
        val data = signedIn(folder.newFolder())
        val live = data.graph.liveUpdates
        live.start()
        testScheduler.runCurrent()

        data.events.send(LiveEvent.Event(ServerEvent.ScanStarted(1)))
        testScheduler.runCurrent()
        assertEquals(listOf(ScanState(1, null)), live.scans.value)

        data.events.send(LiveEvent.Event(ServerEvent.ScanProgress(1, ScanPhase.Probing, processed = 25, total = 100)))
        testScheduler.runCurrent()
        assertEquals(listOf(ScanState(1, 0.25f)), live.scans.value)

        data.events.send(LiveEvent.Event(ServerEvent.ScanFinished(1)))
        testScheduler.runCurrent()
        assertTrue(live.scans.value.isEmpty())
    }

    @Test
    fun `libraries the server says are scanning show as scans`() = runTest {
        val data = signedIn(folder.newFolder()) {
            api.answer("libraries", listOf(Library(1, "Movies", LibraryKind.Movies, scanning = true), Library(2, "TV", LibraryKind.Shows)))
        }
        data.graph.liveUpdates.start()
        collect(data.graph.libraries.libraries())
        testScheduler.runCurrent()

        assertEquals(listOf(ScanState(1, null)), data.graph.liveUpdates.scans.value)
    }

    @Test
    fun `what a library change touches`() {
        assertTrue(ServerLiveUpdates.affects(1, "libhubs:1", null))
        assertTrue(ServerLiveUpdates.affects(1, "library:1:title.asc:head", null))
        assertTrue(ServerLiveUpdates.affects(1, "collections:1", null))
        assertTrue(ServerLiveUpdates.affects(1, SnapshotKeys.HOME, null))
        assertFalse(ServerLiveUpdates.affects(1, "libhubs:2", null))
        assertFalse(ServerLiveUpdates.affects(1, "library:11:title.asc:head", null))
        assertFalse(ServerLiveUpdates.affects(1, SnapshotKeys.PLAYLISTS, null))
    }
}
