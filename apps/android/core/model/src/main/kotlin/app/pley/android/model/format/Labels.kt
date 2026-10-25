package app.pley.android.model.format

import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import java.text.NumberFormat
import java.util.Locale

/*
 * Titles, counts and episode numbers, as apps/web/src/lib/format.ts makes
 * them, plus the spoken descriptions screen readers get for tiles.
 */

/** "1 movie", "0 movies", "12,345 movies", "3 children": the count in [locale]'s digits. */
fun pluralize(n: Int, one: String, many: String = "${one}s", locale: Locale = Locale.getDefault()): String =
    "${NumberFormat.getIntegerInstance(locale).format(n)} ${if (n == 1) one else many}"

/** "S1 · E3", "E3", or "" with neither number. */
fun episodeLabel(seasonNumber: Int?, episodeNumber: Int?): String =
    listOfNotNull(seasonNumber?.let { "S$it" }, episodeNumber?.let { "E$it" }).joinToString(" · ")

/** [episodeLabel] of an episode: its season is [ItemSummary.parentIndex]. */
fun episodeLabel(item: ItemSummary): String = episodeLabel(item.parentIndex, item.index)

/** A season's own title, else "Specials" for season 0, else "Season 2". */
fun seasonTitle(title: String?, index: Int?): String = when {
    !title.isNullOrEmpty() -> title
    index == 0 -> "Specials"
    index != null -> "Season $index"
    else -> "Season"
}

/** "Season 2 · Episode 5", for the episode lines of item pages; the parts that are known. */
fun episodeLongLabel(seasonNumber: Int?, episodeNumber: Int?): String = listOfNotNull(
    seasonNumber?.let { if (it == 0) "Specials" else "Season $it" },
    episodeNumber?.let { "Episode $it" },
).joinToString(" · ")

/** The short line under a card, by kind: a year, "3 seasons", "S2 · E5", "Daft Punk · 2001"… */
fun cardSubtitle(item: ItemSummary, locale: Locale = Locale.getDefault()): String = when (item.kind) {
    ItemKind.Movie -> item.year?.toString().orEmpty()
    ItemKind.Show -> when {
        item.childCount != null && item.childCount > 0 -> pluralize(item.childCount, "season", locale = locale)
        item.leafCount != null -> pluralize(item.leafCount, "episode", locale = locale)
        else -> item.year?.toString().orEmpty()
    }
    ItemKind.Season -> item.leafCount?.let { pluralize(it, "episode", locale = locale) }.orEmpty()
    ItemKind.Episode -> episodeLabel(item)
    ItemKind.Artist -> item.childCount?.let { pluralize(it, "album", locale = locale) }.orEmpty()
    ItemKind.Album -> listOfNotNull(item.parentTitle?.takeIf { it.isNotEmpty() }, item.year?.toString()).joinToString(" · ")
    ItemKind.Track -> listOfNotNull(
        item.grandparentTitle?.takeIf { it.isNotEmpty() },
        item.parentTitle?.takeIf { it.isNotEmpty() },
    ).joinToString(" · ")
    else -> ""
}

/** A title with its context: "The Office — S2 · E3 — Office Olympics", "The Office — Season 2", "Heat (1995)". */
fun fullTitle(item: ItemSummary): String = when (item.kind) {
    ItemKind.Episode -> listOf(item.grandparentTitle.orEmpty(), episodeLabel(item), item.title)
        .filter { it.isNotEmpty() }.joinToString(" — ")
    ItemKind.Season -> listOf(item.parentTitle.orEmpty(), seasonTitle(item.title, item.index))
        .filter { it.isNotEmpty() }.joinToString(" — ")
    ItemKind.Track -> listOf(item.title, item.grandparentTitle.orEmpty()).filter { it.isNotEmpty() }.joinToString(" — ")
    else -> if (item.year != null) "${item.title} (${item.year})" else item.title
}

/** How much of an item has been played, 0 to 1. */
fun progressFraction(positionMs: Long, durationMs: Long?): Float {
    if (positionMs <= 0 || durationMs == null || durationMs <= 0) return 0f
    return (positionMs.toDouble() / durationMs).coerceIn(0.0, 1.0).toFloat()
}

/** [progressFraction] of an item's resume point. */
fun progressFraction(item: ItemSummary): Float = progressFraction(item.userState.positionMs, item.durationMs)

/** Episodes of a show or season not watched yet; 0 for other kinds. */
fun unwatchedCount(item: ItemSummary): Int {
    val leaves = item.leafCount ?: return 0
    return maxOf(0, leaves - (item.viewedLeafCount ?: 0))
}

/** A video the player plays: a movie, an episode or an extra (not a show, a season or music). */
fun isPlayableVideo(kind: ItemKind): Boolean = kind == ItemKind.Movie || kind == ItemKind.Episode || kind == ItemKind.Extra

/** An item's resume point is worth showing: started, not finished, not marked watched. */
fun isInProgress(item: ItemSummary): Boolean {
    val duration = item.durationMs ?: return false
    return !item.userState.watched && item.userState.positionMs > 0 && item.userState.positionMs < duration
}

/**
 * What a screen reader says for a tile: the title, then what places it,
 * then its state. "Glasshouse, Season 2 Episode 5, 26 minutes left";
 * "Northern Lights, 2024, watched"; "Severance, 2 seasons, 6 unwatched".
 */
fun tileDescription(item: ItemSummary, locale: Locale = Locale.getDefault()): String {
    val context = when (item.kind) {
        ItemKind.Episode -> listOfNotNull(
            item.grandparentTitle?.takeIf { it.isNotEmpty() && it != item.title },
            episodeLongLabel(item.parentIndex, item.index).replace(" · ", " ").takeIf { it.isNotEmpty() },
        ).joinToString(", ")
        else -> cardSubtitle(item, locale)
    }
    val unwatched = unwatchedCount(item)
    val state = when {
        isInProgress(item) -> spokenTimeLeft(item.userState.positionMs, item.durationMs)
        item.userState.watched -> "watched"
        (item.kind == ItemKind.Show || item.kind == ItemKind.Season) && unwatched > 0 -> "$unwatched unwatched"
        else -> ""
    }
    return listOf(item.title, context, state).filter { it.isNotEmpty() }.joinToString(", ")
}
