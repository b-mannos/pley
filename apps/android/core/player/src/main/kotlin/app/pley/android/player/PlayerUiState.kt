package app.pley.android.player

/** Where a playback stands. */
sealed interface PlayerPhase {
    /** Asking the server and loading. */
    data object Preparing : PlayerPhase

    /** The TV is changing its display mode to the video's frame rate; the picture may go dark. */
    data object Switching : PlayerPhase

    data object Buffering : PlayerPhase

    data object Playing : PlayerPhase

    data object Paused : PlayerPhase

    /** The end, with nothing next. */
    data object Ended : PlayerPhase

    /** It stopped: [message] for people, and whether trying again may help. */
    data class Error(val message: String, val canRetry: Boolean) : PlayerPhase
}

/** An audio or subtitle track as the player's menus show it. */
data class TrackChoice(
    /** The server's stream index; `null` for "Off". */
    val index: Long?,
    val label: String,
    val selected: Boolean,
)

/**
 * What a player screen shows, in one immutable value. Positions change
 * often: screens read them only where they draw, so the rest of the
 * screen doesn't recompose with them.
 */
data class PlayerUiState(
    val phase: PlayerPhase = PlayerPhase.Preparing,
    val title: String = "",
    val subtitle: String? = null,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val bufferedMs: Long = 0,
    val audio: List<TrackChoice> = emptyList(),
    val subtitles: List<TrackChoice> = emptyList(),
    /** The quality chosen ("Original" or a preset's name). */
    val quality: String = "Original",
    /** "Skip Intro" or "Skip Credits" while one is showing. */
    val skipLabel: String? = null,
    /** The title of what plays next, during its countdown. */
    val upNext: String? = null,
    val upNextSecondsLeft: Int? = null,
)
