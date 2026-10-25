package app.pley.android.auth

import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import androidx.browser.customtabs.CustomTabsIntent
import androidx.browser.customtabs.CustomTabsService
import androidx.core.content.edit
import androidx.test.core.app.ApplicationProvider
import okhttp3.OkHttpClient
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

/** The sign-in page's tab, and the module as the app makes it. */
@RunWith(RobolectricTestRunner::class)
class SignInTabTest {
    private val app = ApplicationProvider.getApplicationContext<Application>()
    private val url = "http://192.168.1.20:7539/oauth/authorize?response_type=code"

    /** A browser offering Custom Tabs, and ephemeral ones when [ephemeral]. */
    private fun installBrowser(ephemeral: Boolean) {
        val pm = shadowOf(app.packageManager)
        val activity = ComponentName(BROWSER, "$BROWSER.Main")
        pm.addActivityIfNotPresent(activity)
        pm.addIntentFilterForActivity(
            activity,
            IntentFilter(Intent.ACTION_VIEW).apply {
                addCategory(Intent.CATEGORY_DEFAULT)
                addCategory(Intent.CATEGORY_BROWSABLE)
                addDataScheme("http")
                addDataScheme("https")
            },
        )
        val service = ComponentName(BROWSER, "$BROWSER.Tabs")
        pm.addServiceIfNotPresent(service)
        pm.addIntentFilterForService(
            service,
            IntentFilter(CustomTabsService.ACTION_CUSTOM_TABS_CONNECTION).apply {
                if (ephemeral) addCategory(CustomTabsService.CATEGORY_EPHEMERAL_BROWSING)
            },
        )
    }

    private fun opened(): Intent {
        val intent = shadowOf(app).nextStartedActivity
        assertNotNull("nothing was opened", intent)
        return intent
    }

    @Test
    fun theTabIsPrivateWhereTheBrowserOffersIt() {
        installBrowser(ephemeral = true)
        SignIn.open(app, url)
        val intent = opened()
        assertEquals(BROWSER, intent.`package`)
        assertEquals(url, intent.dataString)
        assertTrue(intent.getBooleanExtra(CustomTabsIntent.EXTRA_ENABLE_EPHEMERAL_BROWSING, false))
        // From the application, a task of its own.
        assertTrue(intent.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0)
    }

    @Test
    fun theSettingTurnsItOff() {
        installBrowser(ephemeral = true)
        SignIn.open(app, url, privateTab = false)
        assertFalse(opened().getBooleanExtra(CustomTabsIntent.EXTRA_ENABLE_EPHEMERAL_BROWSING, false))
    }

    @Test
    fun browsersWithoutItGetAnOrdinaryTab() {
        installBrowser(ephemeral = false)
        SignIn.open(app, url)
        val intent = opened()
        assertEquals(BROWSER, intent.`package`)
        assertFalse(intent.hasExtra(CustomTabsIntent.EXTRA_ENABLE_EPHEMERAL_BROWSING))
    }

    @Test
    fun theGraphReadsTokensWithoutTheKeystore() {
        val root = OkHttpClient()
        val graph = AuthGraph.create(app, root, "pleyTest/1")
        val issuer = "http://192.168.1.20:7539"
        assertFalse(graph.hasTokens(issuer))
        // What the sign-in keeps, sealed: the start gate only looks for it.
        app.getSharedPreferences("sign_in", Context.MODE_PRIVATE).edit(commit = true) { putString("tokens:$issuer", "c2VhbGVk") }
        assertTrue(graph.hasTokens(issuer))
        assertTrue(graph.tokens(issuer).signedIn.value)
        // The sign-in's clients derive from the app's root: one connection pool.
        assertTrue(Auth.client(app, issuer).connectionPool === root.connectionPool)
    }

    private companion object {
        const val BROWSER = "org.example.browser"
    }
}
