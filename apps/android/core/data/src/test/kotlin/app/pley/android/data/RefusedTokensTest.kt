package app.pley.android.data

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import app.pley.android.auth.AuthHttp
import app.pley.android.auth.DeviceGrant
import app.pley.android.auth.DevicePoll
import app.pley.android.auth.SignedOutException
import app.pley.android.common.SavedServer
import app.pley.android.fixtures.FixtureServer
import app.pley.android.network.NetworkGraph
import app.pley.android.testing.auth.FakeAuthGraph
import app.pley.android.testing.data.MemoryLegacyPreferences
import app.pley.android.testing.data.TestData
import java.io.File
import java.util.Collections
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The app as DataGraph and NetworkGraph wire it, on a FixtureServer that
 * signs in for real: a server that refuses the app's tokens (one that
 * forgot its grants, as the demo server does when the app restarts, or
 * one that refuses even a token just refreshed) signs the app out of it,
 * and a server that isn't listening yet fails the
 * screens' loads, then answers them once it is.
 */
@RunWith(RobolectricTestRunner::class)
class RefusedTokensTest {
    @get:Rule
    val folder = TemporaryFolder()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val servers = mutableListOf<FixtureServer>()
    private val runs = mutableListOf<CoroutineScope>()

    @After
    fun end() {
        runs.forEach { it.cancel() }
        servers.forEach { it.close() }
    }

    private fun server(port: Int = 0): FixtureServer =
        FixtureServer(FixtureServer.Options(scanEveryMs = 0, deviceIntervalSeconds = 1)).also {
            it.start(port)
            servers += it
        }

    private val endpoints = AuthHttp.endpoints("pley-test", OkHttpClient())

    /** One run of the app on [dir], signed in as [auth] says. */
    private inner class AppRun(dir: File, val auth: FakeAuthGraph = FakeAuthGraph(endpoints)) {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { runs += it }
        val network = NetworkGraph.create(context, OkHttpClient(), auth, TestData.PHONE, File(dir, "cache"))
        val graph = AppDataGraph(
            DataEnvironment(
                filesDir = { File(dir, "files") },
                cacheDir = { File(dir, "cache") },
                auth = auth,
                api = network::api,
                events = network::events,
                trackDefaults = HttpTrackDefaults { network.media(it) },
                profile = TestData.PHONE,
                legacy = MemoryLegacyPreferences(),
                foreground = MutableStateFlow(true),
                debuggable = true,
                scope = scope,
            ),
        )

        /** Signs in to [issuer] with a TV's code, approved at once, and opens it. */
        suspend fun signIn(issuer: String) {
            val grant = DeviceGrant(endpoints, auth::tokens)
            val code = grant.start(issuer)
            val end = grant.poll(code).first { it != DevicePoll.Waiting }
            assertEquals(DevicePoll.SignedIn(issuer), end)
            graph.servers.use(SavedServer(issuer, "Demo", null, 0))
        }

        /** Ends this run, as the process would end. */
        fun stop() = scope.cancel()

        /** A new run on the same folder, with the tokens this one kept. */
        fun restart(dir: File, issuer: String): AppRun {
            stop()
            val kept = FakeAuthGraph(endpoints)
            auth.store.load(issuer)?.let(kept.store::save)
            return AppRun(dir, kept)
        }
    }

    /** Everything [flow] gives, in order, while the run lasts; the scans read as the shells and Home do. */
    private fun <T> AppRun.record(flow: Flow<T>): MutableList<T> {
        val seen = Collections.synchronizedList(mutableListOf<T>())
        graph.liveUpdates.scans
        scope.launch { flow.collect { seen += it } }
        return seen
    }

    /** Whether [this] ended Failed because the app was signed out. */
    private fun <T> List<Loadable<T>>.signedOut(): Boolean = (lastOrNull() as? Loadable.Failed)?.error is SignedOutException

    /** Waits up to [ms] for [check], saying what was seen when it never holds. */
    private fun eventually(ms: Long, what: () -> String, check: () -> Boolean) = runBlocking {
        try {
            withTimeout(ms) {
                while (!check()) kotlinx.coroutines.delay(50)
            }
        } catch (_: kotlinx.coroutines.TimeoutCancellationException) {
            fail("not within $ms ms: ${what()}")
        }
    }

    private fun <T> List<Loadable<T>>.shown(): String = synchronized(this) {
        joinToString { s ->
            when (s) {
                Loadable.Loading -> "Loading"
                is Loadable.Ready -> "Ready(fresh=${s.fresh})"
                is Loadable.Failed -> "Failed(${s.error}, stale=${s.stale != null})"
            }
        }
    }

    /** Signs in on [server] and shows Home once, so the next run has snapshots. */
    private fun firstRun(dir: File, server: FixtureServer): AppRun {
        val run = AppRun(dir)
        runBlocking { run.signIn(server.url) }
        val home = run.record(run.graph.hubs.home())
        val libraries = run.record(run.graph.libraries.libraries())
        eventually(10_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            home.lastOrNull() is Loadable.Ready && (home.last() as Loadable.Ready).fresh &&
                libraries.lastOrNull() is Loadable.Ready && (libraries.last() as Loadable.Ready).fresh
        }
        return run
    }

    @Test
    fun `a server that forgot its grants signs the app out of it`() {
        val dir = folder.newFolder()
        val before = server()
        val issuer = before.url
        val port = issuer.substringAfterLast(':').toInt()
        val first = firstRun(dir, before)

        // The server restarts with no grants; the app restarts with its tokens.
        before.close()
        server(port)
        val app = first.restart(dir, issuer)
        val states = app.record(app.graph.session.state)
        val home = app.record(app.graph.hubs.home())
        val libraries = app.record(app.graph.libraries.libraries())

        eventually(15_000, { "session ${states.toList()}; home ${home.shown()}; libraries ${libraries.shown()}" }) {
            states.lastOrNull() == SessionState.SignedOut(issuer)
        }
        assertTrue(app.auth.store.load(issuer) == null)
        // Not skeletons for ever: "sign in again".
        eventually(5_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            home.signedOut() && libraries.signedOut()
        }
    }

    @Test
    fun `a server that forgot its grants and starts after the app signs it out too`() {
        val dir = folder.newFolder()
        val before = server()
        val issuer = before.url
        val port = issuer.substringAfterLast(':').toInt()
        val first = firstRun(dir, before)

        // As the demo server: nothing listens when the app starts, then a fresh server does.
        before.close()
        val app = first.restart(dir, issuer)
        val states = app.record(app.graph.session.state)
        val home = app.record(app.graph.hubs.home())
        val libraries = app.record(app.graph.libraries.libraries())
        eventually(10_000, { "home ${home.shown()}" }) { home.lastOrNull() is Loadable.Failed }
        server(port)

        eventually(20_000, { "session ${states.toList()}; home ${home.shown()}; libraries ${libraries.shown()}" }) {
            states.lastOrNull() == SessionState.SignedOut(issuer)
        }
        eventually(5_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            home.signedOut() && libraries.signedOut()
        }
    }

    @Test
    fun `a server that refuses even a freshly refreshed token signs the app out of it`() {
        val dir = folder.newFolder()
        val server = server()
        val issuer = server.url
        val first = firstRun(dir, server)

        // Its token check no longer takes this sign-in; refreshes still answer.
        server.refuseAccessTokens = true
        val app = first.restart(dir, issuer)
        val states = app.record(app.graph.session.state)
        val home = app.record(app.graph.hubs.home())
        val libraries = app.record(app.graph.libraries.libraries())

        eventually(15_000, { "session ${states.toList()}; home ${home.shown()}; libraries ${libraries.shown()}" }) {
            states.lastOrNull() == SessionState.SignedOut(issuer)
        }
        assertTrue(app.auth.store.load(issuer) == null)
        eventually(5_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            home.signedOut() && libraries.signedOut()
        }
    }

    @Test
    fun `a server not listening yet fails the loads, then answers them once it listens`() {
        val dir = folder.newFolder()
        val server = server()
        val issuer = server.url
        val first = firstRun(dir, server)

        server.mode = FixtureServer.Mode.Down
        val app = first.restart(dir, issuer)
        val home = app.record(app.graph.hubs.home())
        val libraries = app.record(app.graph.libraries.libraries())
        eventually(10_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            home.lastOrNull() is Loadable.Failed && libraries.lastOrNull() is Loadable.Failed
        }

        server.mode = FixtureServer.Mode.Normal
        eventually(20_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            (home.lastOrNull() as? Loadable.Ready)?.fresh == true && (libraries.lastOrNull() as? Loadable.Ready)?.fresh == true
        }
    }

    @Test
    fun `a first start on a server not listening yet fails its loads, never waits`() {
        val dir = folder.newFolder()
        val server = server()
        val issuer = server.url
        val first = AppRun(dir)
        runBlocking { first.signIn(issuer) }

        // Signed in, nothing saved yet: no profile known on this device.
        server.mode = FixtureServer.Mode.Down
        val app = first.restart(dir, issuer)
        val home = app.record(app.graph.hubs.home())
        val libraries = app.record(app.graph.libraries.libraries())
        eventually(10_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            home.lastOrNull() is Loadable.Failed && libraries.lastOrNull() is Loadable.Failed
        }

        server.mode = FixtureServer.Mode.Normal
        eventually(20_000, { "home ${home.shown()}; libraries ${libraries.shown()}" }) {
            (home.lastOrNull() as? Loadable.Ready)?.fresh == true && (libraries.lastOrNull() as? Loadable.Ready)?.fresh == true
        }
    }
}
