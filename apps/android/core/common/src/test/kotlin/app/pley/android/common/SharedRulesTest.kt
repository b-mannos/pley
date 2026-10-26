package app.pley.android.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SharedRulesTest {
    @Test
    fun widthsRoundUpToTheServersSizes() {
        assertEquals(160, ImageUrls.bucket(1))
        assertEquals(160, ImageUrls.bucket(159))
        assertEquals(160, ImageUrls.bucket(160))
        assertEquals(240, ImageUrls.bucket(161))
        assertEquals(1920, ImageUrls.bucket(2000))
        assertEquals("never above the screen or the tier's limit", 1280, ImageUrls.bucket(1500, max = 1280))
        assertEquals(160, ImageUrls.bucket(500, max = 100))
    }

    @Test
    fun picturesKeepTheirVersion() {
        assertEquals("/api/items/7/images/poster?v=1700000000&w=160", ImageUrls.sized("/api/items/7/images/poster?v=1700000000", 159))
        assertEquals("/api/items/7/images/poster?v=1700000000&w=240", ImageUrls.sized("/api/items/7/images/poster?v=1700000000", 161))
        assertEquals("/api/items/7/images/art?v=3&w=1920", ImageUrls.sized("/api/items/7/images/art?v=3", 2000))
        assertEquals("a width already there is replaced", "/api/items/7/images/art?v=3&w=480", ImageUrls.sized("/api/items/7/images/art?w=1920&v=3", 400))
        assertEquals("/api/people/2/photo?w=320", ImageUrls.sized("/api/people/2/photo", 300))
    }

    @Test
    fun devicesFallIntoTiers() {
        val gib = 1L shl 30
        assertEquals("a 1 GB Fire TV stick", Tier.T0, DeviceProfile.tierOf(lowRam = false, totalRamBytes = 900L * (1 shl 20)))
        assertEquals("the 1.5 GB Chromecast HD", Tier.T0, DeviceProfile.tierOf(lowRam = false, totalRamBytes = 14 * gib / 10))
        assertEquals("low-RAM devices whatever their memory", Tier.T0, DeviceProfile.tierOf(lowRam = true, totalRamBytes = 4 * gib))
        assertEquals("a 2 GB stick", Tier.T1, DeviceProfile.tierOf(lowRam = false, totalRamBytes = 19 * gib / 10))
        assertEquals("a 3 GB box", Tier.T2, DeviceProfile.tierOf(lowRam = false, totalRamBytes = 28 * gib / 10))
        assertEquals(Tier.T2, DeviceProfile.tierOf(lowRam = false, totalRamBytes = DeviceProfile.T1_BELOW))
    }

    @Test
    fun originsAreWrittenAsBrowsersDo() {
        assertEquals("http://nas:7539", Addresses.origin("http://NAS:7539/library?x=1"))
        assertEquals("https://pley.example.com", Addresses.origin("https://pley.example.com:443/"))
        assertEquals("http://192.168.1.20", Addresses.origin("http://192.168.1.20:80"))
        assertNull(Addresses.origin("nas:7539"))
        assertNull(Addresses.origin("not a url"))
    }

    @Test
    fun requestsSayTheyAreFromTheApp() {
        assertEquals("pleyAndroid/0.1.0 (Android 17)", UserAgent.of("0.1.0", release = "17"))
        assertEquals("pleyAndroid/0.1.0 (Android ?)", UserAgent.of("0.1.0", release = null))
        assertEquals(
            "pleyAndroid/0.1.0 (Android 12; Chromecast HD)",
            UserAgent.of("0.1.0", release = "12", device = "Chromecast HD"),
        )
    }

    @Test
    fun theDevicesNameIsWhatAHeaderCarries() {
        val ua = { name: String -> UserAgent.of("0.1.0", release = "14", device = name) }
        assertEquals("pleyAndroid/0.1.0 (Android 14; Anna's TV - Living room)", ua("Anna’s TV — (Living room);"))
        assertEquals("pleyAndroid/0.1.0 (Android 14; Den TV)", ua("  Den\tTV 📺 "))
        assertEquals("pleyAndroid/0.1.0 (Android 14)", ua("📺"))
        assertEquals(40, UserAgent.cleaned("x".repeat(100)).length)
    }
}
