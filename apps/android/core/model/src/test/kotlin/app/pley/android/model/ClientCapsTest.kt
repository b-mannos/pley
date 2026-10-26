package app.pley.android.model

import app.pley.android.model.caps.ClientCaps
import app.pley.android.model.caps.DeviceCaps
import app.pley.android.model.caps.DeviceForm
import app.pley.android.model.caps.DisplayCaps
import app.pley.android.model.caps.DolbyVisionCaps
import app.pley.android.model.caps.HlsCaps
import app.pley.android.model.caps.VideoDecoderCaps
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Test

class ClientCapsTest {
    private val v1 = ClientCaps(
        containers = listOf("mkv", "mp4"),
        videoCodecs = listOf("h264", "hevc"),
        audioCodecs = listOf("aac", "eac3"),
        hdr = true,
        dolbyVision = false,
    )

    /** The body apps/server/src/playback.rs `Caps` reads, field for field. */
    @Test
    fun versionOneIsWhatTheServerTakes() {
        assertEquals(
            """{"containers":["mkv","mp4"],"video_codecs":["h264","hevc"],"audio_codecs":["aac","eac3"],"hdr":true,"dolby_vision":false}""",
            PleyJson.encodeToString(ClientCaps.serializer(), v1),
        )
        assertEquals(
            """{"containers":[],"video_codecs":[],"audio_codecs":[],"hdr":false,"dolby_vision":false}""",
            PleyJson.encodeToString(ClientCaps.serializer(), ClientCaps()),
        )
    }

    @Test
    fun versionTwoTravelsWithVersionOne() {
        val v2 = v1.copy(
            capsVersion = ClientCaps.VERSION_2,
            video = listOf(
                VideoDecoderCaps("hevc", hardware = true, profiles = listOf("Main", "Main 10"), maxWidth = 3840, maxHeight = 2160, maxFps = 60.0, maxBitDepth = 10, maxLevel = 153),
            ),
            hdrTypes = listOf(DynamicRange.Hdr10, DynamicRange.Hlg),
            dolbyVisionProfiles = listOf(DolbyVisionCaps(8, maxLevel = 6)),
            audioDecode = listOf("aac", "ac3"),
            audioPassthrough = listOf("eac3", "truehd"),
            audioChannels = 8,
            subtitleCodecs = listOf("subrip", "ass"),
            audioTrackSwitching = true,
            pictureSubtitlesSideload = true,
            hls = HlsCaps(masterPlaylist = true, segments = listOf("ts", "fmp4")),
            display = DisplayCaps(3840, 2160, listOf(23.976f, 50f, 60f)),
            device = DeviceCaps(DeviceForm.Tv, tier = "high", fireTv = false, model = "Chromecast"),
        )
        val body = PleyJson.encodeToJsonElement(ClientCaps.serializer(), v2).jsonObject
        assertEquals(
            listOf(
                "containers", "video_codecs", "audio_codecs", "hdr", "dolby_vision", "caps_version", "video", "hdr_types",
                "dolby_vision_profiles", "audio_decode", "audio_passthrough", "audio_channels", "subtitle_codecs",
                "audio_track_switching", "picture_subtitles_sideload", "hls", "display", "device",
            ),
            body.keys.toList(),
        )
        assertEquals(
            setOf("codec", "hardware", "profiles", "max_width", "max_height", "max_fps", "max_bit_depth", "max_level"),
            body.getValue("video").jsonArray[0].jsonObject.keys,
        )
        assertEquals("""["hdr10","hlg"]""", body["hdr_types"].toString())
        assertEquals("""{"master_playlist":true,"segments":["ts","fmp4"]}""", body["hls"].toString())
        assertEquals("""{"form":"tv","tier":"high","fire_tv":false,"model":"Chromecast"}""", body["device"].toString())
        assertEquals("""{"width":3840,"height":2160,"refresh_rates":[23.976,50.0,60.0]}""", body["display"].toString())
        assertEquals(v2, PleyJson.decodeFromJsonElement(ClientCaps.serializer(), body))
    }
}
