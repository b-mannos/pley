package app.pley.android.player

import kotlinx.coroutines.flow.StateFlow

/** The track playing in the background, for the mini player and Now Playing. */
data class NowPlaying(
    val itemId: Long,
    val title: String,
    val artist: String?,
    val playing: Boolean,
)

/**
 * Music, which keeps playing in the background with the lock screen's
 * controls (a media session service). Video never does.
 */
interface MusicControl {
    /** What plays now; `null` when nothing does. */
    val nowPlaying: StateFlow<NowPlaying?>

    /** Pauses the music because a video starts. */
    fun pauseForVideo()
}
