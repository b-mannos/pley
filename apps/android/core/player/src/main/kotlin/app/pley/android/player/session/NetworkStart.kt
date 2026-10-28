package app.pley.android.player.session

import app.pley.android.model.NetworkReport
import app.pley.android.model.PlaybackNetwork
import app.pley.android.network.ApiException
import app.pley.android.network.PleyApi
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull

/** Where Automatic's start comes from (docs/QUALITY.md, "Automatic"). */
sealed interface StartCeiling {
    /** The server couldn't say (out of reach, too slow, or older): the device's own memory, as before. */
    data object Device : StartCeiling

    /** The server's word, which takes the place of the device's memory: at most [kbps], `null` for no limit. */
    data class Server(val kbps: Int?) : StartCeiling
}

/**
 * What the server knows of this client's network, for Automatic's start:
 * GET /api/playback/network ([lookUp], at most [LOOKUP_MS], made alongside
 * what else a playback asks before it starts, so it adds no wait at home),
 * then, when it asks for one ([resolve]), a speed test on a network it
 * doesn't know: [READ_MS] of GET /api/playback/speedtest from its first
 * byte, its result PUT back. All of it takes [BUDGET_MS] at most from the
 * lookup; after that, Automatic starts with what is known.
 *
 * Automatic's later ceilings go to the server too ([report]), fire and
 * forget. A server without these routes (404) is left alone for the rest
 * of the playback. Main thread only.
 */
class NetworkStart(
    private val api: PleyApi,
    private val clientId: () -> String,
    private val reporter: SessionReporter,
    private val now: () -> Long,
) {
    /** The server has no such routes: nothing more is asked of it. */
    @Volatile
    private var unsupported = false

    /** A lookup: when it started, and the server's answer (`null`: none). */
    class Lookup internal constructor(val startedAt: Long, val answer: PlaybackNetwork?)

    /** GET /api/playback/network, [LOOKUP_MS] at most. */
    suspend fun lookUp(): Lookup {
        val startedAt = now()
        if (unsupported) return Lookup(startedAt, null)
        val answer = withTimeoutOrNull(LOOKUP_MS) { ask { api.playbackNetwork(clientId()) } }
        return Lookup(startedAt, answer)
    }

    /**
     * Where Automatic starts after [lookup]: the server's ceiling, after a
     * speed test when it asks for one; the device's memory when it couldn't say.
     */
    suspend fun resolve(lookup: Lookup): StartCeiling {
        val answer = lookup.answer ?: return StartCeiling.Device
        if (!answer.probe || answer.local) return StartCeiling.Server(answer.ceilingKbps)
        val left = BUDGET_MS - (now() - lookup.startedAt)
        // Too little left to measure anything: start as if nothing were known.
        if (left < READ_MS / 2) return StartCeiling.Server(answer.ceilingKbps)
        val sample = ask { api.speedTest(PROBE_BYTES, READ_MS, left - REPLY_MS) }
        if (sample == null || sample.bytes < MIN_BYTES) return StartCeiling.Server(answer.ceilingKbps)
        val kbps = sample.kbps.coerceIn(1, MAX_KBPS)
        // What the server keeps for it, should its answer be late: 70 % of it, none when it all came.
        val expected = if (sample.complete) null else (kbps * PROBE_SHARE).toInt().coerceAtLeast(1)
        val reply = CompletableDeferred<PlaybackNetwork?>()
        val id = clientId()
        reporter.send { reply.complete(ask { api.reportNetwork(NetworkReport.Probe(id, kbps, sample.complete)) }) }
        val wait = (BUDGET_MS - (now() - lookup.startedAt)).coerceAtLeast(MIN_REPLY_WAIT_MS)
        val answered = withTimeoutOrNull(wait) { reply.await() }
        return StartCeiling.Server(if (answered != null) answered.ceilingKbps else expected)
    }

    /**
     * Automatic's remembered ceiling changed: a step down ([kbps]), a step
     * up to a quality (its bitrate), or `null` (back to the original, or
     * Automatic picked again). Sent behind, best effort.
     */
    fun report(kbps: Int?) {
        if (unsupported) return
        val id = clientId()
        reporter.send { ask { api.reportNetwork(NetworkReport.Ceiling(id, kbps?.coerceIn(1, MAX_KBPS))) } }
    }

    /** [call]'s answer; `null` when it failed, and a 404 marks the server as without these routes. */
    private suspend fun <T> ask(call: suspend () -> T): T? = try {
        call()
    } catch (e: CancellationException) {
        throw e
    } catch (e: ApiException) {
        if (e.status == 404 || e.status == 405) unsupported = true
        null
    } catch (_: Exception) {
        null
    }

    companion object {
        /** The longest the lookup may take. */
        const val LOOKUP_MS = 1_500L

        /** The longest the lookup, the speed test and its report take together. */
        const val BUDGET_MS = 3_500L

        /** How long the speed test reads from its first byte. */
        const val READ_MS = 2_000L

        /** Kept for the report's answer out of the speed test's time. */
        const val REPLY_MS = 300L
        const val MIN_REPLY_WAIT_MS = 100L

        /** What the speed test asks for: 32 MiB, more than 2 s bring on most connections. */
        const val PROBE_BYTES = 32L * 1024 * 1024

        /** Less than this arrived: the test says nothing. */
        const val MIN_BYTES = 256L * 1024

        /** The share of a speed test the server keeps as the ceiling. */
        const val PROBE_SHARE = 0.7

        /** The most the server takes. */
        const val MAX_KBPS = 10_000_000
    }
}
