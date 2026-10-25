package app.pley.android.player.engine

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.TrackSelectionParameters
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.util.Util
import app.pley.android.model.MediaStream
import app.pley.android.model.StreamKind

/** One of the player's track groups, as the mapper compares it with the server's streams. */
data class GroupInfo(
    /** Its position in Tracks.groups. */
    val group: Int,
    /** C.TRACK_TYPE_*. */
    val type: Int,
    val language: String? = null,
    /** The sample MIME type, or for subtitles parsed while extracting, the original one. */
    val mimeType: String? = null,
    val channels: Int = Format.NO_VALUE,
    /** Whether a renderer here plays it, without exceeding its capabilities. */
    val supported: Boolean = true,
)

/**
 * The server's streams of a file mapped to the player's track groups: by
 * order within each kind (the server numbers streams as ffprobe does,
 * Media3 lists the tracks it extracts in the same order), cross-checked on
 * language, codec and channels. A stream Media3 skipped (a codec it doesn't
 * extract) shifts the order, so a stream that fails the check is matched to
 * the one group of its kind that passes it, if exactly one does.
 * Subtitle files beside the media aren't in the container: the session
 * adds those itself.
 */
@OptIn(markerClass = [UnstableApi::class])
object TrackMapper {
    /** Server stream index → group position, and the indexes matched by order alone despite a failed check. */
    data class Mapping(val groups: Map<Int, Int>, val unsure: Set<Int> = emptySet()) {
        fun groupOf(streamIndex: Int): Int? = groups[streamIndex]
    }

    fun groupsOf(tracks: Tracks): List<GroupInfo> = tracks.groups.mapIndexed { i, group ->
        val format = group.getTrackFormat(0)
        GroupInfo(
            group = i,
            type = group.type,
            language = format.language,
            mimeType = mimeOf(format),
            channels = format.channelCount,
            supported = group.isSupported(false),
        )
    }

    fun map(streams: List<MediaStream>, groups: List<GroupInfo>): Mapping {
        val out = linkedMapOf<Int, Int>()
        val unsure = linkedSetOf<Int>()
        for ((kind, type) in KINDS) {
            val ours = streams.filter { it.kind == kind && !it.external && it.index < EXTERNAL_FROM }.sortedBy { it.index }
            val theirs = groups.filter { it.type == type }
            val taken = mutableSetOf<Int>()
            val pending = mutableListOf<Pair<MediaStream, GroupInfo?>>()
            ours.forEachIndexed { i, stream ->
                val byOrder = theirs.getOrNull(i)
                if (byOrder != null && agrees(stream, byOrder)) {
                    out[stream.index] = byOrder.group
                    taken += byOrder.group
                } else {
                    pending += stream to byOrder
                }
            }
            // Then the one group that agrees, for each stream that has one…
            val left = pending.filter { (stream, _) ->
                val candidates = theirs.filter { it.group !in taken && agrees(stream, it) }
                if (candidates.size == 1) {
                    out[stream.index] = candidates[0].group
                    taken += candidates[0].group
                    false
                } else {
                    true
                }
            }
            // …and the order for the rest, flagged.
            for ((stream, byOrder) in left) {
                if (byOrder != null && byOrder.group !in taken) {
                    out[stream.index] = byOrder.group
                    taken += byOrder.group
                    unsure += stream.index
                }
            }
        }
        return Mapping(out, unsure)
    }

    /** Whether a server stream and a group can be the same: language, codec and channels, where both know them. */
    fun agrees(stream: MediaStream, group: GroupInfo): Boolean {
        val ourLanguage = stream.language?.let(::normalize)
        val theirLanguage = group.language?.let(::normalize)
        if (ourLanguage != null && theirLanguage != null && ourLanguage != theirLanguage) return false
        val mimes = MIMES[stream.codec.lowercase()]
        if (mimes != null && group.mimeType != null && group.mimeType !in mimes) return false
        val channels = stream.channels
        if (channels != null && channels > 0 && group.channels > 0 && channels != group.channels) return false
        return true
    }

    /**
     * [params] with the audio stream [audioIndex] and the subtitle stream
     * [subtitleIndex] chosen: `null` subtitles turn text off altogether, so
     * a default-flagged track doesn't show anyway. An index with no group
     * leaves that kind as it was.
     */
    fun select(
        params: TrackSelectionParameters,
        tracks: Tracks,
        mapping: Mapping,
        audioIndex: Int?,
        subtitleIndex: Int?,
        subtitlesOff: Boolean,
    ): TrackSelectionParameters {
        val builder = params.buildUpon()
        audioIndex?.let(mapping::groupOf)?.let { tracks.groups.getOrNull(it) }?.let { group ->
            builder.setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, 0))
        }
        if (subtitlesOff) {
            builder.clearOverridesOfType(C.TRACK_TYPE_TEXT).setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
        } else {
            subtitleIndex?.let(mapping::groupOf)?.let { tracks.groups.getOrNull(it) }?.let { group ->
                builder.setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
                    .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, 0))
            }
        }
        return builder.build()
    }

    /** Chooses on [player] at once (direct play): see [select]. */
    fun apply(player: Player, streams: List<MediaStream>, audioIndex: Int?, subtitleIndex: Int?, subtitlesOff: Boolean): Mapping {
        val tracks = player.currentTracks
        val mapping = map(streams, groupsOf(tracks))
        player.trackSelectionParameters = select(player.trackSelectionParameters, tracks, mapping, audioIndex, subtitleIndex, subtitlesOff)
        return mapping
    }

    private fun normalize(language: String): String? =
        language.takeUnless { it.isBlank() || it.equals("und", true) }?.let { Util.normalizeLanguageCode(it) ?: it.lowercase() }

    /** Subtitles parsed while extracting carry the original MIME type in their codecs. */
    private fun mimeOf(format: Format): String? =
        if (format.sampleMimeType == MimeTypes.APPLICATION_MEDIA3_CUES) format.codecs ?: format.sampleMimeType else format.sampleMimeType

    /** Subtitle files beside the media are numbered from 1000. */
    private const val EXTERNAL_FROM = 1000

    private val KINDS = listOf(
        StreamKind.Video to C.TRACK_TYPE_VIDEO,
        StreamKind.Audio to C.TRACK_TYPE_AUDIO,
        StreamKind.Subtitle to C.TRACK_TYPE_TEXT,
    )

    /** ffprobe's codec names to the MIME types Media3 gives them. */
    private val MIMES: Map<String, Set<String>> = mapOf(
        "h264" to setOf(MimeTypes.VIDEO_H264),
        "hevc" to setOf(MimeTypes.VIDEO_H265, MimeTypes.VIDEO_DOLBY_VISION),
        "av1" to setOf(MimeTypes.VIDEO_AV1, MimeTypes.VIDEO_DOLBY_VISION),
        "vp9" to setOf(MimeTypes.VIDEO_VP9),
        "vp8" to setOf(MimeTypes.VIDEO_VP8),
        "mpeg2video" to setOf(MimeTypes.VIDEO_MPEG2),
        "aac" to setOf(MimeTypes.AUDIO_AAC),
        "mp3" to setOf(MimeTypes.AUDIO_MPEG),
        "mp2" to setOf(MimeTypes.AUDIO_MPEG_L2),
        "ac3" to setOf(MimeTypes.AUDIO_AC3),
        "eac3" to setOf(MimeTypes.AUDIO_E_AC3, MimeTypes.AUDIO_E_AC3_JOC),
        "truehd" to setOf(MimeTypes.AUDIO_TRUEHD),
        "dts" to setOf(MimeTypes.AUDIO_DTS, MimeTypes.AUDIO_DTS_HD, MimeTypes.AUDIO_DTS_EXPRESS, MimeTypes.AUDIO_DTS_X),
        "flac" to setOf(MimeTypes.AUDIO_FLAC),
        "opus" to setOf(MimeTypes.AUDIO_OPUS),
        "vorbis" to setOf(MimeTypes.AUDIO_VORBIS),
        "alac" to setOf(MimeTypes.AUDIO_ALAC),
        "pcm_s16le" to setOf(MimeTypes.AUDIO_RAW),
        "pcm_s24le" to setOf(MimeTypes.AUDIO_RAW),
        "subrip" to setOf(MimeTypes.APPLICATION_SUBRIP),
        "srt" to setOf(MimeTypes.APPLICATION_SUBRIP),
        "ass" to setOf(MimeTypes.TEXT_SSA),
        "ssa" to setOf(MimeTypes.TEXT_SSA),
        "webvtt" to setOf(MimeTypes.TEXT_VTT),
        "mov_text" to setOf(MimeTypes.APPLICATION_TX3G),
        "hdmv_pgs_subtitle" to setOf(MimeTypes.APPLICATION_PGS),
        "dvd_subtitle" to setOf(MimeTypes.APPLICATION_VOBSUB),
        "dvb_subtitle" to setOf(MimeTypes.APPLICATION_DVBSUBS),
    )
}

/**
 * Notices a file whose video or audio the device can't play: a kind that
 * has groups, none of them playable here as they are (Media3 isn't allowed
 * to exceed the renderers' capabilities). The session then asks the server
 * to remux or transcode.
 */
object UnsupportedTrackWatcher {
    fun check(groups: List<GroupInfo>): NeedsServerHelp? {
        fun unplayable(type: Int): Boolean {
            val ofType = groups.filter { it.type == type }
            return ofType.isNotEmpty() && ofType.none { it.supported }
        }
        val video = unplayable(C.TRACK_TYPE_VIDEO)
        val audio = unplayable(C.TRACK_TYPE_AUDIO)
        return if (video || audio) NeedsServerHelp(video = video, audio = audio) else null
    }
}
