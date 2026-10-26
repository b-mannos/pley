package app.pley.android.player.session

import app.pley.android.caps.CapsDecision
import app.pley.android.caps.CapsSnapshot
import app.pley.android.caps.PlaybackCapsPolicy
import app.pley.android.model.DynamicRange
import app.pley.android.model.ItemDetail
import app.pley.android.model.MediaFile
import app.pley.android.model.MediaStream
import app.pley.android.model.PlaybackInfo
import app.pley.android.model.PlaybackMethod
import app.pley.android.model.PlaybackMode
import app.pley.android.model.PlaybackRequest
import app.pley.android.model.StreamKind
import app.pley.android.model.caps.ClientCaps
import app.pley.android.player.PlayRequest

/**
 * How a playback is asked for: which file, from where, with which tracks,
 * and what goes in the body of POST /api/playback. Pure: it reads the item,
 * the device's capabilities and the choices, nothing else.
 *
 * Track indexes go to the server only when the server has to act on them
 * (PleyApi.kt:116-120 of the old player sent them always, which forced
 * remuxes and burn-ins of files that play as they are):
 * - `audio_index` when the audio can't be played here (neither decoded nor
 *   passed through), or the session streams HLS (one audio track);
 * - `subtitle_index` only to burn subtitles shown as pictures into HLS,
 *   and never when this player side-loads them (the server's .mks).
 * In direct play both are switched here, by the engine.
 */
object PlaybackPlan {
    /** The file to play: the one asked for, else the one that plays best here, else the first. */
    fun file(item: ItemDetail, requested: Long?, caps: CapsSnapshot?, policy: PlaybackCapsPolicy): MediaFile? {
        requested?.let { id -> item.files.firstOrNull { it.id == id }?.let { return it } }
        val best = caps?.let { runCatching { policy.bestFile(item, it) }.getOrNull() }
        return item.files.firstOrNull { it.id == best } ?: item.files.firstOrNull()
    }

    /** Where to start: as asked, else from the start when asked or watched, else the resume point. */
    fun startMs(item: ItemDetail, request: PlayRequest): Long = when {
        request.startMs != null -> request.startMs.coerceAtLeast(0)
        request.fromStart || item.userState.watched -> 0
        else -> item.userState.positionMs.coerceAtLeast(0)
    }

    fun audioStreams(file: MediaFile?): List<MediaStream> = file?.streams.orEmpty().filter { it.kind == StreamKind.Audio }

    fun subtitleStreams(file: MediaFile?): List<MediaStream> = file?.streams.orEmpty().filter { it.kind == StreamKind.Subtitle }

    fun videoStream(file: MediaFile?): MediaStream? {
        val videos = file?.streams.orEmpty().filter { it.kind == StreamKind.Video }
        return videos.firstOrNull { it.default } ?: videos.firstOrNull()
    }

    /** The audio to start with: as asked, else the profile's choice, else the file's default, else the first. */
    fun initialAudio(file: MediaFile?, requested: Long?): MediaStream? {
        val audio = audioStreams(file)
        return requested?.let { r -> audio.firstOrNull { it.index.toLong() == r } }
            ?: audio.firstOrNull { it.index == file?.selectedAudio }
            ?: audio.firstOrNull { it.default }
            ?: audio.firstOrNull()
    }

    /**
     * The subtitles to start with: as asked, else the profile's choice (the
     * server applies its defaults to `selected_subtitle`); `null` is off,
     * whatever the file flags as default.
     */
    fun initialSubtitle(file: MediaFile?, requested: Long?): MediaStream? {
        val subs = subtitleStreams(file)
        return requested?.let { r -> subs.firstOrNull { it.index.toLong() == r } }
            ?: subs.firstOrNull { it.index == file?.selectedSubtitle }
    }

    /** Subtitles shown as pictures (PGS, VobSub): a stream burns them into the video. */
    fun isImage(stream: MediaStream?): Boolean = stream != null && stream.textBased == false

    /**
     * Whether pictures chosen for a stream are side-loaded (the .mks drawn
     * by the player) rather than burned in: the report says so
     * (`picture_subtitles_sideload`) and the player can read them.
     */
    fun sideloadsPictures(caps: CapsSnapshot?, canRead: Boolean = true): Boolean =
        canRead && caps?.report?.pictureSubtitlesSideload == true

    /** Where the server's .mks of picture subtitles [index] of [fileId] is. */
    fun pictureSubtitlesPath(fileId: Long, index: Int): String = "/api/files/$fileId/subtitles/$index.mks"

    /** Whether this device decodes [stream]'s audio or sends it as it is to the receiver. */
    fun audioPlaysHere(stream: MediaStream?, caps: CapsSnapshot?): Boolean {
        if (stream == null) return true
        val report = caps?.report ?: return true
        val codec = stream.codec.lowercase()
        val known = report.audioCodecs + report.audioDecode.orEmpty() + report.audioPassthrough.orEmpty()
        return known.any { it.equals(codec, ignoreCase = true) }
    }

    /** Whether this device opens [file]'s container as it is. */
    fun containerPlaysHere(file: MediaFile?, caps: CapsSnapshot?): Boolean {
        val container = file?.container?.lowercase() ?: return true
        val containers = caps?.report?.containers?.map { it.lowercase() } ?: return true
        return container in containers ||
            (container == "mkv" && "matroska" in containers) ||
            (container == "matroska" && "mkv" in containers)
    }

    /**
     * Whether the session will surely be a stream (HLS), before asking:
     * another mode than auto, a video this device doesn't play as it is, a
     * container it doesn't open, or audio it can't play.
     */
    fun streams(mode: PlaybackMode, decision: CapsDecision?, file: MediaFile?, audio: MediaStream?, caps: CapsSnapshot?): Boolean =
        mode == PlaybackMode.Remux || mode == PlaybackMode.Transcode ||
            (decision != null && decision != CapsDecision.Direct) ||
            !containerPlaysHere(file, caps) ||
            !audioPlaysHere(audio, caps)

    /** What a request carries besides the item: the choices of the moment. */
    data class Choices(
        val file: MediaFile,
        val startMs: Long,
        val audio: MediaStream?,
        val subtitle: MediaStream?,
        val quality: QualityPreset,
        val mode: PlaybackMode,
        /** The session will stream (HLS): the server needs the track indexes. */
        val streaming: Boolean,
        /** Pictures are side-loaded by the player: none to burn in. */
        val sideload: Boolean = false,
    )

    /** The body of POST /api/playback for [choices]. */
    fun request(itemId: Long, choices: Choices, clientId: String, caps: CapsSnapshot?): PlaybackRequest {
        val audioHere = audioPlaysHere(choices.audio, caps)
        return PlaybackRequest(
            itemId = itemId,
            fileId = choices.file.id,
            audioIndex = choices.audio?.index?.takeIf { choices.streaming || !audioHere },
            subtitleIndex = choices.subtitle?.index?.takeIf { choices.streaming && isImage(choices.subtitle) && !choices.sideload },
            maxBitrate = choices.quality.maxBitrateKbps,
            maxHeight = choices.quality.maxHeight,
            startMs = choices.startMs,
            mode = choices.mode,
            clientId = clientId,
            caps = caps?.report ?: ClientCaps(),
        )
    }

    /**
     * Whether [info]'s session must be asked again with the track indexes:
     * it streams, though it wasn't expected to, and its audio isn't the one
     * chosen, or the subtitles chosen are pictures to burn in.
     */
    fun needsIndexes(info: PlaybackInfo, sent: PlaybackRequest, choices: Choices): Boolean {
        if (!info.hls) return false
        val audio = choices.audio
        val wrongAudio = audio != null && sent.audioIndex == null && info.audioIndex != null && info.audioIndex != audio.index
        val missingBurn = isImage(choices.subtitle) && sent.subtitleIndex == null && !choices.sideload
        return wrongAudio || missingBurn
    }

    /** The container's MIME type, for engines that can't tell from the address. */
    fun mimeTypeOf(container: String?): String? = when (container?.lowercase()) {
        "mkv", "matroska", "mka" -> "video/x-matroska"
        "mp4", "m4v", "mov" -> "video/mp4"
        "webm" -> "video/webm"
        "avi" -> "video/x-msvideo"
        "ts", "m2ts", "mpegts" -> "video/mp2t"
        else -> null
    }

    /** The video's dynamic range, as the policy reads it. */
    fun rangeOf(video: MediaStream?): DynamicRange = when {
        video == null -> DynamicRange.Sdr
        video.dynamicRange != null && video.dynamicRange != DynamicRange.Unknown -> video.dynamicRange!!
        video.dvProfile != null -> DynamicRange.DolbyVision
        else -> DynamicRange.Sdr
    }
}

/**
 * The Dolby Vision guards, until the server sends Dolby Vision in its
 * streams (S4/S5): a session that would show wrong colours is asked for
 * again another way.
 *
 * - A remux of profile 5: TS segments lose its Dolby Vision signalling, and
 *   profile 5 has no base layer to fall back to, so the picture comes out
 *   green and purple. It is transcoded instead (tone-mapped).
 * - Direct play of profile 7 without a profile 7 decoder (or a quirk that
 *   adds one): Media3 has no fallback for its dual layer. A remux sends the
 *   HEVC base layer, HDR10, when the screen shows HDR10; else a transcode.
 */
object DvGuards {
    /** The mode to ask [info]'s session again with; `null` when it plays right. */
    fun retryMode(info: PlaybackInfo, file: MediaFile?, caps: CapsSnapshot?): PlaybackMode? {
        val video = PlaybackPlan.videoStream(file) ?: return null
        if (PlaybackPlan.rangeOf(video) != DynamicRange.DolbyVision) return null
        return when (video.dvProfile) {
            5 -> if (info.method == PlaybackMethod.Remux) PlaybackMode.Transcode else null
            7 -> {
                val report = caps?.report
                // Quirks (the Shield's) are in the snapshot's profiles already.
                val decodesP7 = report?.dolbyVisionProfiles.orEmpty().any { it.profile == 7 }
                when {
                    info.method != PlaybackMethod.Direct || decodesP7 -> null
                    DynamicRange.Hdr10 in report?.hdrTypes.orEmpty() || report?.hdr == true -> PlaybackMode.Remux
                    else -> PlaybackMode.Transcode
                }
            }
            else -> null
        }
    }
}
