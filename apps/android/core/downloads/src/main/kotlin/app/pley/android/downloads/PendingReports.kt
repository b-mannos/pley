package app.pley.android.downloads

import app.pley.android.model.ProgressState
import java.io.File
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer

/** A progress report made while the server was out of reach. */
@Serializable
internal data class PendingReport(
    val server: String,
    val userId: Long,
    val itemId: Long,
    val positionMs: Long,
    val durationMs: Long? = null,
    val state: ProgressState = ProgressState.Stopped,
    /** When it was made, unix ms on this device. */
    val atMs: Long,
)

/**
 * Reports waiting for their server, the last of each item only (a report
 * says where the item is, so an older one adds nothing), kept in [file]
 * across restarts and written whole.
 */
internal class PendingReports(private val file: File) {
    private var reports: List<PendingReport>? = null

    @Synchronized
    fun add(report: PendingReport) {
        val now = all().filterNot { it.server == report.server && it.userId == report.userId && it.itemId == report.itemId }
        reports = now + report
        save()
    }

    /** The reports of [server] for [userId], oldest first. */
    @Synchronized
    fun of(server: String, userId: Long): List<PendingReport> = all().filter { it.server == server && it.userId == userId }

    /** Takes [report] out once it went (unless a newer one of its item came meanwhile). */
    @Synchronized
    fun sent(report: PendingReport) {
        reports = all().filterNot { it == report }
        save()
    }

    @Synchronized
    fun clear() {
        reports = emptyList()
        file.delete()
    }

    private fun all(): List<PendingReport> = reports ?: read().also { reports = it }

    private fun read(): List<PendingReport> = try {
        if (file.isFile) StoreJson.decodeFromString(ListSerializer(PendingReport.serializer()), file.readText()) else emptyList()
    } catch (_: Exception) {
        emptyList()
    }

    private fun save() {
        file.parentFile?.mkdirs()
        val temp = File(file.path + ".tmp")
        temp.writeText(StoreJson.encodeToString(ListSerializer(PendingReport.serializer()), reports.orEmpty()))
        if (!temp.renameTo(file)) {
            file.delete()
            temp.renameTo(file)
        }
    }
}
