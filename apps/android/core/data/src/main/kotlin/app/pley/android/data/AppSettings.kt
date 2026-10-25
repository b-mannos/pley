package app.pley.android.data

import kotlinx.coroutines.flow.Flow

/** Matching the TV's display mode to the video. */
enum class FrameRateMatching {
    /** Never. */
    Off,

    /** Phones: only when the switch is seamless. */
    Seamless,

    /** TVs: for videos of 10 minutes or more, following the TV's own setting. */
    Auto,

    /** TVs: always. */
    Always,
}

/** Sending a sound format to the receiver as it is, or decoding it here. */
enum class Passthrough { Auto, On, Off }

/** Subtitle backgrounds. */
enum class SubtitleBackground { None, DarkBox }

/**
 * This device's settings (not the profile's: those live on the server).
 * Defaults are what the app does without being asked.
 */
data class Settings(
    /** "Original" or a preset's name, on Wi-Fi or Ethernet. */
    val qualityOnHomeNetwork: String = "Original",
    /** "Original" or a preset's name, on mobile data. */
    val qualityOnMobile: String = "Original",
    /** `null`: the device's default, Seamless on phones and Auto on TVs. */
    val frameRateMatching: FrameRateMatching? = null,
    /** Per format: "ac3", "eac3", "eac3-joc", "truehd", "dts", "dts-hd". */
    val passthrough: Map<String, Passthrough> = emptyMap(),
    /** TVs, under Advanced. */
    val tunneling: Boolean = false,
    /** A share of the video's height. */
    val subtitleSize: Float = 0.048f,
    val subtitleBackground: SubtitleBackground = SubtitleBackground.None,
    val autoplayNext: Boolean = true,
    /** Sign in from a private browser tab, which keeps no cookie behind. */
    val privateSignInTab: Boolean = true,
    /** TVs: skip "Who's watching?" and start as this profile. */
    val alwaysStartAs: Long? = null,
    val showPlaybackDetails: Boolean = false,
)

interface AppSettings {
    val settings: Flow<Settings>

    suspend fun update(change: (Settings) -> Settings)
}
