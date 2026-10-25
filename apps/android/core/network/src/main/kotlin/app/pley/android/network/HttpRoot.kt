package app.pley.android.network

import app.pley.android.common.DeviceProfile
import java.util.concurrent.TimeUnit
import okhttp3.ConnectionPool
import okhttp3.Dispatcher
import okhttp3.Interceptor
import okhttp3.OkHttpClient

/**
 * The app's one OkHttp client, which every other derives from
 * (newBuilder), so they all share its connections and threads.
 */
object HttpRoot {
    /** Idle connections kept: enough for the API, pictures and a stream at once. */
    const val IDLE_CONNECTIONS = 10

    /**
     * How long an idle connection is kept, in seconds: under the 30 s after
     * which the server closes one (apps/server/src/http.rs, HEADER_TIMEOUT),
     * so a request never goes out on a connection the server is closing.
     */
    const val KEEP_ALIVE_SECONDS = 25L

    /** Requests at once, for the whole app. */
    const val MAX_REQUESTS = 64

    /** Requests at once to one server (pictures have their own limit, see NetworkGraph.images). */
    const val MAX_REQUESTS_PER_HOST = 8

    /**
     * The root client, introducing itself as [userAgent] (UserAgent.of: the
     * server names the device after it). Redirects never go from http to
     * https or back on their own; a connection that failed is tried again
     * on another route. [localRoute], when given, sends connections to the
     * home network over Wi-Fi or Ethernet while mobile data is the default
     * network (LocalRoute). [profile] is for limits by tier, none yet.
     */
    fun create(
        userAgent: String,
        @Suppress("UNUSED_PARAMETER") profile: DeviceProfile,
        localRoute: LocalRoute? = null,
    ): OkHttpClient = OkHttpClient.Builder()
        .connectionPool(ConnectionPool(IDLE_CONNECTIONS, KEEP_ALIVE_SECONDS, TimeUnit.SECONDS))
        .dispatcher(
            Dispatcher().apply {
                maxRequests = MAX_REQUESTS
                maxRequestsPerHost = MAX_REQUESTS_PER_HOST
            },
        )
        .followSslRedirects(false)
        .retryOnConnectionFailure(true)
        .addInterceptor(userAgent(userAgent))
        .apply { localRoute?.applyTo(this) }
        .build()

    /** Sets `User-Agent` on every request. */
    fun userAgent(value: String): Interceptor = Interceptor { chain ->
        chain.proceed(chain.request().newBuilder().header("User-Agent", value).build())
    }
}
