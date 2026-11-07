package app.pley.android.screens.admin.livetv

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.PleyJson
import app.pley.android.model.admin.LiveTvAccess
import app.pley.android.model.admin.LiveTvPerson
import app.pley.android.model.admin.LiveTvSettings
import app.pley.android.screens.admin.AdminViewModel
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** Live TV's settings as read: what the page shows, and the whole answer, which a change is sent on. */
data class LiveTvOptions(val settings: LiveTvSettings, val raw: JsonObject) {
    /** These settings with [changes] (snake_case names, as the server has them). */
    fun with(changes: Map<String, JsonElement>): LiveTvOptions {
        val next = JsonObject(raw + changes)
        return LiveTvOptions(PleyJson.decodeFromJsonElement(LiveTvSettings.serializer(), next), next)
    }

    companion object {
        fun of(raw: JsonObject) = LiveTvOptions(PleyJson.decodeFromJsonElement(LiveTvSettings.serializer(), raw), raw)
    }
}

/** A person's own access to Live TV, whatever "Open to" says. */
enum class PersonAccess { AsOpenTo, Always, Never }

/**
 * Live TV's Options, as the web's tab: who watches, how channels play, and
 * how often channels and guides are read. Each value is sent as it is
 * chosen. PUT /api/live/settings takes the whole of them (what it leaves
 * out goes back to its default), so a change is sent on what was read,
 * settings this app doesn't show included.
 */
class LiveTvOptionsViewModel(admin: ServerAdmin) : AdminViewModel(admin) {
    val options = resource { LiveTvOptions.of(get(JsonObject.serializer(), listOf("api", "live", "settings"))) }

    /** The accounts "People" lists: everyone but administrators, who always watch. */
    val people = resource {
        get(ListSerializer(LiveTvPerson.serializer()), listOf("api", "users")).filterNot { it.isAdmin }
    }

    fun setAccess(access: LiveTvAccess) = set("access" to JsonPrimitive(access.wire))

    fun setAgeLimited(on: Boolean) = set("age_limited" to JsonPrimitive(on))

    fun setDeinterlace(on: Boolean) = set("deinterlace" to JsonPrimitive(on))

    /** `null`: as many lines as the channel has. */
    fun setMaxHeight(height: Int?) = set("max_height" to (height?.let(::JsonPrimitive) ?: JsonNull))

    fun setRefreshHours(hours: Int) = set("refresh_hours" to JsonPrimitive(hours))

    fun setHideOffline(on: Boolean) = set("hide_offline" to JsonPrimitive(on))

    fun setGuideDays(days: Int) = set("guide_days" to JsonPrimitive(days))

    /** [person] watches as "Open to" says, always, or never. */
    fun setPerson(person: Long, access: PersonAccess) {
        val settings = options.state.value.valueOrNull?.settings ?: return
        val allowed = settings.allowedUsers.filter { it != person } + listOfNotNull(person.takeIf { access == PersonAccess.Always })
        val denied = settings.deniedUsers.filter { it != person } + listOfNotNull(person.takeIf { access == PersonAccess.Never })
        set("allowed_users" to ids(allowed), "denied_users" to ids(denied))
    }

    /**
     * Shows [changes] at once and sends them with the rest of what was
     * read; the server's answer is shown after, and what it has is read
     * again when it refuses them.
     */
    private fun set(vararg changes: Pair<String, JsonElement>) {
        val shown = options.state.value.valueOrNull ?: return
        val next = shown.with(changes.toMap())
        options.set(next)
        change {
            try {
                val answer = send("PUT", JsonObject.serializer(), listOf("api", "live", "settings"), next.raw)
                // A later change shown meanwhile is the one its own answer settles.
                if (options.state.value.valueOrNull == next) options.set(LiveTvOptions.of(answer))
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                options.reload()
                throw e
            }
        }
    }

    companion object {
        val factory: ViewModelProvider.Factory = viewModelFactory {
            initializer { LiveTvOptionsViewModel(screensGraph().data.admin) }
        }
    }
}

/** How [settings] has [person]: always, never, or as "Open to" says. */
fun personAccess(settings: LiveTvSettings, person: Long): PersonAccess = when (person) {
    in settings.allowedUsers -> PersonAccess.Always
    in settings.deniedUsers -> PersonAccess.Never
    else -> PersonAccess.AsOpenTo
}

private fun ids(ids: List<Long>) = JsonArray(ids.map(::JsonPrimitive))
