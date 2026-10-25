package app.pley.android.auth

import android.content.Context
import android.os.Bundle
import android.util.Log
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.pley.android.common.UserAgent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A TV signing in with a code against a disposable pley server, approved
 * from the computer (docs/ANDROID.md). Runs only when given the server:
 * `-e pleyServer http://127.0.0.1:<port>` with `adb reverse` to it. It
 * prints the code (logcat tag PleyE2E), waits for the approval, checks the
 * tokens are a viewer's, waits [HOLD_MS] so the computer can see the app in
 * Account → Apps, and signs out.
 */
@RunWith(AndroidJUnit4::class)
class DeviceSignInOnServerTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()

    private fun say(line: String) {
        Log.i(TAG, line)
        InstrumentationRegistry.getInstrumentation().sendStatus(0, Bundle().apply { putString("stream", "$TAG $line\n") })
    }

    @Test
    fun aTvSignsInWithACode() = runBlocking {
        val server = InstrumentationRegistry.getArguments().getString("pleyServer")
        assumeTrue("no pleyServer given", server != null)
        val root = OkHttpClient()
        val graph = AuthGraph.create(context, root, UserAgent.of("e2e"))

        val code = graph.deviceSignIn.start(server!!)
        say("USER_CODE=${code.userCode} URI=${code.verificationUri} INTERVAL=${code.intervalSeconds}")
        val outcome = withTimeout(5 * 60_000) { graph.deviceSignIn.poll(code).first { it != DevicePoll.Waiting } }
        say("POLL=$outcome")
        assertEquals(DevicePoll.SignedIn(code.issuer), outcome)
        assertTrue(graph.hasTokens(code.issuer))

        val tokens = graph.tokens(code.issuer)
        val api = AuthHttp.api(root, tokens)
        val me = withContext(Dispatchers.IO) {
            api.newCall(Request.Builder().url("${code.issuer}/api/auth/me").build()).execute().use { JSONObject(it.body.string()) }
        }
        val user = me.optJSONObject("user") ?: me
        say("ME username=${user.optString("username")} is_admin=${user.optBoolean("is_admin")} scope=${tokens.scopes().sorted()}")
        assertFalse(user.optBoolean("is_admin", true))

        say("SIGNED_IN holding ${HOLD_MS / 1000} s")
        Thread.sleep(HOLD_MS)
        tokens.signOut()
        assertFalse(graph.hasTokens(code.issuer))
        say("SIGNED_OUT")
    }

    private companion object {
        const val TAG = "PleyE2E"
        const val HOLD_MS = 20_000L
    }
}
