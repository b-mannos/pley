package app.pley.android.network

import app.pley.android.common.DeviceProfile
import okhttp3.OkHttpClient

/**
 * The app's one OkHttp client, which every other derives from
 * (newBuilder), so they all share its connections and threads. Not tuned
 * yet: the pool kept under the server's 30 s idle limit, the dispatcher's
 * limits and the redirect rules come with the network module.
 */
object HttpRoot {
    fun create(userAgent: String, @Suppress("UNUSED_PARAMETER") profile: DeviceProfile): OkHttpClient =
        OkHttpClient.Builder()
            .followSslRedirects(false)
            .addInterceptor { chain ->
                chain.proceed(chain.request().newBuilder().header("User-Agent", userAgent).build())
            }
            .build()
}
