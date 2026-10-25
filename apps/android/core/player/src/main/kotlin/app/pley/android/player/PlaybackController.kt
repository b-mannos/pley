package app.pley.android.player

import kotlinx.coroutines.flow.StateFlow

/**
 * One playback, as a player screen drives it: the server's session,
 * progress reports, tracks, quality, markers and what plays next. The
 * phone's and TV's screens share it; neither talks to the server itself.
 */
interface PlaybackController {
    val state: StateFlow<PlayerUiState>

    /** The engine playing, for the screen's surface; `null` until prepared. */
    val engine: PlayerEngine?

    fun play()

    fun pause()

    fun seekTo(positionMs: Long)

    /** Another audio track by the server's stream index; remembered for the item. */
    fun selectAudio(index: Long)

    /** Another subtitle track, or none when [index] is `null`; remembered for the item. */
    fun selectSubtitle(index: Long?)

    /** "Original" or a quality preset's name. */
    fun selectQuality(name: String)

    /** Skips the intro or credits showing now. */
    fun skip()

    /** Plays what comes next now, without waiting for the countdown. */
    fun playNext()

    /** Tries again after an error. */
    fun retry()

    /** Ends the playback: the last position is reported and the server's session closed. */
    fun close()
}
