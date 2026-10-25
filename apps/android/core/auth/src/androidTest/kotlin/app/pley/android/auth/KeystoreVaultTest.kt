package app.pley.android.auth

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.security.KeyStore
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** The tokens' vault on a real device: a key in the Android Keystore that never leaves it. */
@RunWith(AndroidJUnit4::class)
class KeystoreVaultTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val issuer = "http://192.168.1.20:7539"

    @After
    fun clean() {
        keystore().deleteEntry(ALIAS)
        context.deleteSharedPreferences(FILE)
    }

    private fun keystore() = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }

    @Test
    fun sealsAndOpensWithItsOwnServer() {
        val vault = KeystoreVault(ALIAS)
        val sealed = vault.seal("pley_rt_secret".toByteArray(), issuer)
        assertTrue(keystore().containsAlias(ALIAS))
        assertFalse(String(sealed, Charsets.ISO_8859_1).contains("pley_rt_secret"))
        assertArrayEquals("pley_rt_secret".toByteArray(), KeystoreVault(ALIAS).open(sealed, issuer))
        // Another server's name doesn't open it, nor does a changed byte.
        assertNull(vault.open(sealed, "http://other:7539"))
        assertNull(vault.open(sealed.copyOf().also { it[it.size - 1] = (it[it.size - 1] + 1).toByte() }, issuer))
        // A fresh IV each time.
        assertFalse(sealed.contentEquals(vault.seal("pley_rt_secret".toByteArray(), issuer)))
    }

    @Test
    fun tokensAreKeptReadAndWiped() {
        val store = TokenStore(PrefsShelf(context, FILE), KeystoreVault(ALIAS))
        val tokens = StoredTokens(issuer, "$issuer/api/oauth/token", "$issuer/api/oauth/revoke", "pley_at_a", Long.MAX_VALUE / 2, "pley_rt_a", "library play")
        store.save(tokens)
        // A new process: the same file and key.
        val again = TokenStore(PrefsShelf(context, FILE), KeystoreVault(ALIAS))
        assertTrue(again.has(issuer))
        assertEquals(tokens, again.load(issuer))
        again.wipe(issuer)
        assertFalse(TokenStore(PrefsShelf(context, FILE), KeystoreVault(ALIAS)).has(issuer))
    }

    @Test
    fun tokensWhoseKeyIsGoneAreSignedOut() {
        TokenStore(PrefsShelf(context, FILE), KeystoreVault(ALIAS))
            .save(StoredTokens(issuer, "$issuer/api/oauth/token", "$issuer/api/oauth/revoke", "a", 1, "r", "library"))
        // A reset takes the Keystore's keys.
        keystore().deleteEntry(ALIAS)
        val heard = mutableListOf<String>()
        val store = TokenStore(PrefsShelf(context, FILE), KeystoreVault(ALIAS)) { heard += it }
        assertTrue(store.has(issuer))
        assertNull(store.load(issuer))
        assertFalse(store.has(issuer))
        assertEquals(listOf(issuer), heard)
    }

    private companion object {
        const val ALIAS = "pley_test_sign_in"
        const val FILE = "sign_in_test"
    }
}
