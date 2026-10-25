package app.pley.android.model

import kotlinx.serialization.Serializable

/**
 * When subtitles come on by themselves, for titles without a choice
 * (apps/server/src/streams.rs, `SubtitleMode`).
 */
@Serializable(with = SubtitleMode.Serializer::class)
enum class SubtitleMode(override val wire: String) : WireEnum {
    /** Never. */
    Off("off"),

    /** Forced subtitles (foreign dialogue) in the language of the audio: the server's default. */
    Forced("forced"),

    /** Full subtitles in the subtitle language when the audio is in another. */
    Foreign("foreign"),

    /** Always, in the subtitle language. */
    Always("always"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<SubtitleMode>("SubtitleMode", SubtitleMode.entries, SubtitleMode.Unknown)
}

/**
 * The tracks a profile starts titles with when it hasn't picked any for
 * them: the body and the answer of PUT /api/me/track-defaults, a route the
 * server doesn't have yet (today only the web app sets these, with
 * PATCH /api/me). The same three fields come with [Me].
 *
 * Both languages are ISO 639 codes ("eng", "fr"). A `null` is left out of
 * the body (PleyJson), which the route is to read as `null`.
 */
@Serializable
data class TrackDefaults(
    /** The main audio track in this language; `null`: the file's default track. */
    val audioLanguage: String? = null,
    /** `null`: the audio's language. */
    val subtitleLanguage: String? = null,
    val subtitleMode: SubtitleMode = SubtitleMode.Forced,
)
