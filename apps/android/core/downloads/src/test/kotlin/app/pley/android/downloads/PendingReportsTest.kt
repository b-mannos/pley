package app.pley.android.downloads

import app.pley.android.model.MediaFile
import app.pley.android.model.ProgressState
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Reports kept while offline (the last of each item, across restarts), and the file a phone downloads. */
class PendingReportsTest {
    @get:Rule
    val folder = TemporaryFolder()

    @Test
    fun theLastReportOfAnItemIsKept() {
        val file = folder.root.resolve("reports.json")
        val reports = PendingReports(file)
        reports.add(report(itemId = 1, position = 10_000, at = 1))
        reports.add(report(itemId = 2, position = 20_000, at = 2))
        reports.add(report(itemId = 1, position = 30_000, at = 3))

        val again = PendingReports(file).of("http://nas:7539", 2)

        assertEquals(listOf(2L to 20_000L, 1L to 30_000L), again.map { it.itemId to it.positionMs })
    }

    @Test
    fun aReportSentGoesButANewerOneStays() {
        val reports = PendingReports(folder.root.resolve("reports.json"))
        val first = report(itemId = 1, position = 10_000, at = 1)
        reports.add(first)
        reports.add(report(itemId = 1, position = 40_000, at = 5))

        reports.sent(first)

        assertEquals(listOf(40_000L), reports.of("http://nas:7539", 2).map { it.positionMs })
    }

    @Test
    fun aPhoneDownloadsTheSharpestFileUpTo1080p() {
        val uhd = MediaFile(id = 1, path = "a.mkv", size = 60_000_000_000, height = 2160)
        val hd = MediaFile(id = 2, path = "b.mkv", size = 8_000_000_000, height = 1080)
        val sd = MediaFile(id = 3, path = "c.mkv", size = 2_000_000_000, height = 480)

        assertEquals(2L, AppDownloads.pickFile(listOf(uhd, hd, sd))?.id)
        assertEquals(1L, AppDownloads.pickFile(listOf(uhd))?.id)
        assertEquals("4K only: the smaller", 4L, AppDownloads.pickFile(listOf(uhd, uhd.copy(id = 4, size = 30_000_000_000)))?.id)
    }

    private fun report(itemId: Long, position: Long, at: Long) = PendingReport(
        server = "http://nas:7539",
        userId = 2,
        itemId = itemId,
        positionMs = position,
        durationMs = 100_000,
        state = ProgressState.Stopped,
        atMs = at,
    )
}
