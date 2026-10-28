package app.pley.android.data

import app.pley.android.testing.data.TestData
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class SettingsTest {
    @get:Rule
    val folder = TemporaryFolder()

    @Test
    fun `defaults are what the app does without being asked`() = runTest {
        val settings = TestData(this, folder.newFolder()).graph.settings.settings.first()
        assertEquals(Settings(), settings)
        // Automatic where no quality was ever chosen (docs/QUALITY.md).
        assertEquals("auto", settings.qualityOnHomeNetwork)
        assertEquals("auto", settings.qualityOnMobile)
        assertTrue(settings.privateSignInTab)
        assertTrue(settings.autoplayNext)
        assertFalse(settings.tunneling)
        assertNull(settings.alwaysStartAs)
        assertEquals(FrameRateMatching.Seamless, settings.frameRateMatching(tv = false))
        assertEquals(FrameRateMatching.Auto, settings.frameRateMatching(tv = true))
        assertEquals(Passthrough.Auto, settings.passthrough("truehd"))
    }

    @Test
    fun `every setting survives a restart`() = runTest {
        val dir = folder.newFolder()
        val changed = Settings(
            qualityOnHomeNetwork = "1080p",
            qualityOnMobile = "720p",
            frameRateMatching = FrameRateMatching.Always,
            passthrough = mapOf("eac3-joc" to Passthrough.On, "dts" to Passthrough.Off),
            tunneling = true,
            subtitleSize = 0.076f,
            subtitleBackground = SubtitleBackground.DarkBox,
            subtitleFont = SubtitleFont.Serif,
            subtitleRaise = 0.1f,
            autoplayNext = false,
            privateSignInTab = false,
            alwaysStartAs = 3,
            showPlaybackDetails = true,
            gridDensity = GridDensity.Large,
            allowSoftwareVideo = true,
        )
        val first = appRun()
        val before = TestData(this, dir, profile = TestData.FIRE_TV_T0, scope = first)
        before.graph.settings.update { changed }
        assertEquals(changed, before.graph.settings.settings.first())
        first.cancel()
        testScheduler.runCurrent()

        val after = TestData(this, dir, profile = TestData.FIRE_TV_T0)
        assertEquals(changed, after.graph.settings.settings.first())

        // Back to the defaults: nothing stays stored.
        after.graph.settings.update { Settings() }
        assertEquals(Settings(), after.graph.settings.settings.first())
    }

    @Test
    fun `the original chosen is kept, as Automatic left untouched is`() = runTest {
        val dir = folder.newFolder()
        val first = appRun()
        val before = TestData(this, dir, scope = first)
        before.graph.settings.update { it.copy(qualityOnHomeNetwork = Settings.ORIGINAL) }
        first.cancel()
        testScheduler.runCurrent()

        val after = TestData(this, dir).graph.settings.settings.first()
        assertEquals(Settings.ORIGINAL, after.qualityOnHomeNetwork)
        assertEquals(Settings.AUTOMATIC, after.qualityOnMobile)
    }

    @Test
    fun `what a device can't have is put back`() = runTest {
        val phone = TestData(this, folder.newFolder(), debuggable = false).graph.settings
        phone.update {
            it.copy(
                frameRateMatching = FrameRateMatching.Always,
                subtitleSize = 0.05f,
                subtitleRaise = 0.07f,
                allowSoftwareVideo = true,
                passthrough = mapOf("mp3" to Passthrough.On, "ac3" to Passthrough.Off),
                qualityOnMobile = " ",
            )
        }
        val s = phone.settings.first()
        // A TV's choice on a phone: the phone's default.
        assertNull(s.frameRateMatching)
        assertEquals(FrameRateMatching.Seamless, s.frameRateMatching(tv = false))
        // The nearest size offered.
        assertEquals(0.048f, s.subtitleSize)
        assertEquals(0.05f, s.subtitleRaise)
        // Release builds never allow software video.
        assertFalse(s.allowSoftwareVideo)
        assertEquals(mapOf("ac3" to Passthrough.Off), s.passthrough)
        assertEquals("auto", s.qualityOnMobile)
    }

    @Test
    fun `phones and TVs offer their own frame-rate choices`() {
        assertEquals(listOf(FrameRateMatching.Seamless, FrameRateMatching.Off), FrameRateMatching.choices(tv = false))
        assertEquals(listOf(FrameRateMatching.Auto, FrameRateMatching.Always, FrameRateMatching.Off), FrameRateMatching.choices(tv = true))
        assertEquals(listOf(0.038f, 0.048f, 0.06f, 0.076f), Settings.SUBTITLE_SIZES)
    }
}
