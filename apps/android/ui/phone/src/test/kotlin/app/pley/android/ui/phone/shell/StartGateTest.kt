package app.pley.android.ui.phone.shell

import app.pley.android.screens.PleyKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StartGateTest {
    @Test
    fun noServerChoosesOne() {
        assertEquals(PleyKey.Servers, startKey(PhoneStart(server = null, signedIn = false)))
        // Tokens without a server can't happen; the server comes first all the same.
        assertEquals(PleyKey.Servers, startKey(PhoneStart(server = null, signedIn = true)))
    }

    @Test
    fun aServerWithoutTokensSignsInThere() {
        assertEquals(PleyKey.SignIn("http://nas:7539"), startKey(PhoneStart("http://nas:7539", signedIn = false)))
    }

    @Test
    fun signedInGoesHome() {
        assertEquals(PleyKey.Home, startKey(PhoneStart("http://nas:7539", signedIn = true)))
    }

    @Test
    fun gettingInIsServersSignInAndLink() {
        assertTrue(PleyKey.Servers.isGettingIn)
        assertTrue(PleyKey.SignIn("x").isGettingIn)
        assertTrue(PleyKey.Link("x").isGettingIn)
        assertFalse(PleyKey.Profiles.isGettingIn)
        assertFalse(PleyKey.Home.isGettingIn)
    }

    @Test
    fun tabsKnowTheirRoots() {
        assertEquals(PhoneTab.Search, PhoneTab.rootOf(PleyKey.Search("north")))
        assertEquals(PhoneTab.Me, PhoneTab.rootOf(PleyKey.Me))
        assertEquals(null, PhoneTab.rootOf(PleyKey.Item(1)))
    }
}
