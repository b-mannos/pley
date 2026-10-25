package app.pley.android.network

import app.pley.android.model.ServerEvent
import kotlinx.coroutines.flow.Flow

/** What the server's event stream brings. */
sealed interface LiveEvent {
    data class Event(val event: ServerEvent) : LiveEvent

    /**
     * Connected again after a break. The server replays nothing, so
     * whatever was shown may be stale: refresh it.
     */
    data object Reconnected : LiveEvent
}

/**
 * The server's events (GET /api/events, server-sent events): library
 * changes and scans, while collected. It reconnects by itself, waiting
 * longer after each failure, and with a fresh token when the stream ends
 * with the old one.
 */
fun interface ServerEvents {
    fun events(): Flow<LiveEvent>
}
