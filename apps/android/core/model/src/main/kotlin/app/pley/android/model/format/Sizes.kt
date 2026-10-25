package app.pley.android.model.format

import java.util.Locale

/*
 * Sizes and rates, as apps/web/src/lib/format.ts writes them: a point for
 * decimals whatever the language, like JavaScript's toFixed.
 */

private val BYTE_UNITS = listOf("B", "KB", "MB", "GB", "TB")

/** 1536 → "1.5 KB", 150 MiB → "150 MB": three significant digits, in units of 1024, up to TB. */
fun formatBytes(bytes: Long?): String {
    if (bytes == null) return ""
    var v = bytes.toDouble()
    var i = 0
    while (v >= 1024 && i < BYTE_UNITS.size - 1) {
        v /= 1024
        i++
    }
    val digits = if (v >= 100 || i == 0) 0 else 1
    return "${fixed(v, digits)} ${BYTE_UNITS[i]}"
}

/** Bits per second → "8.2 Mbps", "12 Mbps", "320 kbps"; empty without a rate. */
fun formatBitrate(bps: Long?): String {
    if (bps == null || bps == 0L) return ""
    if (bps >= 1_000_000) return "${fixed(bps / 1_000_000.0, if (bps >= 10_000_000) 0 else 1)} Mbps"
    return "${Math.round(bps / 1000.0)} kbps"
}

/** [value] with [digits] decimals and a point, rounding half up as toFixed does for these values. */
internal fun fixed(value: Double, digits: Int): String = String.format(Locale.ROOT, "%.${digits}f", value)
