package app.pley.android.screens.onboarding

import android.content.Context
import app.pley.android.auth.Needs
import app.pley.android.auth.Profile
import app.pley.android.common.sha1Hex
import app.pley.android.screens.ScreensGraph
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

/**
 * The household's profiles as this device last saw them on a server, so
 * "Who's watching?" draws at once and the TV's start gate knows how many
 * there are without the network.
 */
interface HouseholdStore {
    /** The profiles of [server]; `null` when the app never saw them. */
    suspend fun read(server: String): List<Profile>?

    suspend fun write(server: String, profiles: List<Profile>)
}

/** A profile as saved: what the picker shows, no more. */
@Serializable
private data class SavedProfile(
    val id: Long,
    val name: String,
    val avatarUrl: String? = null,
    val avatarColor: String? = null,
    val current: Boolean = false,
    val needs: String = "none",
)

/** The household of each server in a small JSON file under filesDir/onboarding, read and written off the main thread. */
internal class FileHouseholdStore(
    private val dir: () -> File,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) : HouseholdStore {
    override suspend fun read(server: String): List<Profile>? = withContext(io) {
        val file = fileOf(server)
        if (!file.isFile) return@withContext null
        try {
            json.decodeFromString(ListSerializer(SavedProfile.serializer()), file.readText()).map { it.toProfile() }
        } catch (_: IOException) {
            null
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
        }
    }

    override suspend fun write(server: String, profiles: List<Profile>) = withContext(io) {
        val file = fileOf(server)
        try {
            file.parentFile?.mkdirs()
            val text = json.encodeToString(ListSerializer(SavedProfile.serializer()), profiles.map { it.toSaved() })
            // Written beside, then moved over: a crash never leaves half a list.
            val temp = File(file.parentFile, file.name + ".tmp")
            temp.writeText(text)
            if (!temp.renameTo(file)) {
                file.delete()
                temp.renameTo(file)
            }
        } catch (_: IOException) {
            // The picker loads from the server next time.
        }
        Unit
    }

    private fun fileOf(server: String): File = File(dir(), "household-${sha1Hex(server.trimEnd('/'))}.json")

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        fun of(context: Context): FileHouseholdStore {
            val app = context.applicationContext
            return FileHouseholdStore({ File(app.filesDir, "onboarding") })
        }
    }
}

private fun Profile.toSaved() = SavedProfile(id, name, avatarUrl, avatarColor, current, needs.name.lowercase())

private fun SavedProfile.toProfile() = Profile(id, name, avatarUrl, avatarColor, current, Needs.of(needs))

/**
 * The household's size for the TV's start gate (TvShellSource's
 * householdSize): "Who's watching?" shows at a cold start when the
 * household has several profiles.
 */
object Household {
    /**
     * How many profiles [server]'s household had when this device last
     * saw it (1 while unknown), from the disk only. When [signedIn], the
     * list is asked for again in the background for the next start.
     */
    suspend fun sizeAtStart(context: Context, graph: () -> ScreensGraph, server: String?, signedIn: Boolean): Int {
        if (server == null) return 1
        val store = FileHouseholdStore.of(context)
        val size = store.read(server)?.size ?: 1
        if (signedIn) {
            background.launch {
                // After the first frames: the Keystore and the network wait their turn.
                delay(REFRESH_DELAY_MS)
                runCatching {
                    val g = graph()
                    refresh(g.auth.profiles(server, g.network.media(server)).list(), store, server)
                }
            }
        }
        return size.coerceAtLeast(1)
    }

    private suspend fun refresh(profiles: List<Profile>, store: HouseholdStore, server: String) {
        if (profiles.isNotEmpty()) store.write(server, profiles)
    }

    private val background = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private const val REFRESH_DELAY_MS = 3_000L
}
