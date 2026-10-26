package app.pley.android.player

import app.pley.android.model.Marker
import app.pley.android.model.MediaFile
import app.pley.android.model.MediaStream
import app.pley.android.model.PlaybackMethod
import app.pley.android.model.TranscodeInfo
import app.pley.android.player.sleep.SleepState

/** Where a playback stands. */
sealed interface PlayerPhase {
    /** Asking the server and loading. */
    data object Preparing : PlayerPhase

    /** The TV is changing its display mode to the video's frame rate; the picture may go dark. */
    data object Switching : PlayerPhase

    data object Buffering : PlayerPhase

    data object Playing : PlayerPhase

    data object Paused : PlayerPhase

    /** The end, with nothing next: the screen goes back to where playing started. */
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
    /** Subtitles shown as pictures, which a stream draws onto the video ("burn-in"). */
    val burnIn: Boolean = false,
)

/** A quality in the player's menu (apps/web/src/player/SettingsPopover.tsx). */
data class QualityChoice(
    /** "original", "8m"…: what [PlaybackController.selectQuality] takes. */
    val id: String,
    /** "Original", "8 Mbps 1080p". */
    val label: String,
    /** "1080p · 8.2 Mbps", "Plays as 8 Mbps 1080p here", "Converted". */
    val hint: String?,
    val selected: Boolean,
)

/** A chapter for the chapters menu and the seek bar's ticks. */
data class ChapterMark(
    val title: String?,
    val startMs: Long,
    val endMs: Long,
    /** Absolute address of its picture, once the file has seek previews. */
    val imageUrl: String?,
)

/** One line of the details: "Video", "HEVC 4K HDR10 → H.264 1080p SDR". */
data class DetailRow(val label: String, val value: String)

/** How the video plays, while the details are open (apps/web/src/player/PlaybackDetails.tsx). */
data class PlaybackDetailsUi(
    /** "Direct play", "Direct stream", "Transcoding", "Direct stream · Audio transcoding". */
    val method: String,
    /** Why, in a sentence or two. */
    val why: String,
    /** Where a transcode runs ("GPU", "CPU"…), for its badge; `null` otherwise. */
    val pipeline: String?,
    val rows: List<DetailRow>,
    /** The engine's own lines: decoders, picture, dropped frames. */
    val engine: List<String>,
    /** The way the server sends it; `null` in details made by hand. */
    val playbackMethod: PlaybackMethod? = null,
    /** The file playing as the server described it, for a full technical readout. */
    val file: MediaFile? = null,
    /** The streams in play: the file's video, the audio and the subtitles chosen. */
    val video: MediaStream? = null,
    val audio: MediaStream? = null,
    val subtitle: MediaStream? = null,
    /** How [subtitle] reaches the screen; `null` without subtitles. */
    val subtitleDelivery: SubtitleDelivery? = null,
    /** The transcode's live numbers (remux and transcode); `null` in direct play or before they come. */
    val transcode: TranscodeInfo? = null,
)

/** How subtitles reach the screen. */
enum class SubtitleDelivery {
    /** Read from the file played as it is, drawn by the player. */
    File,

    /** The server's WebVTT (streams, and subtitle files beside the media), drawn by the player. */
    Server,

    /** Picture subtitles beside a stream: the server's .mks of the track, drawn by the player. */
    ServerPictures,

    /** Drawn into the picture by the server's transcode. */
    BurnedIn,
}

/** What plays next, during its card. */
data class UpNextUi(
    val itemId: Long,
    val title: String,
    /** "Up next · S1 E2", or "Up next". */
    val heading: String,
    /** Absolute address of its still (16:9), for the card; preloaded. */
    val imageUrl: String?,
    /** Seconds before it plays by itself; `null` when it won't (autoplay off, or cancelled). */
    val secondsLeft: Int?,
)

/**
 * What a player screen shows, in one immutable value. Positions change
 * often: screens read them only where they draw, so the rest of the
 * screen doesn't recompose with them.
 */
data class PlayerUiState(
    val phase: PlayerPhase = PlayerPhase.Preparing,
    /** The item playing; 0 until known. */
    val itemId: Long = 0,
    /** A movie's "Title (Year)", an episode's show. */
    val title: String = "",
    /** An episode's "S1 E2 — Title". */
    val subtitle: String? = null,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val bufferedMs: Long = 0,
    val audio: List<TrackChoice> = emptyList(),
    val subtitles: List<TrackChoice> = emptyList(),
    /** The quality chosen, as the menu's row says it: "Original", "8 Mbps 1080p", "8 Mbps 1080p (auto)". */
    val quality: String = "Original",
    val qualities: List<QualityChoice> = emptyList(),
    /** "Direct play", "Direct stream", "Transcoding"; `null` before the server answered. */
    val method: String? = null,
    /** Why it plays this way when the player chose it: "The original didn't play smoothly on this device." */
    val note: String? = null,
    /** The player switched to a transcode by itself: offer "Play original". */
    val canPlayOriginal: Boolean = false,
    /** The server can't convert fast enough: say so, and offer a lower quality. */
    val suggestLowerQuality: Boolean = false,
    /** "Skip Intro" or "Skip Credits" while one is showing. */
    val skipLabel: String? = null,
    /** The playing file's intro and credits, for the seek bar. */
    val markers: List<Marker> = emptyList(),
    val chapters: List<ChapterMark> = emptyList(),
    /** Seek previews are ready: [PlaybackController.thumbnail] gives them. */
    val hasThumbnails: Boolean = false,
    /** The title of what plays next, during its card. */
    val upNext: String? = null,
    val upNextSecondsLeft: Int? = null,
    /** Everything the Up Next card shows. */
    val upNextCard: UpNextUi? = null,
    /** The details, while open. */
    val details: PlaybackDetailsUi? = null,
    /** A short message for a toast, until [PlaybackController.dismissNotice]: "This is the last episode". */
    val notice: String? = null,
    /** How far the subtitles are moved: positive shows them later ([PlaybackController.setSubtitleOffset]). */
    val subtitleOffsetMs: Long = 0,
    /** The subtitles chosen can be moved: text, or pictures drawn by the player from the server's .mks. */
    val subtitleSync: Boolean = false,
    /** The sleep timer. */
    val sleep: SleepState = SleepState(),
    /** The server stopped this playback (an administrator, or another device): the error says so, and the screen leaves. */
    val stoppedByServer: Boolean = false,
)
