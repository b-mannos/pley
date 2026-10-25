package app.pley.android.fixtures

import app.pley.android.model.PleyJson
import java.util.concurrent.TimeUnit
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.FormBody
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import org.junit.Assert.assertEquals

/** An answer read whole: status, headers and body. */
class Answer(val code: Int, val headers: okhttp3.Headers, val bytes: ByteArray) {
    val text: String get() = bytes.toString(Charsets.UTF_8)

    fun header(name: String): String? = headers[name]

    fun <T> decode(serializer: KSerializer<T>): T = PleyJson.decodeFromString(serializer, text)

    fun json(): JsonObject = PleyJson.parseToJsonElement(text).jsonObject

    fun field(name: String): String? = json()[name]?.jsonPrimitive?.content
}

/** Talks to a FixtureServer as an app does: no redirects followed, no cookies. */
class Client(private val server: FixtureServer, var token: String? = null) {
    val http: OkHttpClient = OkHttpClient.Builder()
        .followRedirects(false)
        .readTimeout(10, TimeUnit.SECONDS)
        .build()

    fun get(path: String, headers: Map<String, String> = emptyMap()): Answer =
        send(Request.Builder().url(server.url + path).get(), headers)

    fun send(method: String, path: String, json: String? = null): Answer {
        val body = json?.toRequestBody("application/json".toMediaType())
            ?: if (method == "DELETE") null else ByteArray(0).toRequestBody(null)
        return send(Request.Builder().url(server.url + path).method(method, body), emptyMap())
    }

    fun form(path: String, vararg fields: Pair<String, String>): Answer {
        val body = FormBody.Builder().apply { fields.forEach { (k, v) -> add(k, v) } }.build()
        return send(Request.Builder().url(server.url + path).post(body), emptyMap(), auth = false)
    }

    fun <T> read(path: String, serializer: KSerializer<T>): T {
        val answer = get(path)
        assertEquals("GET $path: ${answer.text}", 200, answer.code)
        return answer.decode(serializer)
    }

    private fun send(builder: Request.Builder, headers: Map<String, String>, auth: Boolean = true): Answer {
        if (auth) token?.let { builder.header("Authorization", "Bearer $it") }
        headers.forEach { (k, v) -> builder.header(k, v) }
        return http.newCall(builder.build()).execute().use(::answer)
    }

    private fun answer(response: Response) = Answer(response.code, response.headers, response.body.bytes())
}

/** A clock tests move by hand. */
class FakeClock(var now: Long = 1_800_000_000_000L) : () -> Long {
    override fun invoke(): Long = now
}

/** A started FixtureServer with [options], closed after [block]. */
fun <T> withServer(options: FixtureServer.Options = FixtureServer.Options(scanEveryMs = 0), block: (FixtureServer) -> T): T =
    FixtureServer(options).use { server ->
        server.start()
        block(server)
    }
