package app.pley.android.fixtures

import app.pley.android.model.ApiErrorBody
import app.pley.android.model.PleyJson
import java.net.URLDecoder
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerializationException
import mockwebserver3.MockResponse
import mockwebserver3.RecordedRequest
import okhttp3.HttpUrl
import okio.Buffer

/** One request, with what the routes read of it. */
internal class Call(val request: RecordedRequest) {
    val method: String = request.method
    val url: HttpUrl = request.url
    val path: String = url.encodedPath

    /** The path's parts without the empty ones: `["api", "items", "12"]`. */
    val segments: List<String> = url.pathSegments.filter { it.isNotEmpty() }

    val text: String get() = request.body?.utf8().orEmpty()

    fun query(name: String): String? = url.queryParameter(name)

    fun header(name: String): String? = request.headers[name]

    /** An `application/x-www-form-urlencoded` body: each name's values. */
    fun form(): Map<String, List<String>> = text.split('&').filter { it.isNotEmpty() }.map { pair ->
        val name = pair.substringBefore('=')
        val value = if ('=' in pair) pair.substringAfter('=') else ""
        decode(name) to decode(value)
    }.groupBy({ it.first }, { it.second })

    /** The body as JSON of [serializer], or `null` when it isn't. */
    fun <T> body(serializer: KSerializer<T>): T? = try {
        PleyJson.decodeFromString(serializer, text)
    } catch (_: SerializationException) {
        null
    } catch (_: IllegalArgumentException) {
        null
    }

    /**
     * The path's values where [pattern] has `*` ("api/items/ * /children",
     * without the spaces), or `null` when the path isn't that.
     */
    fun match(pattern: List<String>): List<String>? {
        if (pattern.size != segments.size) return null
        val values = mutableListOf<String>()
        for ((want, got) in pattern.zip(segments)) {
            when (want) {
                "*" -> values += got
                got -> Unit
                else -> return null
            }
        }
        return values
    }

    private fun decode(s: String): String = URLDecoder.decode(s, Charsets.UTF_8.name())
}

/** The answers the routes give. */
internal object Reply {
    fun json(text: String, code: Int = 200, headers: Map<String, String> = emptyMap()): MockResponse =
        MockResponse.Builder()
            .code(code)
            .setHeader("Content-Type", "application/json")
            .apply { headers.forEach { (name, value) -> setHeader(name, value) } }
            .body(text)
            .build()

    fun <T> json(serializer: KSerializer<T>, value: T, code: Int = 200, headers: Map<String, String> = emptyMap()): MockResponse =
        json(PleyJson.encodeToString(serializer, value), code, headers)

    fun noContent(): MockResponse = MockResponse.Builder().code(204).build()

    /** pley's own refusal (docs/API.md, "Errors"). */
    fun error(code: Int, name: String, message: String, wwwAuthenticate: String? = null): MockResponse =
        json(
            ApiErrorBody.serializer(),
            ApiErrorBody(name, message),
            code,
            if (wwwAuthenticate != null) mapOf("WWW-Authenticate" to wwwAuthenticate) else emptyMap(),
        )

    fun notFound(message: String = "not found"): MockResponse = error(404, "not_found", message)

    fun badRequest(message: String): MockResponse = error(400, "bad_request", message)

    fun bytes(bytes: ByteArray, type: String, code: Int = 200, headers: Map<String, String> = emptyMap()): MockResponse =
        bytes(bytes, 0, bytes.size, type, code, headers)

    fun bytes(
        bytes: ByteArray,
        offset: Int,
        length: Int,
        type: String,
        code: Int = 200,
        headers: Map<String, String> = emptyMap(),
    ): MockResponse = MockResponse.Builder()
        .code(code)
        .setHeader("Content-Type", type)
        .apply { headers.forEach { (name, value) -> setHeader(name, value) } }
        .body(Buffer().write(bytes, offset, length))
        .build()

    /** A picture: kept as long as its address, which changes with it. */
    fun picture(bytes: ByteArray): MockResponse =
        bytes(bytes, "image/png", headers = mapOf("Cache-Control" to "private, max-age=31536000, immutable"))
}
