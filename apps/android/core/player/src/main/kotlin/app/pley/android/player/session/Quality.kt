package app.pley.android.player.session

import app.pley.android.model.MediaFile
import app.pley.android.model.PlaybackMode
import app.pley.android.model.StreamKind

/**
 * A quality the player offers (apps/web/src/player/quality.ts): the
 * original, or a transcode capped at [maxBitrateKbps] and [maxHeight].
 * [id] is what the server's `auto_quality` and the settings use ("8m").
 */
data class QualityPreset(
    val id: String,
    val maxBitrateKbps: Int?,
    val maxHeight: Int?,
) {
    val original: Boolean get() = id == ORIGINAL_ID

    /**
     * How the quality is asked for: the original lets the server choose
     * (direct play when the device can), every other quality is a transcode.
     */
    val mode: PlaybackMode get() = if (original) PlaybackMode.Auto else PlaybackMode.Transcode

    companion object {
        const val ORIGINAL_ID = "original"
    }
}

/** The web's qualities, best first, and which of them a file is offered. */
object Qualities {
    val ORIGINAL = QualityPreset(QualityPreset.ORIGINAL_ID, null, null)

    val ALL: List<QualityPreset> = listOf(
        ORIGINAL,
        QualityPreset("30m", 30_000, 2160),
        QualityPreset("20m", 20_000, 1080),
        QualityPreset("15m", 15_000, 2160),
        QualityPreset("12m", 12_000, 1080),
        QualityPreset("8m", 8_000, 1080),
        QualityPreset("4m", 4_000, 720),
        QualityPreset("2m", 2_000, 480),
    )

    /** The preset with [id], else the original. */
    fun byId(id: String?): QualityPreset = ALL.firstOrNull { it.id == id } ?: ORIGINAL

    /**
     * The qualities worth offering for [file]: the original, then those that
     * make it smaller (every one when its bitrate and size are unknown).
     */
    fun forFile(file: MediaFile?): List<QualityPreset> {
        val kbps = file?.bitrate?.takeIf { it > 0 }?.let { it / 1000.0 }
        val height = file?.height ?: file?.streams?.firstOrNull { it.kind == StreamKind.Video }?.height
        if (kbps == null && height == null) return ALL
        return ALL.filter { q ->
            q.original ||
                (kbps != null && (q.maxBitrateKbps ?: 0) < kbps) ||
                (height != null && (q.maxHeight ?: 0) < height)
        }
    }

    /**
     * The quality to play [file] at: [remembered] when the file is offered
     * it, else the original (an 8 Mbps cap on a 5 Mbps file plays the
     * original, as on the web).
     */
    fun choose(file: MediaFile?, remembered: String?): QualityPreset {
        val wanted = byIdOrLabel(remembered)
        return forFile(file).firstOrNull { it.id == wanted.id } ?: ORIGINAL
    }

    /** A setting's value: a preset's id ("8m"), or "Original" as the settings' default says it. */
    fun byIdOrLabel(value: String?): QualityPreset {
        if (value == null) return ORIGINAL
        return ALL.firstOrNull { it.id.equals(value, ignoreCase = true) } ?: ORIGINAL
    }
}
