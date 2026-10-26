package app.pley.android.player.session

import android.content.Context
import app.pley.android.model.PlaybackInfo
import app.pley.android.model.PlaybackMethod
import app.pley.android.player.R

/** What the session says to people, from strings_session.xml. */
interface PlayerTexts {
    fun quality(preset: QualityPreset): String

    /** "8 Mbps 1080p (auto)": the original, converted at the quality the server chose. */
    fun autoQuality(preset: QualityPreset): String

    /** The original's hint in the quality menu when the server converts it; [file] is the original's "4K · 62 Mbps". */
    fun playsAs(preset: QualityPreset, file: String?): String

    /** Every quality but the original. */
    val converted: String
    val skipIntro: String
    val skipCredits: String

    /** "Up next · S1 E2", or "Up next". */
    fun upNext(label: String?): String
    val lastEpisode: String
    val lastInQueue: String
    val subtitlesOff: String
    val subtitlesFailed: String

    /** "Direct play", "Direct stream", "Transcoding", "Direct stream · Audio transcoding". */
    fun method(info: PlaybackInfo): String

    /** The method of a file played from its download: "Downloaded". */
    val downloaded: String get() = "Downloaded"

    /** A download that doesn't play here as it is, with the server out of reach to convert it. */
    val downloadNeedsServer: String get() = "This download doesn't play on this device as it is. It plays with the server in reach, which converts it."

    /** What [method] means, when nothing more specific is known. */
    fun why(method: PlaybackMethod): String
    fun fallback(reason: FallbackReason): String
    val slowServer: String
    fun failure(failure: Failure, detail: String?): String
    val noFile: String

    /** The server stopped the playback: who did, and their [message] when given. */
    fun stopped(message: String?, byAdmin: Boolean): String

    // The details' rows.
    val rowVideo: String
    val rowAudio: String
    val rowBitrate: String
    val rowEncoder: String
    val rowHdr: String
    val rowSubtitles: String
    val rowSpeed: String
    fun toneMapped(tonemap: String): String
    val burnedIn: String
    fun speed(speed: String, readySeconds: Int): String
}

/** [PlayerTexts] from the app's resources. */
class ResourcePlayerTexts(context: Context) : PlayerTexts {
    private val res = context.applicationContext.resources

    private fun s(id: Int) = res.getString(id)

    override fun quality(preset: QualityPreset): String = s(
        when (preset.id) {
            "30m" -> R.string.session_quality_30m
            "20m" -> R.string.session_quality_20m
            "15m" -> R.string.session_quality_15m
            "12m" -> R.string.session_quality_12m
            "8m" -> R.string.session_quality_8m
            "4m" -> R.string.session_quality_4m
            "2m" -> R.string.session_quality_2m
            else -> R.string.session_quality_original
        },
    )

    override fun autoQuality(preset: QualityPreset): String = res.getString(R.string.session_quality_auto, quality(preset))

    override fun playsAs(preset: QualityPreset, file: String?): String =
        if (file.isNullOrEmpty()) {
            res.getString(R.string.session_quality_plays_as, quality(preset))
        } else {
            res.getString(R.string.session_quality_plays_as_file, quality(preset), file)
        }

    override val converted: String get() = s(R.string.session_quality_converted)
    override val skipIntro: String get() = s(R.string.session_skip_intro)
    override val skipCredits: String get() = s(R.string.session_skip_credits)

    override fun upNext(label: String?): String =
        if (label.isNullOrEmpty()) s(R.string.session_up_next) else res.getString(R.string.session_up_next_episode, label)

    override val lastEpisode: String get() = s(R.string.session_last_episode)
    override val lastInQueue: String get() = s(R.string.session_last_in_queue)
    override val subtitlesOff: String get() = s(R.string.session_subtitles_off)
    override val subtitlesFailed: String get() = s(R.string.session_subtitles_failed)

    override fun method(info: PlaybackInfo): String = when (info.method) {
        PlaybackMethod.Remux -> if (info.audioConverted) s(R.string.session_method_remux_audio) else s(R.string.session_method_remux)
        PlaybackMethod.Transcode -> s(R.string.session_method_transcode)
        else -> s(R.string.session_method_direct)
    }

    override fun why(method: PlaybackMethod): String = s(
        when (method) {
            PlaybackMethod.Remux -> R.string.session_why_remux
            PlaybackMethod.Transcode -> R.string.session_why_transcode
            else -> R.string.session_why_direct
        },
    )

    override fun fallback(reason: FallbackReason): String = s(
        when (reason) {
            FallbackReason.Unplayable -> R.string.session_fallback_unplayable
            FallbackReason.NotSmooth -> R.string.session_fallback_not_smooth
        },
    )

    override val slowServer: String get() = s(R.string.session_slow_server)

    override fun failure(failure: Failure, detail: String?): String = when (failure) {
        Failure.Unplayable -> s(R.string.session_error_unplayable)
        Failure.Stuck -> s(R.string.session_error_stuck)
        Failure.Short -> s(R.string.session_error_short)
        Failure.Stopped -> s(R.string.session_error_stopped)
        Failure.Network -> s(R.string.session_error_network)
        Failure.Engine -> res.getString(R.string.session_error_engine, detail?.substringBefore(':')?.takeIf { it.isNotBlank() } ?: "?")
    }

    override val noFile: String get() = s(R.string.session_error_no_file)
    override val downloaded: String get() = s(R.string.session_method_downloaded)
    override val downloadNeedsServer: String get() = s(R.string.session_error_download_needs_server)

    override fun stopped(message: String?, byAdmin: Boolean): String {
        val who = s(if (byAdmin) R.string.session_stopped_by_admin else R.string.session_stopped_elsewhere)
        return if (message.isNullOrBlank()) who else res.getString(R.string.session_stopped_message, who, message.trim())
    }
    override val rowVideo: String get() = s(R.string.session_row_video)
    override val rowAudio: String get() = s(R.string.session_row_audio)
    override val rowBitrate: String get() = s(R.string.session_row_bitrate)
    override val rowEncoder: String get() = s(R.string.session_row_encoder)
    override val rowHdr: String get() = s(R.string.session_row_hdr)
    override val rowSubtitles: String get() = s(R.string.session_row_subtitles)
    override val rowSpeed: String get() = s(R.string.session_row_speed)

    override fun toneMapped(tonemap: String): String = res.getString(R.string.session_tone_mapped, tonemap)

    override val burnedIn: String get() = s(R.string.session_burned_in)

    override fun speed(speed: String, readySeconds: Int): String =
        if (readySeconds > 0) res.getString(R.string.session_speed_ready, speed, readySeconds) else res.getString(R.string.session_speed, speed)
}
