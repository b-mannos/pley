package app.pley.android.fixtures

import app.pley.android.model.ItemDetail
import app.pley.android.model.ServerEvent
import app.pley.android.model.StreamKind
import okhttp3.Request
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Picture subtitles as .mks, and sessions stopped from elsewhere (DELETE /api/sessions/{id}). */
class StopAndPicturesTest {
    @Test
    fun pictureSubtitlesComeAloneInMatroska() = withServer { server ->
        val app = Client(server, server.accessToken())
        val (file, picture, text) = (1L..200L).firstNotNullOf { id ->
            val answer = app.get("/api/items/$id")
            if (answer.code != 200) return@firstNotNullOf null
            val f = answer.decode(ItemDetail.serializer()).files.firstOrNull() ?: return@firstNotNullOf null
            val p = f.streams.firstOrNull { it.kind == StreamKind.Subtitle && it.textBased == false } ?: return@firstNotNullOf null
            val t = f.streams.first { it.kind == StreamKind.Subtitle && it.textBased == true }
            Triple(f, p, t)
        }
        val mks = app.get("/api/files/${file.id}/subtitles/${picture.index}.mks")
        assertEquals(200, mks.code)
        assertEquals("video/x-matroska", mks.header("Content-Type"))
        assertArrayEquals(byteArrayOf(0x1A, 0x45, 0xDF.toByte(), 0xA3.toByte()), mks.bytes.copyOfRange(0, 4))
        assertTrue(String(mks.bytes, Charsets.ISO_8859_1).contains("S_HDMV/PGS"))
        // Text has its WebVTT, not an .mks.
        assertEquals(404, app.get("/api/files/${file.id}/subtitles/${text.index}.mks").code)
    }

    @Test
    fun aStoppedSessionHearsWhyAndItsStreamAnswers410() = withServer { server ->
        val robin = Client(server, server.accessToken(FixtureIds.ROBIN))
        val alex = Client(server, server.accessToken())
        val info = robin.send("POST", "/api/playback", """{"item_id":${FixtureIds.SAMPLE_1080P},"mode":"transcode","caps":{"picture_subtitles_sideload":true}}""")
        assertEquals(200, info.code)
        val session = info.field("session_id")!!
        assertEquals(200, robin.get("/api/playback/$session/0.ts").code)

        val request = Request.Builder().url("${server.url}/api/events").header("Authorization", "Bearer ${robin.token}").build()
        robin.http.newCall(request).execute().use { response ->
            val source = response.body.source()
            val deadline = System.currentTimeMillis() + 5_000
            while (server.eventListeners == 0 && System.currentTimeMillis() < deadline) Thread.sleep(10)
            // Robin can't stop Alex's sessions, but Alex, the administrator, stops Robin's.
            assertEquals(204, alex.send("DELETE", "/api/sessions/$session", """{"message":"Server maintenance"}""").code)
            var stopped: ServerEvent.SessionStopped? = null
            while (stopped == null) {
                val line = source.readUtf8Line() ?: break
                if (line.startsWith("data: ")) stopped = ServerEvent.parse(line.removePrefix("data: ")) as? ServerEvent.SessionStopped
            }
            assertEquals(ServerEvent.SessionStopped(session, "Server maintenance", byAdmin = true), stopped)
        }
        val gone = robin.get("/api/playback/$session/1.ts")
        assertEquals(410, gone.code)
        assertEquals("session_stopped", gone.field("error"))
        assertEquals("Server maintenance", gone.field("message"))
        // Stopping it again is fine; an unknown one isn't.
        assertEquals(204, robin.send("DELETE", "/api/sessions/$session").code)
        assertEquals(404, robin.send("DELETE", "/api/sessions/nope").code)
    }

    @Test
    fun onlyTheirOwnForOthers() = withServer { server ->
        val robin = Client(server, server.accessToken(FixtureIds.ROBIN))
        val alex = Client(server, server.accessToken())
        val session = alex.send("POST", "/api/playback", """{"item_id":${FixtureIds.SAMPLE_1080P},"mode":"transcode","caps":{}}""").field("session_id")!!
        assertEquals(403, robin.send("DELETE", "/api/sessions/$session").code)
        assertEquals(200, alex.get("/api/playback/$session/0.ts").code)
        assertEquals(204, alex.send("DELETE", "/api/sessions/$session").code)
        assertEquals(410, alex.get("/api/playback/$session/0.ts").code)
    }
}
