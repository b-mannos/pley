package app.pley.android.auth

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONArray
import org.json.JSONException

/** What switching to a profile takes (`needs` of GET /api/oauth/profiles). */
enum class Needs {
    /** Nothing: it switches at once. */
    NONE,

    /** Its PIN. */
    PIN,

    /** Its password, which only the browser asks for: the app signs in again. */
    PASSWORD,

    /** It can't be used from the app. */
    LOCKED,
    ;

    companion object {
        /** Anything this app doesn't know is read as locked. */
        fun of(name: String): Needs = when (name) {
            "none" -> NONE
            "pin" -> PIN
            "password" -> PASSWORD
            else -> LOCKED
        }
    }
}

/** One profile of the household. No usernames, roles or limits: the server doesn't send them to apps. */
data class Profile(
    val id: Long,
    val name: String,
    val avatarUrl: String?,
    val avatarColor: String?,
    val current: Boolean,
    val needs: Needs,
) {
    companion object {
        fun parseList(text: String): List<Profile> {
            val array = try {
                JSONArray(text)
            } catch (_: JSONException) {
                throw AuthException("the server's answer can't be read: try again")
            }
            return (0 until array.length()).mapNotNull { i ->
                val o = array.optJSONObject(i) ?: return@mapNotNull null
                val id = o.optLong("id", -1).takeIf { it > 0 } ?: return@mapNotNull null
                Profile(
                    id = id,
                    name = plain(o.optString("name")),
                    avatarUrl = if (o.isNull("avatar_url")) null else o.optString("avatar_url").ifEmpty { null },
                    avatarColor = if (o.isNull("avatar_color")) null else o.optString("avatar_color").ifEmpty { null },
                    current = o.optBoolean("current"),
                    needs = Needs.of(o.optString("needs")),
                )
            }
        }
    }
}

/** How a switch went. */
sealed interface SwitchResult {
    data object Switched : SwitchResult

    /** Ask for the profile's PIN, then switch again with it. */
    data object NeedsPin : SwitchResult

    /**
     * The profile needs its password: sign in again in the browser
     * (Auth.begin), where the person picks it. The old sign-in is ended
     * once the new one is in.
     */
    data object SignInAgain : SwitchResult

    data class Failed(val message: String) : SwitchResult
}

/**
 * The household's profiles for pley's own apps, and switching between them
 * with the switch-profile grant: with a PIN when the profile has one; never
 * with a password or a two-step code, which only the browser asks for.
 */
class Profiles(private val api: OkHttpClient, private val tokens: Tokens) {
    /** Whether this sign-in may switch profiles: the server offers it and allowed it. */
    fun available(): Boolean = "profiles" in tokens.scopes()

    /** The household (GET /api/oauth/profiles); empty for someone who isn't a home member. */
    suspend fun list(): List<Profile> = withContext(Dispatchers.IO) {
        val request = Request.Builder().url(tokens.issuer + PATH).header("Accept", "application/json").get().build()
        api.newCall(request).execute().use { response ->
            val text = response.body?.string().orEmpty()
            when {
                response.isSuccessful -> Profile.parseList(text)
                response.code == 401 -> throw SignedOutException()
                response.code == 403 -> throw AuthException("this sign-in can't switch profiles: sign in again to allow it")
                else -> throw AuthException("the server couldn't answer (HTTP ${response.code})")
            }
        }
    }

    /** Switches to [profile], with [pin] when it needs one. */
    suspend fun switchTo(profile: Profile, pin: String? = null): SwitchResult {
        when (profile.needs) {
            Needs.NONE -> Unit
            Needs.PIN -> if (pin.isNullOrEmpty()) return SwitchResult.NeedsPin
            Needs.PASSWORD -> return SwitchResult.SignInAgain
            Needs.LOCKED -> return SwitchResult.Failed("this profile can't be used from the app")
        }
        return try {
            tokens.switchProfile(profile.id, pin?.takeIf { profile.needs == Needs.PIN })
            SwitchResult.Switched
        } catch (e: SignedOutException) {
            throw e
        } catch (e: TokenException) {
            val oauth = e.oauth
            if (oauth?.error == "invalid_grant" && oauth.reason == "browser") SwitchResult.SignInAgain
            else SwitchResult.Failed(message(oauth))
        }
    }

    companion object {
        const val PATH = "/api/oauth/profiles"

        /** A refused switch in a few words, by the server's reason. */
        fun message(error: OAuthError?): String = when (error?.reason) {
            "wrong_pin" -> "wrong PIN"
            "throttled" -> "too many wrong PINs: try again in a few minutes"
            "pin_locked" ->
                "this profile's PIN is locked after too many wrong tries: its owner can set it again in their account, or an administrator can"
            "browser" -> "this profile needs its password: sign in again to use it"
            "not_allowed" -> "you can't switch to this profile"
            else -> when (error?.error) {
                "unauthorized_client" -> "this sign-in can't switch profiles: sign in again to allow it"
                "invalid_request" -> error.description ?: "you can't switch to this profile"
                else -> error?.description ?: "the server refused: try again"
            }
        }
    }
}
