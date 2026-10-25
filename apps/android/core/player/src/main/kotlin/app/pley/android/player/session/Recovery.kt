package app.pley.android.player.session

import app.pley.android.model.PlaybackMethod
import app.pley.android.model.PlaybackMode

/** Why a fallback plays something other than the original. */
enum class FallbackReason {
    /** The device couldn't play the original file. */
    Unplayable,

    /** The original didn't play smoothly here. */
    NotSmooth,
}

/** Why playback gave up. */
enum class Failure {
    /** Neither the original nor a conversion plays here. */
    Unplayable,

    /** It stops at the same spot each time. */
    Stuck,

    /** It stopped and couldn't get going again. */
    Stopped,

    /** The server couldn't be reached. */
    Network,

    /** The engine failed in a way no new session helps with. */
    Engine,
}

/**
 * The web player's recovery ladder (apps/web/src/player/VideoPlayer.tsx),
 * with the native player's own steps: an unsupported track counts as a
 * decoder error, and a decoder that drops frames switches to a transcode
 * once per item.
 *
 * - Direct play that the device can't decode or open: a remux when the
 *   video is H.264 or HEVC and only the container or the audio failed,
 *   else a transcode; each once.
 * - 404 or 410 ("playback session expired") or an unreadable playlist: a
 *   new session at the position, at most 3 times.
 * - Network errors (after the engine's own retries): a new session, at most 3.
 * - A 12 s stall: a direct play that has data but doesn't move becomes a
 *   transcode; a stream asks the server first ([Action.CheckServer]); a
 *   stream that keeps stopping gets one new session, a starving one up to 3.
 *
 * Budgets start over after 5 s of playback and on Retry. Not thread-safe:
 * the controller calls it on the main thread.
 */
class Recovery {
    sealed interface Action {
        /** A new session at the position; [mode] is asked for from now on when set. */
        data class NewSession(val mode: PlaybackMode?, val reason: FallbackReason?) : Action

        /** Ask the server how its transcode is doing, then call [afterServerCheck]. */
        data object CheckServer : Action

        /** The server can't keep up: offer a lower quality, keep playing. */
        data object SuggestLowerQuality : Action

        data class Fail(val failure: Failure, val detail: String?) : Action
    }

    /** The mode asked for instead of the quality's, after a fallback; `null` without one. */
    var fallbackMode: PlaybackMode? = null
        private set

    /** Why [fallbackMode] is set. */
    var fallbackReason: FallbackReason? = null
        private set

    /** The automatic switch for dropped frames happened (once per item). */
    var struggleSwitched: Boolean = false
        private set

    private var remuxTried = false
    private var transcodeTried = false

    /** New sessions for expired sessions and starving stalls. */
    var recreate = 0
        private set

    /** New sessions after network errors. */
    var network = 0
        private set

    /** New sessions for media that arrives and doesn't play. */
    var stuck = 0
        private set

    private var playedFrom: Long? = null

    /** Where playback is now: 5 s played on from where it last (re)started means the trouble is over. */
    fun onPosition(positionMs: Long) {
        val from = playedFrom
        if (from == null || positionMs < from) {
            playedFrom = positionMs
        } else if (positionMs - from >= RECOVERED_AFTER_MS) {
            recreate = 0
            network = 0
            stuck = 0
            playedFrom = positionMs
        }
    }

    /** A seek or a new session: progress counts from where it lands. */
    fun onJump() {
        playedFrom = null
    }

    /** What to do about [error] in a session played as [method], whose video is [videoCodec]. */
    fun decide(error: EngineError, method: PlaybackMethod, videoCodec: String?): Action = when (error) {
        is EngineError.Unsupported -> fallback(method, videoCodec, partial = error.audio || error.container, detail = error.detail)
        is EngineError.Http -> if (error.status == 404 || error.status == 410) expired(error.detail) else networkError(error.detail)
        is EngineError.Playlist -> expired(error.detail)
        is EngineError.Network -> networkError(error.detail)
        is EngineError.Other -> Action.Fail(Failure.Engine, error.detail)
    }

    private fun fallback(method: PlaybackMethod, videoCodec: String?, partial: Boolean, detail: String): Action {
        val copyable = videoCodec?.lowercase() in COPYABLE_VIDEO
        return when {
            method == PlaybackMethod.Direct && partial && copyable && !remuxTried -> {
                remuxTried = true
                switchTo(PlaybackMode.Remux, FallbackReason.Unplayable)
            }
            method != PlaybackMethod.Transcode && !transcodeTried -> {
                transcodeTried = true
                switchTo(PlaybackMode.Transcode, FallbackReason.Unplayable)
            }
            else -> Action.Fail(Failure.Unplayable, detail)
        }
    }

    private fun switchTo(mode: PlaybackMode, reason: FallbackReason): Action {
        fallbackMode = mode
        fallbackReason = reason
        return Action.NewSession(mode, reason)
    }

    private fun expired(detail: String): Action =
        if (recreate < MAX_RECREATE) {
            recreate++
            Action.NewSession(null, null)
        } else {
            Action.Fail(Failure.Stopped, detail)
        }

    private fun networkError(detail: String): Action =
        if (network < MAX_NETWORK) {
            network++
            Action.NewSession(null, null)
        } else {
            Action.Fail(Failure.Network, detail)
        }

    /**
     * Playback stood still for 12 s while meant to play. [starving] when it
     * waits for data (nothing buffered ahead), rather than for a decoder.
     */
    fun onStall(method: PlaybackMethod, starving: Boolean): Action {
        if (!starving && method == PlaybackMethod.Direct && !transcodeTried) {
            transcodeTried = true
            return switchTo(PlaybackMode.Transcode, FallbackReason.NotSmooth)
        }
        if (method != PlaybackMethod.Direct) return Action.CheckServer
        return afterStall(starving)
    }

    /** The server's transcode runs at [speed] (`null` when it can't say): too slow, or try a new session. */
    fun afterServerCheck(speed: Double?, starving: Boolean): Action =
        if (speed != null && speed < 1.0) Action.SuggestLowerQuality else afterStall(starving)

    private fun afterStall(starving: Boolean): Action = when {
        // Media that arrives and doesn't play: one new session, the same again would only loop.
        !starving -> if (stuck < MAX_STUCK) {
            stuck++
            Action.NewSession(null, null)
        } else {
            Action.Fail(Failure.Stuck, null)
        }
        recreate < MAX_RECREATE -> {
            recreate++
            Action.NewSession(null, null)
        }
        else -> Action.Fail(Failure.Stopped, null)
    }

    /**
     * The decoder drops frames: one switch to a transcode per item, with
     * "Play original" offered; `null` when it switched already or transcodes.
     */
    fun onStruggling(method: PlaybackMethod): Action? {
        if (struggleSwitched || method == PlaybackMethod.Transcode) return null
        struggleSwitched = true
        transcodeTried = true
        return switchTo(PlaybackMode.Transcode, FallbackReason.NotSmooth)
    }

    /** "Play original" after a fallback: the quality's own mode again (dropped frames don't switch twice). */
    fun playOriginal() {
        fallbackMode = null
        fallbackReason = null
    }

    /** Another quality chosen: its own mode, as on the web. */
    fun qualityChanged() = playOriginal()

    /** Retry after an error: every new-session budget again. */
    fun retry() {
        recreate = 0
        network = 0
        stuck = 0
        playedFrom = null
    }

    /** Another item: everything starts over. */
    fun newItem() {
        retry()
        fallbackMode = null
        fallbackReason = null
        struggleSwitched = false
        remuxTried = false
        transcodeTried = false
    }

    companion object {
        const val RECOVERED_AFTER_MS = 5_000L
        const val MAX_RECREATE = 3
        const val MAX_NETWORK = 3
        const val MAX_STUCK = 1

        /** Videos a remux copies, which the device's decoders take in another container. */
        private val COPYABLE_VIDEO = setOf("h264", "hevc")
    }
}
