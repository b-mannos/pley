package app.pley.android.designsystem

import android.content.Context
import app.pley.android.common.DeviceProfile
import coil3.ImageLoader
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import okhttp3.OkHttpClient

/**
 * The app's one image loader (Coil), on the images client so pictures
 * share its connections. Not tuned yet: the caches sized by tier, hardware
 * bitmaps and the T0 limits come with the artwork.
 */
object PleyImageLoader {
    /** [client] is called on the first picture, not before: the first frame waits for nothing. */
    fun create(context: Context, client: () -> OkHttpClient, @Suppress("UNUSED_PARAMETER") profile: DeviceProfile): ImageLoader =
        ImageLoader.Builder(context)
            .components { add(OkHttpNetworkFetcherFactory(callFactory = { client() })) }
            .build()
}
