package app.pley.android.auth

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import androidx.browser.customtabs.CustomTabsClient
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.net.toUri
import app.pley.android.Links
import app.pley.android.describe
import java.io.IOException
import java.net.URI
import java.net.URISyntaxException
import java.net.URLDecoder
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import org.json.JSONObject

/**
 * pley for Android's client ID: a client metadata document built into
 * every pley server (apps/server/src/oauth/builtin/android.json), never
 * fetched, so signing in works without internet.
 */
const val CLIENT_ID = "https://hhaidar.github.io/pley/oauth/android.json"

/** Where the browser comes back: the app id as a private-use scheme (RFC 8252 §7.1). */
const val REDIRECT_SCHEME = "app.pley.android"
const val REDIRECT_PATH = "/oauth/callback"
const val REDIRECT_URI = "$REDIRECT_SCHEME:$REDIRECT_PATH"

/** What the app asks for: no more than its built-in document allows. */
val SCOPES = listOf("library", "play", "playlists", "profiles")

/** The extension grant for switching profiles (pley servers with profiles). */
const val SWITCH_PROFILE_GRANT = "urn:pley:params:oauth:grant-type:switch-profile"

/**
 * A sign-in waiting for the browser to come back. [returnTo] is the class
 * of the app's activity that started it, brought back on top afterwards.
 */
data class PendingFlow(
    val server: String,
    val issuer: String,
    val state: String,
    val verifier: String,
    val tokenEndpoint: String,
    val revocationEndpoint: String,
    val scope: String,
    val createdAt: Long,
    val returnTo: String?,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("server", server)
        .put("issuer", issuer)
        .put("state", state)
        .put("verifier", verifier)
        .put("token_endpoint", tokenEndpoint)
        .put("revocation_endpoint", revocationEndpoint)
        .put("scope", scope)
        .put("created_at", createdAt)
        .put("return_to", returnTo ?: JSONObject.NULL)

    override fun toString(): String = "PendingFlow(server=$server)"

    companion object {
        fun fromJson(o: JSONObject): PendingFlow = PendingFlow(
            server = o.getString("server"),
            issuer = o.getString("issuer"),
            state = o.getString("state"),
            verifier = o.getString("verifier"),
            tokenEndpoint = o.getString("token_endpoint"),
            revocationEndpoint = o.getString("revocation_endpoint"),
            scope = o.getString("scope"),
            createdAt = o.getLong("created_at"),
            returnTo = if (o.isNull("return_to")) null else o.getString("return_to"),
        )
    }
}

/** The browser's way back to the app: `app.pley.android:/oauth/callback?…`. */
data class Callback(
    val code: String?,
    val state: String?,
    val iss: String?,
    val error: String?,
    val errorDescription: String?,
) {
    override fun toString(): String = "Callback(error=$error)"

    companion object {
        /** The callback in [uri]; `null` for anything else, or a parameter given twice. */
        fun parse(uri: String): Callback? {
            val parsed = try {
                URI(uri)
            } catch (_: URISyntaxException) {
                return null
            }
            if (!parsed.scheme.equals(REDIRECT_SCHEME, ignoreCase = true)) return null
            if (parsed.rawAuthority != null || parsed.rawPath != REDIRECT_PATH) return null
            val params = mutableMapOf<String, String>()
            for (pair in parsed.rawQuery.orEmpty().split('&').filter { it.isNotEmpty() }) {
                val name = decode(pair.substringBefore('=')) ?: return null
                val value = decode(if ('=' in pair) pair.substringAfter('=') else "") ?: return null
                if (params.put(name, value) != null) return null
            }
            fun get(name: String) = params[name]?.ifEmpty { null }
            return Callback(get("code"), get("state"), get("iss"), get("error"), get("error_description"))
        }

        private fun decode(text: String): String? = try {
            URLDecoder.decode(text, "UTF-8")
        } catch (_: IllegalArgumentException) {
            null
        }
    }
}

/** Where a sign-in stands, for the screens that show it. */
sealed interface SignInState {
    data object Idle : SignInState

    /** The browser came back; the code is being exchanged. */
    data object Finishing : SignInState

    data class SignedIn(val issuer: String) : SignInState

    data class Failed(val message: String) : SignInState
}

/** What a callback leads to. */
sealed interface CallbackOutcome {
    /** Not for a waiting sign-in (another one, an old one, or a second answer): nothing happens. */
    data object Ignored : CallbackOutcome

    data class Failed(val flow: PendingFlow, val message: String) : CallbackOutcome

    data class Exchange(val flow: PendingFlow, val code: String) : CallbackOutcome
}

/**
 * Signing in through the server's own page (OAuth 2.1 with PKCE, RFC 8252):
 * the app opens the page in the browser, the person signs in there (with
 * their password and two-step code, which the app never sees) and allows
 * the app, and the browser comes back with a code the app exchanges for
 * tokens. Never in a web view, and never with the web view's cookie.
 */
class SignIn(
    private val http: OkHttpClient,
    private val store: TokenStore,
    private val tokensOf: (String) -> Tokens,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    /**
     * Starts signing in to [server]: reads its metadata, keeps a new
     * waiting sign-in (replacing an older one for that server) and gives
     * the address of its sign-in page.
     */
    suspend fun start(server: String, returnTo: String? = null): String {
        val home = Links.address(server) ?: throw AuthException("that isn't a server address")
        Transport.checkAddress(home)
        val metadata = ServerMetadata.fetch(http, home)
        val flow = PendingFlow(
            server = home,
            issuer = metadata.issuer,
            state = Pkce.state(),
            verifier = Pkce.verifier(),
            tokenEndpoint = metadata.tokenEndpoint,
            revocationEndpoint = metadata.revocationEndpoint,
            scope = scopeFor(metadata),
            createdAt = clock(),
            returnTo = returnTo,
        )
        store.keepFlow(flow)
        return authorizationUrl(metadata.authorizationEndpoint, flow)
    }

    /** What [callback] means for the waiting sign-ins; the one it answers is forgotten either way. */
    fun handle(callback: Callback): CallbackOutcome {
        val state = callback.state ?: return CallbackOutcome.Ignored
        val flow = store.takeFlow(state, clock(), FLOW_LIFETIME_MS) ?: return CallbackOutcome.Ignored
        // RFC 9207: the answer must come from the server the sign-in went to.
        if (callback.iss == null || callback.iss != flow.issuer) {
            return CallbackOutcome.Failed(flow, "the answer came from another server than the one you signed in to: try again")
        }
        if (callback.error != null) {
            val message = when (callback.error) {
                "access_denied" -> "you didn't allow the app"
                else -> callback.errorDescription?.let { "the server refused: ${plain(it)}" }
                    ?: "the server refused (${plain(callback.error)})"
            }
            return CallbackOutcome.Failed(flow, message)
        }
        val code = callback.code ?: return CallbackOutcome.Failed(flow, "the server sent no code: try again")
        return CallbackOutcome.Exchange(flow, code)
    }

    /**
     * Exchanges [code] for tokens and keeps them. An earlier sign-in to
     * the same server is ended afterwards (it was replaced: a profile
     * that needed its password, or signing in again).
     */
    suspend fun exchange(flow: PendingFlow, code: String): StoredTokens {
        val answer = TokenEndpoint.post(
            http,
            flow.tokenEndpoint,
            listOf(
                "grant_type" to "authorization_code",
                "code" to code,
                "code_verifier" to flow.verifier,
                "client_id" to CLIENT_ID,
                "redirect_uri" to REDIRECT_URI,
            ),
        )
        val tokens = TokenResponse.parse(answer, flow.scope)
            .stored(flow.issuer, flow.tokenEndpoint, flow.revocationEndpoint, clock())
        val previous = tokensOf(flow.issuer).install(tokens)
        if (previous != null && previous.refreshToken != tokens.refreshToken) Tokens.revoke(http, previous)
        return tokens
    }

    /** Exchanges the code of [outcome]; what to show. */
    suspend fun complete(outcome: CallbackOutcome.Exchange): SignInState = try {
        SignInState.SignedIn(exchange(outcome.flow, outcome.code).issuer)
    } catch (e: TokenException) {
        // A code works once, for a minute.
        SignInState.Failed(if (e.oauth?.error == "invalid_grant") "the sign-in took too long: try again" else e.message.orEmpty())
    } catch (e: IOException) {
        SignInState.Failed(describe(e))
    }

    /** Handles [callback] to the end: what to show, or `null` when it answers no waiting sign-in. */
    suspend fun finish(callback: Callback): SignInState? = when (val outcome = handle(callback)) {
        CallbackOutcome.Ignored -> null
        is CallbackOutcome.Failed -> SignInState.Failed(outcome.message)
        is CallbackOutcome.Exchange -> complete(outcome)
    }

    companion object {
        /** Sign-ins left waiting longer than this are forgotten (the server's own last 10 minutes from its page). */
        const val FLOW_LIFETIME_MS = 30 * 60_000L

        /**
         * The scopes to ask [metadata]'s server for: all of [SCOPES] it
         * offers. Servers without profile switching don't know "profiles"
         * and would refuse the whole sign-in for it.
         */
        fun scopeFor(metadata: ServerMetadata): String {
            val offered = metadata.scopesSupported
            return SCOPES.filter { offered == null || it in offered }.ifEmpty { listOf("library") }.joinToString(" ")
        }

        /** The sign-in page's address for [flow] (OAuth 2.1 §4.1.1). */
        fun authorizationUrl(endpoint: String, flow: PendingFlow): String {
            val params = listOf(
                "response_type" to "code",
                "client_id" to CLIENT_ID,
                "redirect_uri" to REDIRECT_URI,
                "scope" to flow.scope,
                "state" to flow.state,
                "code_challenge" to Pkce.challenge(flow.verifier),
                "code_challenge_method" to "S256",
            )
            val query = params.joinToString("&") { (name, value) -> "${encode(name)}=${encode(value)}" }
            return endpoint + (if ('?' in endpoint) "&" else "?") + query
        }

        /** Percent-encodes all but RFC 3986's unreserved characters. */
        fun encode(text: String): String = buildString {
            for (byte in text.toByteArray(Charsets.UTF_8)) {
                val c = byte.toInt() and 0xff
                if (c in 'A'.code..'Z'.code || c in 'a'.code..'z'.code || c in '0'.code..'9'.code || c.toChar() in "-._~") {
                    append(c.toChar())
                } else {
                    append('%').append("0123456789ABCDEF"[c shr 4]).append("0123456789ABCDEF"[c and 0xf])
                }
            }
        }

        /**
         * Opens [url] in a Custom Tab (RFC 8252: the browser, never a web
         * view), or in the browser when none offers Custom Tabs.
         */
        fun open(context: Context, url: String) {
            val uri = url.toUri()
            val browser = CustomTabsClient.getPackageName(context, null)
            try {
                if (browser != null) {
                    val tab = CustomTabsIntent.Builder().setShowTitle(true).build()
                    tab.intent.setPackage(browser)
                    if (context !is Activity) tab.intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    tab.launchUrl(context, uri)
                } else {
                    val view = Intent(Intent.ACTION_VIEW, uri).addCategory(Intent.CATEGORY_BROWSABLE)
                    if (context !is Activity) view.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    context.startActivity(view)
                }
            } catch (_: ActivityNotFoundException) {
                throw AuthException("there's no browser on this device to sign in with")
            }
        }
    }
}

/**
 * The app's sign-in, one per process: the stored tokens, one [Tokens] per
 * server (so refreshes stay one at a time), and the state of the sign-in
 * the browser comes back to.
 */
object Auth {
    @Volatile private var parts: Parts? = null
    private val tokens = ConcurrentHashMap<String, Tokens>()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow<SignInState>(SignInState.Idle)

    /** The last sign-in's progress: Finishing, then SignedIn or Failed. */
    val state: StateFlow<SignInState> = _state.asStateFlow()

    private class Parts(val http: OkHttpClient, val store: TokenStore, val signIn: SignIn)

    private fun parts(context: Context): Parts = parts ?: synchronized(this) {
        parts ?: run {
            // The application's context only: no activity is kept.
            val app = context.applicationContext
            val http = AuthHttp.endpoints()
            val store = TokenStore(PrefsShelf(app), KeystoreVault())
            Parts(http, store, SignIn(http, store, { issuer -> tokens(app, issuer) })).also { parts = it }
        }
    }

    /** The tokens of [issuer] (a server's address), shared by every client of the app. */
    fun tokens(context: Context, issuer: String): Tokens = tokens.getOrPut(issuer) {
        val p = parts(context)
        Tokens(issuer, p.store, p.http)
    }

    /** An OkHttp client for [issuer]'s API, images and streams, with its tokens. */
    fun client(context: Context, issuer: String): OkHttpClient =
        AuthHttp.api(parts(context).http, tokens(context, issuer))

    /**
     * Signs in to [server] from [activity]: its sign-in page opens in the
     * browser, and [activity] is brought back when the browser returns.
     */
    suspend fun begin(activity: Activity, server: String) {
        val url = parts(activity).signIn.start(server, returnTo = activity.javaClass.name)
        _state.value = SignInState.Idle
        SignIn.open(activity, url)
    }

    /** The browser came back with [uri] (see SignInRedirectActivity); `false` when it answers no waiting sign-in. */
    internal fun callback(context: Context, uri: String, done: (PendingFlow) -> Unit): Boolean {
        val signIn = parts(context).signIn
        val callback = Callback.parse(uri) ?: return false
        val outcome = signIn.handle(callback)
        when (outcome) {
            CallbackOutcome.Ignored -> return false
            is CallbackOutcome.Failed -> {
                _state.value = SignInState.Failed(outcome.message)
                done(outcome.flow)
            }
            is CallbackOutcome.Exchange -> {
                _state.value = SignInState.Finishing
                done(outcome.flow)
                scope.launch {
                    _state.value = try {
                        signIn.complete(outcome)
                    } catch (e: CancellationException) {
                        throw e
                    } catch (_: Exception) {
                        // Anything complete doesn't word itself fails the sign-in, never the app.
                        SignInState.Failed("the sign-in couldn't be finished: try again")
                    }
                }
            }
        }
        return true
    }
}
