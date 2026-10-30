package app.pley.android.model

import kotlinx.serialization.SerializationException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ServerEventTest {
    @Test
    fun knownEventsRead() {
        assertEquals(ServerEvent.ScanStarted(1), ServerEvent.parse("""{"type":"scan_started","library_id":1}"""))
        assertEquals(
            ServerEvent.ScanProgress(1, ScanPhase.Probing, 2, 3),
            ServerEvent.parse("""{"type":"scan_progress","library_id":1,"phase":"probing","processed":2,"total":3}"""),
        )
        assertEquals(
            ServerEvent.ScanFinished(2, added = 4, updated = 1, removed = 0),
            ServerEvent.parse("""{"type":"scan_finished","library_id":2,"added":4,"updated":1,"removed":0}"""),
        )
        assertEquals(ServerEvent.LibraryChanged(3), ServerEvent.parse("""{"type":"library_changed","library_id":3}"""))
        assertEquals(ServerEvent.LibrariesChanged, ServerEvent.parse("""{"type":"libraries_changed"}"""))
        assertEquals(ServerEvent.SessionsChanged, ServerEvent.parse("""{"type":"sessions_changed"}"""))
        assertEquals(
            ServerEvent.SessionStopped("s1", "Server maintenance", byAdmin = true),
            ServerEvent.parse("""{"type":"session_stopped","session_id":"s1","message":"Server maintenance","by_admin":true}"""),
        )
        assertEquals(ServerEvent.SessionStopped("s2", null, false), ServerEvent.parse("""{"type":"session_stopped","session_id":"s2","message":null,"by_admin":false}"""))
        assertEquals(ServerEvent.WatchlistChanged, ServerEvent.parse("""{"type":"watchlist_changed"}"""))
        assertEquals(
            ServerEvent.UserStateChanged(listOf(203, 202, 201), children = false),
            ServerEvent.parse("""{"type":"user_state_changed","item_ids":[203,202,201],"children":false}"""),
        )
        assertEquals(ServerEvent.UserStateChanged(listOf(201), children = true), ServerEvent.parse("""{"type":"user_state_changed","item_ids":[201],"children":true}"""))
        assertEquals(ServerEvent.LiveChannelsChanged, ServerEvent.parse("""{"type":"live_channels_changed"}"""))
        assertEquals(ServerEvent.LiveTunersChanged, ServerEvent.parse("""{"type":"live_tuners_changed"}"""))
    }

    @Test
    fun unknownTypesAreKeptByName() {
        val event = ServerEvent.parse("""{"type":"markers_found","item_id":203}""")
        assertEquals(ServerEvent.Unknown("markers_found"), event)
        assertNull(event.libraryId)
        assertEquals(ServerEvent.Unknown(""), ServerEvent.parse("""{"library_id":1}"""))
    }

    @Test
    fun newFieldsAndPhasesDontBreakKnownEvents() {
        val event = ServerEvent.parse("""{"type":"scan_progress","library_id":1,"phase":"fingerprinting","processed":2,"total":3,"eta_ms":9}""")
        assertEquals(ServerEvent.ScanProgress(1, ScanPhase.Unknown, 2, 3), event)
        assertEquals(1L, event.libraryId)
    }

    @Test(expected = SerializationException::class)
    fun textThatIsntAnObjectFails() {
        ServerEvent.parse("""["scan_started"]""")
    }

    @Test
    fun eventsWriteWhatTheServerSends() {
        assertEquals(
            """{"type":"scan_progress","library_id":1,"phase":"probing","processed":2,"total":3}""",
            PleyJson.encodeToString(ServerEvent.serializer(), ServerEvent.ScanProgress(1, ScanPhase.Probing, 2, 3)),
        )
        assertEquals("""{"type":"libraries_changed"}""", PleyJson.encodeToString(ServerEvent.serializer(), ServerEvent.LibrariesChanged))
        assertEquals(
            """{"type":"user_state_changed","item_ids":[203,202,201],"children":false}""",
            PleyJson.encodeToString(ServerEvent.serializer(), ServerEvent.UserStateChanged(listOf(203, 202, 201))),
        )
        val events = listOf(
            ServerEvent.ScanStarted(4),
            ServerEvent.SessionsChanged,
            ServerEvent.WatchlistChanged,
            ServerEvent.UserStateChanged(listOf(201), children = true),
            ServerEvent.LiveChannelsChanged,
            ServerEvent.LiveTunersChanged,
            ServerEvent.Unknown("later"),
        )
        for (event in events) {
            assertEquals(event, ServerEvent.parse(PleyJson.encodeToString(ServerEvent.serializer(), event)))
        }
    }
}
