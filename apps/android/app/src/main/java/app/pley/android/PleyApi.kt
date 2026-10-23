package app.pley.android

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** What the web app asks the player to play (src/lib/shell.ts over there). */
data class PlayRequest(
    val itemId: Long,
    val startMs: Long,
    val fileId: Long?,
    val audio: Long?,
    val sub: Long?,
    val title: String,
) {
    fun toJson(): String = JSONObject()
        .put("item_id", itemId)
        .put("start_ms", startMs)
        .put("file_id", fileId ?: JSONObject.NULL)
        .put("audio", audio ?: JSONObject.NULL)
        .put("sub", sub ?: JSONObject.NULL)
        .put("title", title)
        .toString()

    companion object {
        fun parse(json: String): PlayRequest? = try {
            val o = JSONObject(json)
            val id = o.optLong("item_id", -1)
            if (id <= 0) {
                null
            } else {
                PlayRequest(
                    itemId = id,
                    startMs = o.optLong("start_ms", 0).coerceAtLeast(0),
                    fileId = o.optNullableLong("file_id"),
                    audio = o.optNullableLong("audio"),
                    sub = o.optNullableLong("sub"),
                    title = o.optString("title"),
                )
            }
        } catch (_: org.json.JSONException) {
            null
        }
    }
}

private fun JSONObject.optNullableLong(name: String): Long? = if (has(name) && !isNull(name)) optLong(name) else null

/** What a device plays as it is, for the server's direct play / transcode decision. */
data class Caps(
    val containers: List<String>,
    val videoCodecs: List<String>,
    val audioCodecs: List<String>,
    val hdr: Boolean,
    val dolbyVision: Boolean,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("containers", JSONArray(containers))
        .put("video_codecs", JSONArray(videoCodecs))
        .put("audio_codecs", JSONArray(audioCodecs))
        .put("hdr", hdr)
        .put("dolby_vision", dolbyVision)
}

data class SubtitleTrack(
    val index: Long,
    val language: String?,
    val title: String?,
    val url: String?,
    val default: Boolean,
    val forced: Boolean,
    /** A sidecar file next to the video, not a track inside it. */
    val external: Boolean,
)

/** The server's answer to `POST /api/playback`. */
data class Playback(
    val sessionId: String,
    val method: String,
    val url: String,
    val hls: Boolean,
    val durationMs: Long,
    val subtitles: List<SubtitleTrack>,
) {
    companion object {
        fun parse(o: JSONObject): Playback {
            val subs = o.optJSONArray("subtitles") ?: JSONArray()
            return Playback(
                sessionId = o.getString("session_id"),
                method = o.getString("method"),
                url = o.getString("url"),
                hls = o.optBoolean("hls"),
                durationMs = o.optLong("duration_ms"),
                subtitles = (0 until subs.length()).map { i ->
                    val s = subs.getJSONObject(i)
                    SubtitleTrack(
                        index = s.getLong("index"),
                        language = s.optString("language").takeIf { !s.isNull("language") && it.isNotEmpty() },
                        title = s.optString("title").takeIf { !s.isNull("title") && it.isNotEmpty() },
                        url = s.optString("url").takeIf { !s.isNull("url") && it.isNotEmpty() },
                        default = s.optBoolean("default"),
                        forced = s.optBoolean("forced"),
                        external = s.optBoolean("external"),
                    )
                },
            )
        }

        /** The body of `POST /api/playback` for `request`. */
        fun body(request: PlayRequest, caps: Caps, clientId: String, mode: String, startMs: Long): JSONObject =
            JSONObject()
                .put("item_id", request.itemId)
                .put("file_id", request.fileId ?: JSONObject.NULL)
                .put("audio_index", request.audio ?: JSONObject.NULL)
                // The player shows text subtitles itself; the server only burns in image ones (it knows which).
                .put("subtitle_index", request.sub ?: JSONObject.NULL)
                .put("start_ms", startMs)
                .put("mode", mode)
                .put("client_id", clientId)
                .put("caps", caps.toJson())
    }
}

/** The episode that comes next, where to start it, and its name. */
data class NextItem(val id: Long, val startMs: Long, val title: String)

/** What `/api/server` says. */
data class ServerInfo(val name: String, val version: String, val setupRequired: Boolean)

class ApiException(val status: Int, message: String) : IOException(message)

/** Why a request failed, in a few words for people. */
fun describe(error: Throwable): String = when (error) {
    is java.net.ConnectException -> "nothing answers there"
    is java.net.SocketTimeoutException -> "no answer in time"
    is java.net.UnknownHostException -> "that address isn't known on this network"
    is javax.net.ssl.SSLException -> "its secure connection failed"
    is ApiException -> error.message ?: "HTTP ${error.status}"
    is org.json.JSONException -> "that isn't a pley server"
    else -> error.message ?: error.javaClass.simpleName
}

/** The few calls the native player makes to the server, with the web view's session. */
class PleyApi(private val server: String, private val token: String?) {
    private suspend fun call(method: String, path: String, body: JSONObject? = null): String =
        withContext(Dispatchers.IO) {
            val connection = URL(server.trimEnd('/') + path).openConnection() as HttpURLConnection
            try {
                connection.requestMethod = method
                connection.connectTimeout = 10_000
                connection.readTimeout = 20_000
                connection.setRequestProperty("Accept", "application/json")
                if (token != null) connection.setRequestProperty("Authorization", "Bearer $token")
                if (body != null) {
                    connection.doOutput = true
                    connection.setRequestProperty("Content-Type", "application/json")
                    connection.outputStream.use { it.write(body.toString().toByteArray()) }
                }
                val status = connection.responseCode
                val stream = if (status in 200..299) connection.inputStream else connection.errorStream
                val text = stream?.bufferedReader()?.use { it.readText() }.orEmpty()
                if (status !in 200..299) {
                    val message = try {
                        JSONObject(text).optString("message").ifEmpty { "HTTP $status" }
                    } catch (_: org.json.JSONException) {
                        "HTTP $status"
                    }
                    throw ApiException(status, message)
                }
                text
            } finally {
                connection.disconnect()
            }
        }

    /** What the server says about itself; throws when nothing (or something else) answers. */
    suspend fun serverInfo(): ServerInfo {
        val text = call("GET", "/api/server")
        val o = try {
            JSONObject(text)
        } catch (_: org.json.JSONException) {
            throw IOException("that isn't a pley server")
        }
        val name = o.optString("name")
        if (!o.has("setup_required")) throw IOException("that isn't a pley server")
        return ServerInfo(name.ifEmpty { "pley" }, o.optString("version"), o.optBoolean("setup_required"))
    }

    suspend fun startPlayback(body: JSONObject): Playback = Playback.parse(JSONObject(call("POST", "/api/playback", body)))

    suspend fun progress(itemId: Long, positionMs: Long, durationMs: Long, state: String, sessionId: String) {
        val body = JSONObject()
            .put("position_ms", positionMs)
            .put("state", state)
            .put("session_id", sessionId)
        if (durationMs > 0) body.put("duration_ms", durationMs)
        call("POST", "/api/items/$itemId/progress", body)
    }

    suspend fun stop(sessionId: String) {
        call("DELETE", "/api/playback/$sessionId")
    }

    /** The next episode after `itemId`, if there is one. */
    suspend fun next(itemId: Long): NextItem? {
        val item = JSONObject(call("GET", "/api/items/$itemId/next")).optJSONObject("item") ?: return null
        val state = item.optJSONObject("user_state")
        val watched = state?.optBoolean("watched") ?: false
        val position = state?.optLong("position_ms") ?: 0
        val show = item.optString("grandparent_title")
        val title = if (show.isNotEmpty()) "$show · ${item.optString("title")}" else item.optString("title")
        return NextItem(item.getLong("id"), if (watched) 0 else position, title)
    }
}
