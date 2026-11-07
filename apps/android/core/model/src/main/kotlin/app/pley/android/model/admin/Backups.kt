package app.pley.android.model.admin

import kotlinx.serialization.Serializable

/** GET /api/system/backups: the database's health and its backups, newest first. */
@Serializable
data class DatabaseBackups(
    val health: DatabaseHealth = DatabaseHealth(),
    val backups: List<DatabaseBackup> = emptyList(),
)

/** The check of the database at start; [ok] is `null` until it finished. */
@Serializable
data class DatabaseHealth(
    val ok: Boolean? = null,
    /** What SQLite found, when the database is damaged. */
    val problem: String? = null,
)

/** A copy of the database (POST /api/system/backups answers the new one). */
@Serializable
data class DatabaseBackup(
    val name: String,
    val bytes: Long = 0,
    /** Unix seconds. */
    val createdAt: Long = 0,
)
