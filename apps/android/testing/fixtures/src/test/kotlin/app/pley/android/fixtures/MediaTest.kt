package app.pley.android.fixtures

import app.pley.android.model.ItemDetail
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Files with Range, the HLS stream, subtitles in parts, and pictures per width bucket. */
class MediaTest {
    private val stream = "/api/files/${FixtureIds.SAMPLE_1080P_FILE}/stream"

    @Test
    fun rangeGives206WithTheRightContentRange() = withServer { server ->
        val app = Client(server, server.accessToken())
        val whole = app.get(stream)
        assertEquals(200, whole.code)
        assertEquals("bytes", whole.header("Accept-Ranges"))
        assertEquals("video/mp4", whole.header("Content-Type"))
        val size = whole.bytes.size
        assertTrue(size > 1_000_000)

        val part = app.get(stream, mapOf("Range" to "bytes=100-1099"))
        assertEquals(206, part.code)
        assertEquals("bytes 100-1099/$size", part.header("Content-Range"))
        assertArrayEquals(whole.bytes.copyOfRange(100, 1_100), part.bytes)

        val open = app.get(stream, mapOf("Range" to "bytes=${size - 10}-"))
        assertEquals("bytes ${size - 10}-${size - 1}/$size", open.header("Content-Range"))
        assertEquals(10, open.bytes.size)

        val suffix = app.get(stream, mapOf("Range" to "bytes=-500"))
        assertEquals("bytes ${size - 500}-${size - 1}/$size", suffix.header("Content-Range"))
        assertArrayEquals(whole.bytes.copyOfRange(size - 500, size), suffix.bytes)

        val head = okhttp3.Request.Builder().url(server.url + stream).head().header("Authorization", "Bearer ${app.token}").build()
        app.http.newCall(head).execute().use {
            assertEquals(200, it.code)
            assertEquals(size.toString(), it.header("Content-Length"))
        }

        val past = app.get(stream, mapOf("Range" to "bytes=$size-"))
        assertEquals(416, past.code)
        assertEquals("bytes */$size", past.header("Content-Range"))
    }

    @Test
    fun theHdrClipIsMatroska() = withServer { server ->
        val app = Client(server, server.accessToken())
        val head = app.get("/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/stream", mapOf("Range" to "bytes=0-3"))
        assertEquals(206, head.code)
        assertEquals("video/x-matroska", head.header("Content-Type"))
        // EBML's magic number.
        assertArrayEquals(byteArrayOf(0x1A, 0x45, 0xDF.toByte(), 0xA3.toByte()), head.bytes)
    }

    @Test
    fun theHlsStream() = withServer { server ->
        val app = Client(server, server.accessToken())
        val info = app.send("POST", "/api/playback", """{"item_id":${FixtureIds.SAMPLE_1080P},"mode":"transcode","caps":{}}""")
        val session = info.field("session_id")!!
        val playlist = app.get("/api/playback/$session/index.m3u8")
        assertEquals("application/vnd.apple.mpegurl", playlist.header("Content-Type"))
        assertTrue(playlist.text.startsWith("#EXTM3U"))
        val segments = playlist.text.lines().filter { it.endsWith(".ts") }
        assertEquals(listOf("0.ts", "1.ts", "2.ts", "3.ts", "4.ts"), segments)
        val segment = app.get("/api/playback/$session/0.ts")
        assertEquals("video/mp2t", segment.header("Content-Type"))
        // MPEG-TS packets start with 0x47.
        assertEquals(0x47.toByte(), segment.bytes[0])
        assertEquals(404, app.get("/api/playback/$session/9.ts").code)
    }

    @Test
    fun oneSubtitleTrackComesPartialThenWhole() = withServer { server ->
        val app = Client(server, server.accessToken())
        val path = "/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/subtitles/${FixtureIds.SAMPLE_HDR10_SUBTITLE}.vtt"
        val first = app.get(path)
        assertEquals(200, first.code)
        assertEquals("0", first.header("X-Pley-Complete"))
        assertTrue(first.text.startsWith("WEBVTT"))
        val second = app.get(path)
        assertNull(second.header("X-Pley-Complete"))
        assertTrue(second.text.length > first.text.length)
        assertTrue(second.text.startsWith(first.text))
        assertEquals(4, second.text.split("-->").size - 1)

        val french = app.get("/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/subtitles/${FixtureIds.SAMPLE_HDR10_EXTERNAL_SUBTITLE}.vtt")
        assertNull(french.header("X-Pley-Complete"))
        assertTrue("Rien ici" in french.text)
        assertEquals(404, app.get("/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/subtitles/7.vtt").code)
    }

    @Test
    fun picturesComeInTheServersWidths() = withServer { server ->
        val app = Client(server, server.accessToken())
        fun size(path: String): Pair<Int, Int> {
            val answer = app.get(path)
            assertEquals(path, 200, answer.code)
            assertEquals("image/png", answer.header("Content-Type"))
            val b = answer.bytes
            fun int(at: Int) = ((b[at].toInt() and 0xFF) shl 24) or ((b[at + 1].toInt() and 0xFF) shl 16) or
                ((b[at + 2].toInt() and 0xFF) shl 8) or (b[at + 3].toInt() and 0xFF)
            return int(16) to int(20)
        }
        assertEquals(240 to 360, size("/api/items/1/images/poster?w=200"))
        assertEquals(1920 to 1080, size("/api/items/1/images/backdrop?w=5000"))
        assertEquals(320 to 180, size("/api/items/${FixtureIds.SHOW_S1E1}/images/thumb?w=320"))
        assertEquals(160 to 160, size("/api/items/${FixtureIds.ALBUM}/images/poster?w=100"))
        assertEquals(256 to 256, size("/api/users/${FixtureIds.ALEX}/avatar"))
        assertEquals(480 to 720, size("/api/collections/${FixtureIds.COLLECTION}/images/poster?w=480"))
        assertEquals(3200 to 1800, size("/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/trickplay/0.jpg?v=x"))
        assertEquals(320 to 180, size("/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/chapters/1.jpg"))
        val person = app.read("/api/items/1", ItemDetail.serializer()).cast.first { it.photoUrl != null }
        assertEquals(160 to 240, size("/api/people/${person.id}/photo?w=160"))
        assertEquals(404, app.get("/api/users/${FixtureIds.SAM}/avatar").code)
        assertEquals(404, app.get("/api/files/${FixtureIds.SAMPLE_HDR10_FILE}/trickplay/1.jpg").code)
    }
}
