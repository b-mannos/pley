package app.pley.android.network

import android.content.Context
import app.pley.android.auth.AuthGraph
import app.pley.android.common.DeviceProfile
import app.pley.android.common.LocalNetwork
import app.pley.android.common.NotBuiltYet
import java.io.File
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import okhttp3.OkHttpClient

/**
 * The network side of the app: one client per purpose, all derived from
 * the root so they share its connections, and the API on top. Made lazily:
 * nothing here runs before the first frame.
 */
interface NetworkGraph {
    /** The client every other derives from. */
    val root: OkHttpClient

    /** The API of [issuer] (a server's origin), as profile [userId] signed in there. */
    fun api(issuer: String, userId: Long?): PleyApi

    /** For pictures: its own limits per host, the server speaking HTTP/1.1. */
    val images: OkHttpClient

    /** For video and audio of [issuer]: long reads, as HLS segments can wait for the transcoder. */
    fun media(issuer: String): OkHttpClient

    /** For subtitles of [issuer], which the transcoder may still be writing. */
    fun subtitles(issuer: String): OkHttpClient

    /** The event stream of [issuer]. */
    fun events(issuer: String): ServerEvents

    val discovery: Discovery
    val sweep: NetworkSweep
    val probe: ServerProbe
    val localNetwork: LocalNetworkAccess

    companion object {
        /** Not built yet: clients are the root itself, and anything else says so. */
        fun create(context: Context, root: OkHttpClient, auth: AuthGraph, profile: DeviceProfile, cacheDir: File): NetworkGraph =
            NotBuiltNetworkGraph(context.applicationContext, root)
    }
}

private class NotBuiltNetworkGraph(private val context: Context, override val root: OkHttpClient) : NetworkGraph {
    override fun api(issuer: String, userId: Long?): PleyApi = throw NotBuiltYet("The API")
    override val images: OkHttpClient get() = root
    override fun media(issuer: String): OkHttpClient = root
    override fun subtitles(issuer: String): OkHttpClient = root
    override fun events(issuer: String): ServerEvents = ServerEvents { notBuilt("Server events") }
    override val discovery = Discovery { notBuilt("Discovery") }
    override val sweep = NetworkSweep { notBuilt("Searching the network") }
    override val probe = ServerProbe { throw NotBuiltYet("Checking an address") }
    override val localNetwork = object : LocalNetworkAccess {
        override fun missing(): List<String> = LocalNetwork.missing(context)

        override fun isPermissionBlock(error: Throwable): Boolean = LocalNetwork.isPermissionBlock(error)
    }

    private fun <T> notBuilt(what: String): Flow<T> = flow { throw NotBuiltYet(what) }
}
