package app.pley.android.common

/**
 * Picture addresses at the sizes the server makes (apps/server/src/images.rs
 * BUCKETS): a width is rounded up to the next one, so every card of a size
 * shares one file in the caches. The `?v=` the server put in the address
 * stays: it changes when the picture does, so a cached one never goes stale.
 */
object ImageUrls {
    val BUCKETS = listOf(160, 240, 320, 480, 640, 960, 1280, 1920)

    /** The largest picture the server makes. */
    val LARGEST = BUCKETS.last()

    /**
     * The server's width for a picture drawn [px] pixels wide: the smallest
     * bucket at least that wide, never above [max] (the screen's width, or
     * 1280 on T0 devices) nor the largest.
     */
    fun bucket(px: Int, max: Int = LARGEST): Int {
        val cap = BUCKETS.lastOrNull { it <= max } ?: BUCKETS.first()
        val fit = BUCKETS.firstOrNull { it >= px } ?: LARGEST
        return minOf(fit, cap)
    }

    /** [url] (as the API gave it) asking for the width of [bucket]`(px, max)`. */
    fun sized(url: String, px: Int, max: Int = LARGEST): String {
        val fragment = url.indexOf('#').takeIf { it >= 0 }
        val base = if (fragment != null) url.substring(0, fragment) else url
        val tail = if (fragment != null) url.substring(fragment) else ""
        val query = base.substringAfter('?', "")
        val path = base.substringBefore('?')
        val kept = query.split('&').filter { it.isNotEmpty() && !it.startsWith("w=") }
        val params = kept + "w=${bucket(px, max)}"
        return path + "?" + params.joinToString("&") + tail
    }
}
