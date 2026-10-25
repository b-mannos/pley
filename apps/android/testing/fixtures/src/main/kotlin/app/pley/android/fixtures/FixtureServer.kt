package app.pley.android.fixtures

import java.io.Closeable
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer

/**
 * A fake pley server on this machine (127.0.0.1, a free port), for tests,
 * demos and benchmarks: nothing the app does in them reaches a real server.
 * Only the skeleton for now: every request is answered 501 until the
 * routes, their data and the sign-in are added.
 */
class FixtureServer : Closeable {
    private val server = MockWebServer()

    /** The server's address, once [start]ed: `http://127.0.0.1:<port>`. */
    val url: String
        // By address: a hosts file may give 127.0.0.1 a name of its own.
        get() = "http://127.0.0.1:${server.port}"

    fun start() {
        server.dispatcher = object : mockwebserver3.Dispatcher() {
            override fun dispatch(request: mockwebserver3.RecordedRequest): MockResponse =
                MockResponse.Builder()
                    .code(501)
                    .addHeader("Content-Type", "application/json")
                    .body("""{"code":"not_built","message":"the fixture server doesn't answer this yet"}""")
                    .build()
        }
        server.start(java.net.InetAddress.getByName("127.0.0.1"), 0)
    }

    override fun close() {
        server.close()
    }
}
