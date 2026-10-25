package app.pley.android

import android.content.Context
import androidx.core.content.edit
import app.pley.android.common.SavedServer
import app.pley.android.common.Servers
import org.json.JSONArray
import org.json.JSONObject

/** The servers opened before, kept in the app's preferences as JSON. */
class ServerStore(context: Context) {
    private val prefs = context.getSharedPreferences("servers", Context.MODE_PRIVATE)

    fun all(): List<SavedServer> = decode(prefs.getString(KEY, null))

    fun save(server: SavedServer) {
        prefs.edit { putString(KEY, encode(Servers.withServer(all(), server))) }
    }

    fun forget(url: String) {
        prefs.edit { putString(KEY, encode(all().filter { it.url != url })) }
    }

    companion object {
        private const val KEY = "saved"

        fun encode(servers: List<SavedServer>): String = JSONArray().apply {
            for (s in servers) {
                put(
                    JSONObject()
                        .put("url", s.url)
                        .put("name", s.name)
                        .put("id", s.id ?: JSONObject.NULL)
                        .put("last_used", s.lastUsed),
                )
            }
        }.toString()

        fun decode(json: String?): List<SavedServer> {
            if (json.isNullOrEmpty()) return emptyList()
            return try {
                val array = JSONArray(json)
                (0 until array.length()).mapNotNull { i ->
                    val o = array.optJSONObject(i) ?: return@mapNotNull null
                    val url = o.optString("url").takeIf { it.isNotEmpty() } ?: return@mapNotNull null
                    SavedServer(
                        url = url,
                        name = o.optString("name", url),
                        id = if (o.isNull("id")) null else o.optString("id"),
                        lastUsed = o.optLong("last_used"),
                    )
                }
            } catch (_: org.json.JSONException) {
                emptyList()
            }
        }
    }
}
