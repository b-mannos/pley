package app.pley.android.player.engine

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.HttpDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.upstream.DefaultLoadErrorHandlingPolicy
import androidx.media3.exoplayer.upstream.LoadErrorHandlingPolicy
import app.pley.android.common.Addresses
import app.pley.android.player.EngineSource
import okhttp3.Call
import okhttp3.OkHttpClient
import okhttp3.Request

/**
 * Where the engine's bytes come from: the media client of the server each
 * request goes to (NetworkGraph.media, which sends the Bearer token to that
 * server's own origin only), through Media3's OkHttp data source. Local
 * files and app assets (tests) are read directly.
 */
@OptIn(markerClass = [UnstableApi::class])
class MediaLoading(
    context: Context,
    /** The media client for a server's origin (`http://nas:7539`). */
    private val mediaClient: (origin: String) -> OkHttpClient,
) {
    /** Requests go out on their own origin's client. */
    val calls: Call.Factory = Call.Factory { request: Request ->
        val url = request.url.toString()
        mediaClient(Addresses.origin(url) ?: url).newCall(request)
    }

    val http: HttpDataSource.Factory = OkHttpDataSource.Factory(calls)

    private val dataSources = DefaultDataSource.Factory(context.applicationContext, http)

    val sources: MediaSource.Factory = DefaultMediaSourceFactory(dataSources)
        .setLoadErrorHandlingPolicy(MediaRetries())

    /** The media source for [source], its type told by the source rather than guessed from the address. */
    fun mediaSource(source: EngineSource): MediaSource = sources.createMediaSource(mediaItem(source))

    companion object {
        fun mediaItem(source: EngineSource): MediaItem = MediaItem.Builder()
            .setUri(source.url)
            .setMimeType(if (source.hls) MimeTypes.APPLICATION_M3U8 else source.mimeType)
            .build()
    }
}

/**
 * Retries for segments and files: up to [RETRIES] times, waiting 0.5 s, then
 * twice as long each time up to 8 s. A 404 or 410 isn't retried and doesn't
 * exclude the track: the server's session is gone, and the playback session
 * starts a new one at the same position. 401 and 403 aren't either: a token
 * the client couldn't refresh won't work a second later.
 */
@OptIn(markerClass = [UnstableApi::class])
class MediaRetries : DefaultLoadErrorHandlingPolicy(RETRIES) {
    override fun getRetryDelayMsFor(loadErrorInfo: LoadErrorHandlingPolicy.LoadErrorInfo): Long {
        if (isFinal(loadErrorInfo.exception)) return C.TIME_UNSET
        if (super.getRetryDelayMsFor(loadErrorInfo) == C.TIME_UNSET) return C.TIME_UNSET
        return backoffMs(loadErrorInfo.errorCount)
    }

    override fun getFallbackSelectionFor(
        fallbackOptions: LoadErrorHandlingPolicy.FallbackOptions,
        loadErrorInfo: LoadErrorHandlingPolicy.LoadErrorInfo,
    ): LoadErrorHandlingPolicy.FallbackSelection? =
        if (isFinal(loadErrorInfo.exception)) null else super.getFallbackSelectionFor(fallbackOptions, loadErrorInfo)

    override fun getMinimumLoadableRetryCount(dataType: Int): Int = RETRIES

    companion object {
        const val RETRIES = 5
        const val FIRST_DELAY_MS = 500L
        const val MAX_DELAY_MS = 8_000L

        /** The wait before retry [errorCount] (1 for the first). */
        fun backoffMs(errorCount: Int): Long {
            val shift = (errorCount - 1).coerceIn(0, 10)
            return (FIRST_DELAY_MS shl shift).coerceAtMost(MAX_DELAY_MS)
        }

        /** HTTP answers that retrying won't change. */
        fun isFinal(error: Throwable?): Boolean {
            var e = error
            while (e != null) {
                if (e is HttpDataSource.InvalidResponseCodeException) return e.responseCode in FINAL_CODES
                e = e.cause
            }
            return false
        }

        private val FINAL_CODES = setOf(401, 403, 404, 410)
    }
}
