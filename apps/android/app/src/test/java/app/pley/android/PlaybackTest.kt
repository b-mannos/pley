package app.pley.android

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PlaybackTest {
    @Test
    fun requestsFromTheWebAppAreRead() {
        val request = PlayRequest.parse("""{"item_id":7,"start_ms":90000,"file_id":null,"audio":2,"sub":null,"title":"Taxi · Pilot"}""")
        assertEquals(PlayRequest(7, 90_000, null, 2, null, "Taxi · Pilot"), request)
        assertEquals(request, PlayRequest.parse(request!!.toJson()))
        assertNull(PlayRequest.parse("""{"item_id":0}"""))
        assertNull(PlayRequest.parse("nope"))
    }

    @Test
    fun theServerIsToldWhatThisDevicePlays() {
        val caps = Caps(listOf("mkv", "mp4"), listOf("h264", "hevc"), listOf("aac", "ac3"), hdr = true, dolbyVision = false)
        val body = Playback.body(PlayRequest(7, 0, 3, null, 5, "Heat"), caps, "android-1", "auto", 90_000)
        assertEquals(7, body.getLong("item_id"))
        assertEquals(3, body.getLong("file_id"))
        assertEquals(true, body.isNull("audio_index"))
        assertEquals(5, body.getLong("subtitle_index"))
        assertEquals(90_000, body.getLong("start_ms"))
        assertEquals("android-1", body.getString("client_id"))
        assertEquals("hevc", body.getJSONObject("caps").getJSONArray("video_codecs").getString(1))
        assertEquals(true, body.getJSONObject("caps").getBoolean("hdr"))
    }

    @Test
    fun answersAreRead() {
        val answer = JSONObject(
            """{"session_id":"s1","method":"transcode","url":"/api/playback/s1/index.m3u8","hls":true,"file_id":3,
               "duration_ms":1500000,"audio_index":1,"subtitle_index":null,"burn_subtitles":false,"reason":null,
               "subtitles":[{"index":4,"codec":"subrip","language":"en","title":null,"default":true,"forced":false,
                             "external":false,"text_based":true,"url":"/api/files/3/subtitles/4.vtt"}]}""",
        )
        val playback = Playback.parse(answer)
        assertEquals("s1", playback.sessionId)
        assertEquals(true, playback.hls)
        assertEquals(1_500_000, playback.durationMs)
        assertEquals(SubtitleTrack(4, "en", null, "/api/files/3/subtitles/4.vtt", default = true, forced = false, external = false), playback.subtitles.single())
    }
}
