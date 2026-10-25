package app.pley.android.designsystem.image

import app.pley.android.common.ImageUrls
import app.pley.android.common.Tier
import app.pley.android.designsystem.PleyMotion
import coil3.intercept.Interceptor
import coil3.network.HttpException
import coil3.request.ErrorResult
import coil3.request.ImageResult
import coil3.request.SuccessResult
import coil3.request.crossfade
import coil3.size.Dimension
import kotlinx.coroutines.CancellationException

/**
 * What the device allows pictures: how wide they may be asked for, and
 * whether they fade in.
 *
 * - Never wider than the screen needs ([screenWidthPx], rounded up to the
 *   server's next width) and never 1920 on T0.
 * - A 150 ms fade, only for pictures that weren't in the memory cache
 *   (Coil's crossfade skips those), none on T0, and none for an
 *   [ArtworkImage] that says not to.
 */
class ArtworkPolicy(val tier: Tier, screenWidthPx: Int) {
    /** The widest picture this device asks for. */
    val maxWidth: Int = run {
        val screen = ImageUrls.BUCKETS.firstOrNull { it >= screenWidthPx } ?: ImageUrls.LARGEST
        if (tier == Tier.T0) minOf(screen, T0_MAX) else screen
    }

    /** How long a picture fades in; 0 for none. */
    val fadeMillis: Int = if (tier == Tier.T0) 0 else PleyMotion.IMAGE_FADE_MS

    /** The server's width for a box [px] wide ([maxWidth] when unknown), under [cap] when it has one. */
    fun width(px: Int?, cap: Int = 0): Int = ImageUrls.bucket(px ?: limit(cap), limit(cap))

    /** [url] asking for [width]`(px, cap)`. */
    fun sized(url: String, px: Int?, cap: Int = 0): String = ImageUrls.sized(url, px ?: limit(cap), limit(cap))

    private fun limit(cap: Int): Int = if (cap > 0) minOf(cap, maxWidth) else maxWidth

    companion object {
        /** 1280: a T0 device never decodes 1920-wide pictures. */
        const val T0_MAX = 1280
    }
}

/**
 * Loads [ArtworkImage]s: asks for each address at the width of the box it
 * fills (Coil resolves the box's size before this runs), tries them in
 * order until one loads, and remembers addresses the server said don't
 * exist, so a missing poster costs one request, not one per frame it shows.
 * Any other request passes through.
 */
class ArtworkInterceptor(private val policy: ArtworkPolicy) : Interceptor {
    private val missing = object : LinkedHashMap<String, Unit>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Unit>?): Boolean = size > MISSING_KEPT
    }

    override suspend fun intercept(chain: Interceptor.Chain): ImageResult {
        val image = chain.request.data as? ArtworkImage ?: return chain.proceed()
        val px = (chain.size.width as? Dimension.Pixels)?.px
        var last: ImageResult? = null
        for (candidate in image.candidates) {
            val url = policy.sized(candidate, px, image.maxWidth)
            if (isMissing(url)) continue
            val request = chain.request.newBuilder()
                .data(url)
                .apply { if (policy.fadeMillis > 0 && image.fade) crossfade(policy.fadeMillis) }
                .build()
            val result = try {
                chain.withRequest(request).proceed()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                ErrorResult(null, request, e)
            }
            if (result is SuccessResult) return result
            val code = ((result as? ErrorResult)?.throwable as? HttpException)?.response?.code
            if (code == 404 || code == 410) markMissing(url)
            last = result
        }
        return last ?: ErrorResult(null, chain.request, NoArtworkException())
    }

    private fun isMissing(url: String): Boolean = synchronized(missing) { url in missing }

    private fun markMissing(url: String) = synchronized(missing) { missing[url] = Unit }

    private companion object {
        const val MISSING_KEPT = 512
    }
}

/** An item without a picture to try, or none that loaded: the placeholder stays. */
class NoArtworkException : RuntimeException("No artwork")
