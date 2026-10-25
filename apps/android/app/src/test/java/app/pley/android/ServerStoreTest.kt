package app.pley.android

import app.pley.android.common.SavedServer
import org.junit.Assert.assertEquals
import org.junit.Test

class ServerStoreTest {
    private val saved = listOf(
        SavedServer("http://nas:7539", "NAS", "a1", 10),
        SavedServer("http://office:7539", "Office", null, 20),
    )

    @Test
    fun savedServersSurviveARoundTrip() {
        assertEquals(saved, ServerStore.decode(ServerStore.encode(saved)))
        assertEquals(emptyList<SavedServer>(), ServerStore.decode("not json"))
        assertEquals(emptyList<SavedServer>(), ServerStore.decode(null))
    }
}
