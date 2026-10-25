package app.pley.android.auth

import java.util.concurrent.atomic.AtomicInteger
import javax.crypto.KeyGenerator
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/** Whether the app is signed in, read without the Keystore, and the events of sign-ins that end. */
@OptIn(ExperimentalCoroutinesApi::class)
class SignedInStateTest {
    private val server = MockWebServer()
    private val shelf = MemoryShelf()
    private val issuer by lazy { issuerOf(server) }
    private val events = AuthEvents()

    /** A vault that counts how often its key is asked for: the Keystore's work. */
    private val opened = AtomicInteger()
    private val key = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
    private val vault = Vault { opened.incrementAndGet().let { key } }

    @After
    fun stop() = server.shutdown()

    @Test
    fun hasTokensNeverOpensTheVault() {
        TokenStore(shelf, vault).save(savedTokens(issuer))
        opened.set(0)
        val store = TokenStore(shelf, vault)
        assertTrue(store.has(issuer))
        assertFalse(store.has("http://other:7539"))
        // Nor does making the tokens: the first request opens them.
        val tokens = Tokens(issuer, store, endpointsClient())
        assertTrue(tokens.signedIn.value)
        assertEquals(0, opened.get())
        assertEquals("pley_at_one", runBlocking { tokens.accessToken() })
        assertEquals(1, opened.get())
        store.wipe(issuer)
        assertFalse(store.has(issuer))
    }

    @Test
    fun unreadableTokensSignOutAndSaySo() = runTest {
        val heard = mutableListOf<AuthEvent>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { events.events.collect { heard += it } }
        TokenStore(shelf, testVault()).save(savedTokens(issuer))
        // Another key: the Keystore's went with a reset.
        val store = TokenStore(shelf, vault) { events.signedOut(it, SignOutReason.UNREADABLE) }
        val tokens = Tokens(issuer, store, endpointsClient(), events)
        assertTrue(tokens.signedIn.value)
        assertNull(tokens.accessToken())
        assertFalse(tokens.signedIn.value)
        assertFalse(store.has(issuer))
        assertEquals(listOf<AuthEvent>(AuthEvent.SignedOut(issuer, SignOutReason.UNREADABLE)), heard)
    }

    @Test
    fun aSignInTheServerEndedSaysSo() = runTest {
        val heard = mutableListOf<AuthEvent>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { events.events.collect { heard += it } }
        server.enqueue(MockResponse().setResponseCode(400).setBody("""{"error":"invalid_grant","error_description":"spent"}"""))
        val store = TokenStore(shelf, vault)
        store.save(savedTokens(issuer))
        val tokens = Tokens(issuer, store, endpointsClient(), events)
        assertThrows(SignedOutException::class.java) { runBlocking { tokens.refresh("pley_at_one") } }
        assertEquals(listOf<AuthEvent>(AuthEvent.SignedOut(issuer, SignOutReason.ENDED)), heard)

        // Signing out on purpose is no news.
        store.save(savedTokens(issuer))
        server.enqueue(MockResponse())
        tokens.signOut()
        assertEquals(1, heard.size)
    }
}
