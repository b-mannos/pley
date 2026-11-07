package app.pley.android.screens.admin.server

import app.pley.android.data.ServerAdmin
import app.pley.android.model.PleyJson
import app.pley.android.screens.admin.AdminResource
import app.pley.android.screens.admin.AdminViewModel
import app.pley.android.screens.admin.valueOrNull
import kotlinx.coroutines.Job
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject

/** GET and PUT /api/settings. */
internal val SETTINGS: List<Any> = listOf("api", "settings")

/**
 * A page that edits some of the server's settings (Server, Requests): it
 * reads them as [T], which declares the fields it uses, and sends each
 * change as a partial body of PUT /api/settings, which answers with every
 * setting.
 */
abstract class SettingsPageViewModel<T : Any>(admin: ServerAdmin, private val serializer: KSerializer<T>) : AdminViewModel(admin) {
    val settings: AdminResource<T> = resource { get(serializer, SETTINGS) }

    /**
     * Sends [body], showing it at once (a switch flips as it is touched)
     * unless [shownFirst] is false, then the server's answer, or what it has
     * after a refusal. [then] runs after either (reading again what the
     * change touches).
     */
    protected fun save(body: JsonObject, shownFirst: Boolean = true, then: () -> Unit = {}): Job {
        if (shownFirst) settings.state.value.valueOrNull?.let { settings.set(it.patched(serializer, body)) }
        var saved = false
        return change(after = {
            if (!saved) settings.reload()
            then()
        }) {
            val answer = send("PUT", serializer, SETTINGS, body)
            saved = true
            // An answer to an earlier change would undo, for a moment, one still on its way.
            if (busy.value <= 1) settings.set(answer)
        }
    }
}

/** These settings with [body]'s fields: the names are the server's, which the app's JSON writes too. */
internal fun <T> T.patched(serializer: KSerializer<T>, body: JsonObject): T {
    val now = PleyJson.encodeToJsonElement(serializer, this).jsonObject
    return PleyJson.decodeFromJsonElement(serializer, JsonObject(now + body))
}
