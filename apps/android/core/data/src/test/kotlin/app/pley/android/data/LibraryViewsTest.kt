package app.pley.android.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class LibraryViewsTest {
    @get:Rule
    val folder = TemporaryFolder()

    private fun TestScope.store(): DataStore<Preferences> =
        PreferenceDataStoreFactory.create(scope = backgroundScope) { File(folder.root, "library_views.preferences_pb") }

    private fun profile(issuer: String, userId: Long): ScopeState = ScopeState.Ready(ProfileScope(issuer, userId))

    @Test
    fun `each profile of each server keeps its own view of a library`() = runTest {
        val scopes = MutableStateFlow(profile("http://nas:7539", 1))
        val views = DataStoreLibraryViews(store(), scopes)
        views.keep(3, "sort=added&watched=false")
        assertEquals("sort=added&watched=false", views.of(3))
        assertNull("another library", views.of(4))

        scopes.value = profile("http://nas:7539", 2)
        assertNull("another profile", views.of(3))
        scopes.value = profile("http://other:7539", 1)
        assertNull("another server", views.of(3))

        scopes.value = profile("http://nas:7539", 1)
        views.keep(3, "")
        assertNull("the default view is forgotten", views.of(3))
    }

    @Test
    fun `signed out nothing is read or kept`() = runTest {
        val views = DataStoreLibraryViews(store(), MutableStateFlow(ScopeState.SignedOut))
        views.keep(3, "sort=added")
        assertNull(views.of(3))
    }
}
