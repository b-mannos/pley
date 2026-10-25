package app.pley.android.designsystem.image

import androidx.compose.runtime.Immutable
import app.pley.android.common.Tier
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary

/**
 * A picture to draw, as the addresses to try in order: the first that
 * loads wins, as on the web (apps/web/src/lib/images.ts). Addresses are
 * absolute and unsized; the loader adds `w=` for the box it fills
 * ([ArtworkInterceptor]), so screens build these once, off the main
 * thread, without knowing sizes.
 *
 * [maxWidth] caps the width asked for, in pixels (0: only the device's
 * caps): backdrops use [ArtworkImage.backdropCap].
 */
@Immutable
data class ArtworkImage(
    val candidates: List<String>,
    val maxWidth: Int = 0,
) {
    val isEmpty: Boolean get() = candidates.isEmpty()

    companion object {
        /** The widest backdrop asked for: 1280 px, 960 on T0, where one layer of 1280 costs too much. */
        fun backdropCap(tier: Tier): Int = if (tier == Tier.T0) 960 else 1280

        /** One address, made absolute against [origin]; `null` without one. */
        fun of(url: String?, origin: String): ArtworkImage? =
            url?.takeIf { it.isNotEmpty() }?.let { ArtworkImage(listOf(absolute(it, origin))) }
    }
}

/** What kind of picture of an item: the server's image kinds. */
enum class ArtworkKind(internal val path: String) { Poster("poster"), Backdrop("backdrop"), Thumb("thumb") }

/**
 * The web's fallback chains (apps/web/src/lib/images.ts), as absolute
 * addresses on the server at [origin] ("http://192.168.1.20:7539").
 */
object ArtworkCandidates {
    /** GET /api/items/{id}/images/{kind}: a parent's or grandparent's picture. */
    fun itemImage(id: Long, kind: ArtworkKind, origin: String): String = absolute("/api/items/$id/images/${kind.path}", origin)

    /** Own poster → parent's (an episode's season) → grandparent's (its show). */
    fun poster(item: ItemSummary, origin: String): ArtworkImage = ArtworkImage(
        buildList {
            item.posterUrl?.takeIf { it.isNotEmpty() }?.let { add(absolute(it, origin)) }
            item.parentId?.let { add(itemImage(it, ArtworkKind.Poster, origin)) }
            item.grandparentId?.let { add(itemImage(it, ArtworkKind.Poster, origin)) }
        },
    )

    /** Own backdrop → the show's or artist's → the parent's, capped at [ArtworkImage.backdropCap]. */
    fun backdrop(item: ItemSummary, origin: String, tier: Tier): ArtworkImage = ArtworkImage(
        buildList {
            item.backdropUrl?.takeIf { it.isNotEmpty() }?.let { add(absolute(it, origin)) }
            item.grandparentId?.let { add(itemImage(it, ArtworkKind.Backdrop, origin)) }
            item.parentId?.let { add(itemImage(it, ArtworkKind.Backdrop, origin)) }
        },
        maxWidth = ArtworkImage.backdropCap(tier),
    )

    /**
     * A 16:9 still: a movie's backdrop first (its thumb is a frame grab,
     * made on demand), else the own thumb, then an episode's show backdrop
     * or another kind's own backdrop.
     */
    fun thumb(item: ItemSummary, origin: String): ArtworkImage {
        val backdrop = item.backdropUrl?.takeIf { it.isNotEmpty() }
        val out = mutableListOf<String>()
        if (item.kind == ItemKind.Movie && backdrop != null) out += backdrop
        item.thumbUrl?.takeIf { it.isNotEmpty() }?.let { out += it }
        if (item.kind == ItemKind.Episode && item.grandparentId != null) {
            out += "/api/items/${item.grandparentId}/images/${ArtworkKind.Backdrop.path}"
        }
        if (item.kind != ItemKind.Episode && backdrop != null && backdrop !in out) out += backdrop
        return ArtworkImage(out.map { absolute(it, origin) })
    }
}

/** [url] as the API gave it ("/api/items/12/images/poster?v=1") on the server at [origin]. */
internal fun absolute(url: String, origin: String): String = when {
    url.startsWith("http://") || url.startsWith("https://") -> url
    origin.isEmpty() -> url
    url.startsWith("/") -> origin.trimEnd('/') + url
    else -> origin.trimEnd('/') + "/" + url
}
