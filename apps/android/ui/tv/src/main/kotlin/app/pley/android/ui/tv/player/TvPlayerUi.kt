package app.pley.android.ui.tv.player

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.focus.FocusRequester

/** The side panels (design spec §2.7). */
internal enum class PlayerPanel { Subtitles, Audio, Quality, Chapters, Info }

/** What can be given focus in the player. */
internal enum class PlayerFocus {
    /** The video itself, holding focus while the controls are hidden. */
    Video,
    Skip,
    Scrubber,
    PlayPause,
    Subtitles,
    Audio,
    Quality,
    Chapters,
    Info,
    UpNext,

    /** The panel showing (its current choice). */
    Panel,
    Error,
}

/** A request for focus: [serial] makes asking twice for the same thing ask again. */
@Immutable
internal data class FocusAsk(val target: PlayerFocus, val serial: Int)

/** A message at the top right: [ms] on screen, in danger text when [error]. */
@Immutable
internal data class PlayerToast(val text: String, val ms: Long, val serial: Int, val error: Boolean = false)

/**
 * What the TV player shows beyond the playback's own state: the controls
 * hidden or shown, a side panel, where focus goes next, the toast. Only
 * this composition's; nothing here survives the screen.
 */
@Stable
internal class TvPlayerUi {
    /** The controls show. */
    var controls: Boolean by mutableStateOf(false)
        private set

    var panel: PlayerPanel? by mutableStateOf(null)
        private set

    /** The panel shown last, kept while it slides away. */
    var lastPanel: PlayerPanel? by mutableStateOf(null)
        private set

    /** Where focus goes next; the screen asks for it once the target is laid out. */
    var focusAsk: FocusAsk by mutableStateOf(FocusAsk(PlayerFocus.Video, 0))
        private set

    /** Changes with every key while the controls show: the 5 s before they hide start again. */
    var input: Int by mutableIntStateOf(0)
        private set

    var toast: PlayerToast? by mutableStateOf(null)
        private set

    /** The subtitles the CAPTIONS key turned off, to turn back on. */
    var rememberedSubtitle: Long? = null

    /** What has focus now, as far as the player cares. */
    var focused: PlayerFocus? by mutableStateOf(null)

    private val requesters = PlayerFocus.entries.associateWith { FocusRequester() }

    fun requester(target: PlayerFocus): FocusRequester = requesters.getValue(target)

    fun ask(target: PlayerFocus) {
        focusAsk = FocusAsk(target, focusAsk.serial + 1)
    }

    fun touch() {
        input++
    }

    fun showControls(focus: PlayerFocus) {
        controls = true
        touch()
        ask(focus)
    }

    /** Hides the controls; focus goes to [then] (Skip while it shows, else the video). */
    fun hideControls(then: PlayerFocus = PlayerFocus.Video) {
        if (!controls) return
        controls = false
        if (focused != PlayerFocus.UpNext && focused != PlayerFocus.Error) ask(then)
    }

    fun openPanel(panel: PlayerPanel) {
        this.panel = panel
        lastPanel = panel
        ask(PlayerFocus.Panel)
    }

    /** Closes the panel; focus goes back to the button that opened it, unless [refocus] is false. */
    fun closePanel(refocus: Boolean = true) {
        val was = panel ?: return
        panel = null
        touch()
        if (refocus) ask(buttonOf(was))
    }

    fun toast(text: String, ms: Long = TOAST_MS, error: Boolean = false) {
        toast = PlayerToast(text, ms, (toast?.serial ?: 0) + 1, error)
    }

    fun clearToast(serial: Int) {
        if (toast?.serial == serial) toast = null
    }

    companion object {
        /** Toasts show this long (design spec §2.10); errors longer. */
        const val TOAST_MS = 3_500L
        const val ERROR_TOAST_MS = 6_000L

        /** The CAPTIONS key's toast. */
        const val CAPTIONS_TOAST_MS = 2_000L

        /** The controls hide after this long without a key, unless paused or in a panel. */
        const val HIDE_AFTER_MS = 5_000L

        fun buttonOf(panel: PlayerPanel): PlayerFocus = when (panel) {
            PlayerPanel.Subtitles -> PlayerFocus.Subtitles
            PlayerPanel.Audio -> PlayerFocus.Audio
            PlayerPanel.Quality -> PlayerFocus.Quality
            PlayerPanel.Chapters -> PlayerFocus.Chapters
            PlayerPanel.Info -> PlayerFocus.Info
        }
    }
}
