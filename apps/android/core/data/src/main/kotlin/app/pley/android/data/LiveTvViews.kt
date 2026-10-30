package app.pley.android.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import java.io.IOException
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first

/**
 * How Live TV was last left on this device, by the profile shown (the
 * web keeps its own in the browser): the category ("all", "favorites",
 * "recent", "group:UK| Sport"), the view ("Guide" or "Channels") and the
 * channel last watched, which the guide opens back at after a restart.
 */
data class LiveTvView(val category: String? = null, val view: String? = null, val channel: Long? = null)

interface LiveTvViews {
    /** What was kept: all `null` when nothing was. */
    suspend fun of(): LiveTvView

    /** Keeps what isn't `null` of [category], [view] and [channel], the rest as it was. */
    suspend fun keep(category: String? = null, view: String? = null, channel: Long? = null)

    companion object {
        /** Remembers nothing. */
        val None: LiveTvViews = object : LiveTvViews {
            override suspend fun of(): LiveTvView = LiveTvView()

            override suspend fun keep(category: String?, view: String?, channel: Long?) = Unit
        }
    }
}

/**
 * [LiveTvViews] in a DataStore, keys of their own per server and profile.
 * Without a profile (signed out, the server unknown) nothing is read or
 * kept.
 */
internal class DataStoreLiveTvViews(
    private val store: DataStore<Preferences>,
    private val scopes: StateFlow<ScopeState>,
) : LiveTvViews {
    override suspend fun of(): LiveTvView {
        val keys = keys() ?: return LiveTvView()
        return try {
            val prefs = store.data.first()
            LiveTvView(prefs[keys.category], prefs[keys.view], prefs[keys.channel])
        } catch (_: IOException) {
            LiveTvView()
        }
    }

    override suspend fun keep(category: String?, view: String?, channel: Long?) {
        val keys = keys() ?: return
        try {
            store.edit { prefs ->
                category?.let { prefs[keys.category] = it }
                view?.let { prefs[keys.view] = it }
                channel?.let { prefs[keys.channel] = it }
            }
        } catch (_: IOException) {
            // Not kept: Live TV opens as it did next time.
        }
    }

    private class Keys(prefix: String) {
        val category = stringPreferencesKey("$prefix/category")
        val view = stringPreferencesKey("$prefix/view")
        val channel = longPreferencesKey("$prefix/channel")
    }

    private suspend fun keys(): Keys? {
        val profile = (scopes.first { it !is ScopeState.Waiting } as? ScopeState.Ready)?.profile ?: return null
        return Keys("${profile.server}/${profile.userId}")
    }

    companion object {
        /** The DataStore's file, in filesDir. */
        const val FILE = "datastore/live_tv.preferences_pb"
    }
}
