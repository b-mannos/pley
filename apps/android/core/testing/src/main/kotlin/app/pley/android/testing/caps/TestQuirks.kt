package app.pley.android.testing.caps

import app.pley.android.caps.QuirkTable
import java.io.File

/**
 * The quirk table the app ships (core/caps/src/main/res/raw/quirks.json),
 * read from the source tree so tests without Android resources use the
 * real one. Found from any module's folder under apps/android.
 */
object TestQuirks {
    private const val PATH = "core/caps/src/main/res/raw/quirks.json"

    val shipped: QuirkTable by lazy { QuirkTable.parse(file().readText()) }

    private fun file(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            listOf(File(dir, PATH), File(dir, "apps/android/$PATH")).firstOrNull { it.isFile }?.let { return it }
            dir = dir.parentFile
        }
        error("$PATH not found above ${File("").absolutePath}")
    }
}
