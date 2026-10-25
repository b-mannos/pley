package app.pley.android.model

import kotlinx.serialization.Serializable

/**
 * GET /api/files/{id}/trickplay: the seek-bar previews of a file, tiled
 * into JPEG sheets (packages/transcoder-api, `TrickplayInfo`). Thumbnail
 * `k` shows the video at `k × intervalMs`; it is at column `k % columns`,
 * row `(k / columns) % rows` of sheet `k / (columns × rows)`.
 */
@Serializable
data class TrickplayInfo(
    /** Identifies this set of sheets: the `v` of their addresses. */
    val id: String,
    val intervalMs: Long,
    /** The size of one thumbnail, in pixels. */
    val width: Int,
    val height: Int,
    val columns: Int,
    val rows: Int,
    /** Thumbnails in all. */
    val count: Int,
    /** Sheets: "0.jpg" to "{sheets - 1}.jpg". */
    val sheets: Int,
) {
    /** The sheet thumbnail [k] is on. */
    fun sheetOf(k: Int): Int = k / (columns * rows)

    /** Its column and row on that sheet. */
    fun cellOf(k: Int): Pair<Int, Int> = (k % columns) to ((k / columns) % rows)

    /** The thumbnail for [positionMs]. */
    fun thumbnailAt(positionMs: Long): Int =
        if (intervalMs <= 0 || count <= 0) 0 else (positionMs / intervalMs).toInt().coerceIn(0, count - 1)
}
