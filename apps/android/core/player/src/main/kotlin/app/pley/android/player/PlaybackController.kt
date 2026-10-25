package app.pley.android.player

import android.graphics.Bitmap
import androidx.media3.common.text.Cue
import app.pley.android.player.session.SavedPlaybackState
import kotlinx.coroutines.flow.StateFlow

/**
 * One playback, as a player screen drives it: the server's session,
 * progress reports, tracks, quality, markers and what plays next. The
 * phone's and TV's screens share it; neither talks to the server itself.
 * Called on the main thread.
 */
interface PlaybackController {
    val state: StateFlow<PlayerUiState>

    /** The engine playing, for the screen's surface; `null` until prepared. */
    val engine: PlayerEngine?

    /**
     * The server's subtitles showing now (streams, and subtitle files beside
     * the media), for SubtitleView. Subtitles inside the file being played
     * as it is come from the engine's own player instead.
     */
    val cues: StateFlow<List<Cue>>

    fun play()

    fun pause()

    fun seekTo(positionMs: Long)

    /** Another audio track by the server's stream index; remembered for the item. */
    fun selectAudio(index: Long)

    /** Another subtitle track, or none when [index] is `null`; remembered for the item. */
    fun selectSubtitle(index: Long?)

    /** "original" or a quality preset's id ("8m"); remembered for this device and network. */
    fun selectQuality(name: String)

    /** Skips the intro or credits showing now. */
    fun skip()

    /** Plays what comes next now, without waiting for the countdown. */
    fun playNext()

    /** Hides the Up Next card; what plays next doesn't start by itself. */
    fun cancelUpNext()

    /** After the player switched to a transcode by itself: the original again. */
    fun playOriginal()

    /** The details are open: their transcode numbers are asked for every 2 s meanwhile. */
    fun setDetailsOpen(open: Boolean)

    /** The notice was shown. */
    fun dismissNotice()

    /** The seek preview at [positionMs]; `null` until the previews are made. */
    suspend fun thumbnail(positionMs: Long): Bitmap?

    /** What to start again with after the process was killed. */
    fun saveState(): SavedPlaybackState

    /** Tries again after an error. */
    fun retry()

    /** Ends the playback: the last position is reported and the server's session closed. */
    fun close()
}
