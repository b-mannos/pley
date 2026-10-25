package app.pley.android.data

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The web view app's own SharedPreferences, as ServerStore and PlayerActivity write them. */
@RunWith(RobolectricTestRunner::class)
class LegacyPreferencesTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun `reads what the web view app wrote`() {
        val saved = """[{"url":"http://nas:7539","name":"NAS","id":null,"last_used":5}]"""
        context.getSharedPreferences("servers", Context.MODE_PRIVATE).edit().putString("saved", saved).commit()
        context.getSharedPreferences("device", Context.MODE_PRIVATE).edit().putString("client_id", "android-1").commit()

        val legacy = LegacyPreferences.of(context)

        assertEquals(saved, legacy.get(LegacyPreferences.SERVERS_FILE, LegacyPreferences.SERVERS_KEY))
        assertEquals("android-1", legacy.get(LegacyPreferences.DEVICE_FILE, LegacyPreferences.CLIENT_ID_KEY))
        assertEquals(listOf("http://nas:7539"), SavedServersJson.decode(legacy.get("servers", "saved")).map { it.url })
        assertNull(legacy.get("servers", "nothing"))
    }

    @Test
    fun `writes where the web view app reads`() {
        val legacy = LegacyPreferences.of(context)
        legacy.put("device", "client_id", "android-2")
        assertEquals("android-2", context.getSharedPreferences("device", Context.MODE_PRIVATE).getString("client_id", null))
    }
}
