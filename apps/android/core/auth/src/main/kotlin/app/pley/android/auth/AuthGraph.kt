package app.pley.android.auth

import android.content.Context
import app.pley.android.common.NotBuiltYet
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import okhttp3.OkHttpClient

/**
 * Signing in, for the rest of the app: the tokens of each server, the
 * household's profiles, and the TV's sign-in with a code. One per process.
 */
interface AuthGraph {
    /** The tokens of [issuer] (a server's origin), shared by every client. */
    fun tokens(issuer: String): Tokens

    /**
     * Whether the app holds tokens for [issuer], read without the Keystore,
     * so the start screen can be chosen before the first frame.
     */
    fun hasTokens(issuer: String): Boolean

    /** The profiles of [issuer]'s household, over [api] (a client with its tokens). */
    fun profiles(issuer: String, api: OkHttpClient): Profiles

    /** Signing in with a code shown on screen (TVs, and phones without a browser). */
    val deviceSignIn: DeviceSignIn

    companion object {
        /**
         * The sign-in module over the app's [root] client, introducing
         * itself as [userAgent]. Today the built module keeps its own
         * clients; the shared one comes with the device sign-in.
         */
        fun create(context: Context, root: OkHttpClient, userAgent: String): AuthGraph =
            BuiltAuthGraph(context.applicationContext)
    }
}

/** The module as built: browser sign-in and profiles; no tokens check before the Keystore, no device sign-in. */
private class BuiltAuthGraph(private val context: Context) : AuthGraph {
    override fun tokens(issuer: String): Tokens = Auth.tokens(context, issuer)

    // Not built yet: the start gate treats every server as signed out.
    override fun hasTokens(issuer: String): Boolean = false

    override fun profiles(issuer: String, api: OkHttpClient): Profiles = Profiles(api, tokens(issuer))

    override val deviceSignIn: DeviceSignIn = object : DeviceSignIn {
        override suspend fun start(server: String): DeviceCode = throw NotBuiltYet("Signing in with a code")

        override fun poll(code: DeviceCode): Flow<DevicePoll> = flow { throw NotBuiltYet("Signing in with a code") }
    }
}
