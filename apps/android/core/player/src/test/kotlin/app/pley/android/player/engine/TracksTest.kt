package app.pley.android.player.engine

import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.TrackGroup
import androidx.media3.common.TrackSelectionParameters
import androidx.media3.common.Tracks
import app.pley.android.model.ItemDetail
import app.pley.android.model.MediaStream
import app.pley.android.model.StreamKind
import app.pley.android.testing.Fixtures
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class TracksTest {
    /** The movie fixture's MKV: HEVC; TrueHD 7.1 and AC3 commentary in English; PGS and SRT subtitles; a French SRT beside it. */
    private val mkv = Fixtures.decode<ItemDetail>("items/movie_detail.json").files.first { it.container == "mkv" }

    private fun group(i: Int, type: Int, mime: String, language: String? = null, channels: Int = Format.NO_VALUE, supported: Boolean = true) =
        GroupInfo(i, type, language, mime, channels, supported)

    /** What Media3 extracts from that MKV, in order. */
    private val extracted = listOf(
        group(0, C.TRACK_TYPE_VIDEO, MimeTypes.VIDEO_H265),
        group(1, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_TRUEHD, "en", 8),
        group(2, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_AC3, "en", 2),
        group(3, C.TRACK_TYPE_TEXT, MimeTypes.APPLICATION_PGS, "en"),
        group(4, C.TRACK_TYPE_TEXT, MimeTypes.APPLICATION_SUBRIP, "en"),
    )

    @Test
    fun streamsMapByOrderWithinTheirKind() {
        val mapping = TrackMapper.map(mkv.streams, extracted)
        assertEquals(mapOf(0 to 0, 1 to 1, 2 to 2, 3 to 3, 4 to 4), mapping.groups)
        assertEquals(emptySet<Int>(), mapping.unsure)
        assertNull("subtitle files beside the media aren't in the container", mapping.groupOf(1000))
    }

    @Test
    fun aStreamMedia3SkippedDoesNotShiftTheOthers() {
        // Media3 didn't extract the PGS track: the SRT is the first text group.
        val withoutPgs = listOf(extracted[0], extracted[1], extracted[2], extracted[4].copy(group = 3))
        val mapping = TrackMapper.map(mkv.streams, withoutPgs)
        assertEquals(3, mapping.groupOf(4))
        assertNull(mapping.groupOf(3))
    }

    @Test
    fun theCheckFindsAnAudioTrackOutOfPlace() {
        val streams = listOf(
            stream(1, StreamKind.Audio, "dts", "eng", 6),
            stream(2, StreamKind.Audio, "aac", "eng", 2),
            stream(3, StreamKind.Audio, "ac3", "fre", 6),
        )
        // The DTS track wasn't extracted.
        val groups = listOf(
            group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_AAC, "en", 2),
            group(1, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_AC3, "fr", 6),
        )
        val mapping = TrackMapper.map(streams, groups)
        assertEquals(mapOf(2 to 0, 3 to 1), mapping.groups)
    }

    @Test
    fun orderDecidesWhenNothingElseDoesButSaysSo() {
        val streams = listOf(stream(1, StreamKind.Audio, "aac", "eng", 2))
        val groups = listOf(group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_AAC, "de", 2))
        val mapping = TrackMapper.map(streams, groups)
        assertEquals(0, mapping.groupOf(1))
        assertEquals(setOf(1), mapping.unsure)
    }

    @Test
    fun languagesCodecsAndChannelsAreCompared() {
        val eng = stream(1, StreamKind.Audio, "eac3", "eng", 6)
        assertTrue(TrackMapper.agrees(eng, group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_E_AC3_JOC, "en", 6)))
        assertTrue(TrackMapper.agrees(eng, group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_E_AC3, null, Format.NO_VALUE)))
        assertTrue("und says nothing", TrackMapper.agrees(eng.copy(language = "und"), group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_E_AC3, "fr", 6)))
        assertFalse(TrackMapper.agrees(eng, group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_E_AC3, "fr", 6)))
        assertFalse(TrackMapper.agrees(eng, group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_AC3, "en", 6)))
        assertFalse(TrackMapper.agrees(eng, group(0, C.TRACK_TYPE_AUDIO, MimeTypes.AUDIO_E_AC3, "en", 2)))
        assertTrue(TrackMapper.agrees(stream(2, StreamKind.Subtitle, "subrip", "fre"), group(0, C.TRACK_TYPE_TEXT, MimeTypes.APPLICATION_SUBRIP, "fr")))
    }

    @Test
    fun parsedSubtitlesAreComparedByTheirOriginalType() {
        val srt = Format.Builder()
            .setSampleMimeType(MimeTypes.APPLICATION_MEDIA3_CUES)
            .setCodecs(MimeTypes.APPLICATION_SUBRIP)
            .setLanguage("eng")
            .build()
        val groups = TrackMapper.groupsOf(tracksOf(C.TRACK_TYPE_TEXT to srt))
        assertEquals(MimeTypes.APPLICATION_SUBRIP, groups.single().mimeType)
    }

    @Test
    fun choosingAudioAndSubtitlesAndTurningSubtitlesOff() {
        val audio1 = Format.Builder().setId("a1").setSampleMimeType(MimeTypes.AUDIO_TRUEHD).setLanguage("en").setChannelCount(8).build()
        val audio2 = Format.Builder().setId("a2").setSampleMimeType(MimeTypes.AUDIO_AC3).setLanguage("en").setChannelCount(2).build()
        val text = Format.Builder().setId("t1").setSampleMimeType(MimeTypes.APPLICATION_SUBRIP).setLanguage("en").build()
        val tracks = tracksOf(C.TRACK_TYPE_AUDIO to audio1, C.TRACK_TYPE_AUDIO to audio2, C.TRACK_TYPE_TEXT to text)
        val streams = listOf(
            stream(1, StreamKind.Audio, "truehd", "eng", 8),
            stream(2, StreamKind.Audio, "ac3", "eng", 2),
            stream(3, StreamKind.Subtitle, "subrip", "eng"),
        )
        val mapping = TrackMapper.map(streams, TrackMapper.groupsOf(tracks))
        val base = TrackSelectionParameters.Builder().build()

        val commentary = TrackMapper.select(base, tracks, mapping, audioIndex = 2, subtitleIndex = 3, subtitlesOff = false)
        assertEquals(tracks.groups[1].mediaTrackGroup, commentary.overrides.values.first { it.type == C.TRACK_TYPE_AUDIO }.mediaTrackGroup)
        assertEquals(tracks.groups[2].mediaTrackGroup, commentary.overrides.values.first { it.type == C.TRACK_TYPE_TEXT }.mediaTrackGroup)
        assertFalse(C.TRACK_TYPE_TEXT in commentary.disabledTrackTypes)

        val off = TrackMapper.select(commentary, tracks, mapping, audioIndex = null, subtitleIndex = null, subtitlesOff = true)
        assertTrue("Off is off, whatever the file's default", C.TRACK_TYPE_TEXT in off.disabledTrackTypes)
        assertTrue(off.overrides.values.none { it.type == C.TRACK_TYPE_TEXT })
        assertEquals("audio stays as chosen", tracks.groups[1].mediaTrackGroup, off.overrides.values.first { it.type == C.TRACK_TYPE_AUDIO }.mediaTrackGroup)
    }

    @Test
    fun unplayableVideoOrAudioNeedsTheServer() {
        assertNull(UnsupportedTrackWatcher.check(extracted))
        assertNull("no subtitles is fine", UnsupportedTrackWatcher.check(extracted.map { if (it.type == C.TRACK_TYPE_TEXT) it.copy(supported = false) else it }))
        val noTrueHdNoAc3 = extracted.map { if (it.type == C.TRACK_TYPE_AUDIO) it.copy(supported = false) else it }
        assertEquals(NeedsServerHelp(video = false, audio = true), UnsupportedTrackWatcher.check(noTrueHdNoAc3))
        val oneAudioPlays = extracted.map { if (it.group == 1) it.copy(supported = false) else it }
        assertNull(UnsupportedTrackWatcher.check(oneAudioPlays))
        val noVideo = extracted.map { if (it.type == C.TRACK_TYPE_VIDEO) it.copy(supported = false) else it }
        assertEquals(NeedsServerHelp(video = true, audio = false), UnsupportedTrackWatcher.check(noVideo))
        assertNull("an audio-only file has no video to miss", UnsupportedTrackWatcher.check(extracted.filter { it.type == C.TRACK_TYPE_AUDIO }))
    }

    @Test
    fun unsupportedTracksReadFromMedia3() {
        val hevc = Format.Builder().setSampleMimeType(MimeTypes.VIDEO_H265).build()
        val tracks = Tracks(
            listOf(
                Tracks.Group(TrackGroup(hevc), false, intArrayOf(C.FORMAT_EXCEEDS_CAPABILITIES), booleanArrayOf(false)),
            ),
        )
        assertEquals(NeedsServerHelp(video = true, audio = false), UnsupportedTrackWatcher.check(TrackMapper.groupsOf(tracks)))
    }

    private fun stream(index: Int, kind: StreamKind, codec: String, language: String? = null, channels: Int? = null) =
        MediaStream(index = index, kind = kind, codec = codec, language = language, channels = channels)

    private fun tracksOf(vararg groups: Pair<Int, Format>): Tracks = Tracks(
        groups.mapIndexed { i, (_, format) ->
            Tracks.Group(TrackGroup("g$i", format), false, intArrayOf(C.FORMAT_HANDLED), booleanArrayOf(false))
        },
    )
}
