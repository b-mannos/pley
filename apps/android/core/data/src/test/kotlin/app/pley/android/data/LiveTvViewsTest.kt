package app.pley.android.data

import androidx.datastore.core.DataStore
import androidx.datastore.core.okio.OkioStorage
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.PreferencesSerializer
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import okio.FileSystem
import okio.Path.Companion.toPath
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class LiveTvViewsTest {
    @get:Rule
    val folder = TemporaryFolder()

    /** On Okio as the app's are: java.io's rename can't replace the file on a Windows host, and the second write would fail. */
    private fun TestScope.store(): DataStore<Preferences> =
        PreferenceDataStoreFactory.create(
            storage = OkioStorage(FileSystem.SYSTEM, PreferencesSerializer) { File(folder.root, "live_tv.preferences_pb").absolutePath.toPath() },
            scope = backgroundScope,
        )

    private fun profile(issuer: String, userId: Long): ScopeState = ScopeState.Ready(ProfileScope(issuer, userId))

    @Test
    fun `each profile of each server keeps its own category, view and channel`() = runTest {
        val scopes = MutableStateFlow(profile("http://nas:7539", 1))
        val views = DataStoreLiveTvViews(store(), scopes)
        views.keep(category = "group:UK| Sport", view = "Channels")
        views.keep(channel = 101)
        assertEquals(LiveTvView("group:UK| Sport", "Channels", 101), views.of())

        scopes.value = profile("http://nas:7539", 2)
        assertEquals("another profile", LiveTvView(), views.of())
        scopes.value = profile("http://other:7539", 1)
        assertEquals("another server", LiveTvView(), views.of())

        scopes.value = profile("http://nas:7539", 1)
        views.keep(category = "favorites")
        assertEquals("the rest kept as it was", LiveTvView("favorites", "Channels", 101), views.of())
    }

    @Test
    fun `signed out nothing is read or kept`() = runTest {
        val views = DataStoreLiveTvViews(store(), MutableStateFlow(ScopeState.SignedOut))
        views.keep(category = "all", channel = 5)
        assertEquals(LiveTvView(), views.of())
    }
}
