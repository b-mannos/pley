package app.pley.android.player.session

import androidx.media3.common.Format
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.HttpDataSource
import androidx.media3.exoplayer.ExoPlaybackException
import app.pley.android.model.DynamicRange
import app.pley.android.model.JobState
import app.pley.android.model.MediaFile
import app.pley.android.model.MediaStream
import app.pley.android.model.PlaybackInfo
import app.pley.android.model.PlaybackMethod
import app.pley.android.model.StreamAction
import app.pley.android.model.StreamKind
import app.pley.android.model.TranscodeInfo
import app.pley.android.player.PlayQueue
import app.pley.android.player.PlayRequest
import app.pley.android.player.SubtitleDelivery
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

/** The smaller parts: Media3's errors sorted, saved state, the details' words. */
@RunWith(RobolectricTestRunner::class)
@androidx.annotation.OptIn(UnstableApi::class)
class PiecesTest {
    private val texts = ResourcePlayerTexts(RuntimeEnvironment.getApplication())

    private fun error(code: Int, cause: Throwable? = null) = PlaybackException("boom", cause, code)

    @Test
    fun `Media3's errors are sorted for the recovery ladder`() {
        val missing = HttpDataSource.InvalidResponseCodeException(404, "Not Found", null, emptyMap(), DataSpec.Builder().setUri("http://x").build(), ByteArray(0))
        assertEquals(404, (Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS, missing)) as EngineError.Http).status)
        // 410: the server stopped the session, with its message.
        val body = """{"error":"session_stopped","message":"Maintenance"}""".toByteArray()
        val gone = HttpDataSource.InvalidResponseCodeException(410, "Gone", null, emptyMap(), DataSpec.Builder().setUri("http://x").build(), body)
        assertEquals("Maintenance", (Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS, gone)) as EngineError.Stopped).message)
        assertEquals(EngineError.Network::class, Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED))::class)
        assertEquals(EngineError.Playlist::class, Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_PARSING_MANIFEST_MALFORMED))::class)
        val container = Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED)) as EngineError.Unsupported
        assertEquals(true, container.container)
        assertEquals(EngineError.Unsupported(audio = true, container = false, detail = "ERROR_CODE_AUDIO_TRACK_INIT_FAILED: boom"), Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_AUDIO_TRACK_INIT_FAILED)))
        val audioDecoder = ExoPlaybackException.createForRenderer(
            RuntimeException("no decoder"), "audio", 1, Format.Builder().setSampleMimeType(MimeTypes.AUDIO_E_AC3).build(),
            0, null, false, PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
        )
        assertEquals(true, (Media3EngineLink.errorOf(audioDecoder) as EngineError.Unsupported).audio)
        val videoDecoder = ExoPlaybackException.createForRenderer(
            RuntimeException("no decoder"), "video", 0, Format.Builder().setSampleMimeType(MimeTypes.VIDEO_DOLBY_VISION).build(),
            0, null, false, PlaybackException.ERROR_CODE_DECODING_FORMAT_EXCEEDS_CAPABILITIES,
        )
        assertEquals(false, (Media3EngineLink.errorOf(videoDecoder) as EngineError.Unsupported).audio)
        assertEquals(EngineError.Other::class, Media3EngineLink.errorOf(error(PlaybackException.ERROR_CODE_DRM_UNSPECIFIED))::class)
    }

    @Test
    fun `saved state comes back from a Bundle and plays again from where it was`() {
        val saved = SavedPlaybackState(20_001, 30_001, 61_000, 1, null, "4m", PlayQueue.Playlist(1))
        val back = SavedPlaybackState.fromBundle(saved.toBundle())
        assertEquals(saved, back)
        assertEquals(PlayRequest(20_001, fileId = 30_001, startMs = 61_000, audioIndex = 1, queue = PlayQueue.Playlist(1)), back!!.toPlayRequest())
        assertEquals(PlayQueue.Collection(2), SavedPlaybackState.fromBundle(saved.copy(queue = PlayQueue.Collection(2)).toBundle())!!.queue)
        assertNull(SavedPlaybackState.fromBundle(null))
        assertNull(SavedPlaybackState.fromBundle(android.os.Bundle()))
    }

    @Test
    fun `the details read as the web's`() {
        val video = MediaStream(0, StreamKind.Video, "hevc", width = 3840, height = 2160, dynamicRange = DynamicRange.DolbyVision, dvProfile = 8, dvBlCompat = 1)
        assertEquals("HEVC 4K Dolby Vision P8.1", PlaybackDetails.videoLabel(video))
        assertEquals("HEVC 4K Dolby Vision P5", PlaybackDetails.videoLabel(video.copy(dvProfile = 5, dvBlCompat = 0)))
        assertEquals("TrueHD 7.1", PlaybackDetails.audioLabel(MediaStream(1, StreamKind.Audio, "truehd", channels = 8)))
        assertEquals("H.264 1080p", PlaybackDetails.simplified("H.264 1920×1080"))
        val live = TranscodeInfo(jobId = "j", state = JobState.Running, encoder = "h264_nvenc", pipeline = "gpu")
        assertEquals("NVIDIA NVENC (GPU)", PlaybackDetails.encoderLabel(live))

        val audio = MediaStream(1, StreamKind.Audio, "eac3", channels = 6, default = true)
        val file = MediaFile(id = 1, path = "a.mkv", bitrate = 62_000_000, streams = listOf(video, audio))
        val info = PlaybackInfo("s", PlaybackMethod.Remux, "/x", true, 1, audioConverted = true, reasons = listOf("EAC3 audio isn't supported by this player."))
        val remux = PlaybackDetails.build(
            texts, info, file, null, null, 10_000,
            TranscodeInfo("j", video = StreamAction.Copy, audio = StreamAction.Transcode, audioIn = "EAC3 5.1", audioOut = "AAC 2.0", bitrateOut = 40_000_000, speed = 12.0, transcodedMs = 70_000),
            listOf("Video decoder: c2.qti.hevc.decoder"),
        )
        assertEquals("Direct stream · Audio transcoding", remux.method)
        assertEquals("EAC3 audio isn't supported by this player.", remux.why)
        assertNull(remux.pipeline)
        assertEquals(
            listOf(
                "Video" to "HEVC 4K Dolby Vision P8.1",
                "Audio" to "EAC3 5.1 → AAC 2.0",
                "Bitrate" to "62 Mbps → 40 Mbps",
                "Speed" to "12.0× real time · 60 s ready",
            ),
            remux.rows.map { it.label to it.value },
        )
        val direct = PlaybackDetails.build(texts, info.copy(method = PlaybackMethod.Direct, hls = false), file, null, "ignored", 0, null, emptyList())
        assertEquals("Direct play", direct.method)
        assertEquals("The original file, as it is.", direct.why)
        assertEquals(listOf("Video", "Audio", "Bitrate"), direct.rows.map { it.label })
        // A transcode the player fell back to says why.
        val fallback = PlaybackDetails.build(texts, info.copy(method = PlaybackMethod.Transcode, audioConverted = false), file, null, "The original didn't play smoothly on this device.", 0, null, emptyList())
        assertEquals("The original didn't play smoothly on this device.", fallback.why)
        assertEquals("HEVC 4K Dolby Vision P8.1 → …", fallback.rows.first().value)
    }

    @Test
    fun `the details carry the file and the streams in play, and how subtitles reach the screen`() {
        val video = MediaStream(0, StreamKind.Video, "hevc", width = 3840, height = 2160)
        val audio = MediaStream(1, StreamKind.Audio, "eac3", channels = 6)
        val french = MediaStream(2, StreamKind.Audio, "ac3", channels = 6, language = "fre")
        val srt = MediaStream(3, StreamKind.Subtitle, "subrip", textBased = true)
        val pgs = MediaStream(4, StreamKind.Subtitle, "hdmv_pgs_subtitle", textBased = false)
        val beside = MediaStream(1000, StreamKind.Subtitle, "subrip", textBased = true, external = true)
        val file = MediaFile(id = 1, path = "a.mkv", streams = listOf(video, audio, french, srt, pgs, beside))
        val direct = PlaybackInfo("s", PlaybackMethod.Direct, "/x", false, 1)
        val details = PlaybackDetails.build(texts, direct, file, 2, null, 0, null, emptyList(), subtitle = srt)
        assertEquals(file, details.file)
        assertEquals(video, details.video)
        assertEquals(french, details.audio)
        assertEquals(srt, details.subtitle)
        assertEquals(PlaybackMethod.Direct, details.playbackMethod)
        assertEquals(SubtitleDelivery.File, details.subtitleDelivery)
        assertEquals(SubtitleDelivery.Server, PlaybackDetails.deliveryOf(direct, beside))
        val hls = direct.copy(method = PlaybackMethod.Transcode, hls = true)
        assertEquals(SubtitleDelivery.Server, PlaybackDetails.deliveryOf(hls, srt))
        assertEquals(SubtitleDelivery.BurnedIn, PlaybackDetails.deliveryOf(hls.copy(burnSubtitles = true, subtitleIndex = 4), pgs))
        assertEquals(SubtitleDelivery.ServerPictures, PlaybackDetails.deliveryOf(hls, pgs))
        assertNull(PlaybackDetails.build(texts, direct, file, null, null, 0, null, emptyList()).subtitleDelivery)
    }

    @Test
    fun `the texts are the web's words`() {
        assertEquals("30 Mbps 4K", texts.quality(Qualities.byId("30m")))
        assertEquals("Original", texts.quality(Qualities.ORIGINAL))
        assertEquals("8 Mbps 1080p (auto)", texts.autoQuality(Qualities.byId("8m")))
        assertEquals("Up next · S1 E2", texts.upNext("S1 E2"))
        assertEquals("Up next", texts.upNext(null))
        assertEquals("Playback failed (ERROR_CODE_DECODING_FAILED).", texts.failure(Failure.Engine, "ERROR_CODE_DECODING_FAILED: boom"))
    }
}
