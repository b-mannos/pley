package app.pley.android.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.IOException
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first

/**
 * How each library was last left on this device, by the profile shown:
 * its sort, order and filters, as a library page's query without its tab
 * ("sort=added&watched=false"). A library opened again shows them.
 */
interface LibraryViews {
    /** The view [libraryId] was left in: `null` when it never left the default. */
    suspend fun of(libraryId: Long): String?

    /** Remembers [view] for [libraryId] ("" is the default view: forgotten). */
    suspend fun keep(libraryId: Long, view: String)

    companion object {
        /** Remembers nothing. */
        val None: LibraryViews = object : LibraryViews {
            override suspend fun of(libraryId: Long): String? = null

            override suspend fun keep(libraryId: Long, view: String) = Unit
        }
    }
}

/**
 * [LibraryViews] in a DataStore, one key per server, profile and library.
 * Without a profile (signed out, the server unknown) nothing is read or
 * kept.
 */
internal class DataStoreLibraryViews(
    private val store: DataStore<Preferences>,
    private val scopes: StateFlow<ScopeState>,
) : LibraryViews {
    override suspend fun of(libraryId: Long): String? {
        val key = key(libraryId) ?: return null
        return try {
            store.data.first()[key]
        } catch (_: IOException) {
            null
        }
    }

    override suspend fun keep(libraryId: Long, view: String) {
        val key = key(libraryId) ?: return
        try {
            store.edit { if (view.isEmpty()) it.remove(key) else it[key] = view }
        } catch (_: IOException) {
            // Not kept: the library opens as it is next time.
        }
    }

    private suspend fun key(libraryId: Long): Preferences.Key<String>? {
        val profile = (scopes.first { it !is ScopeState.Waiting } as? ScopeState.Ready)?.profile ?: return null
        return stringPreferencesKey("${profile.server}/${profile.userId}/$libraryId")
    }

    companion object {
        /** The DataStore's file, in filesDir. */
        const val FILE = "datastore/library_views.preferences_pb"
    }
}
