package app.pley.android.ui.tv.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The 960 × 540 dp canvas fills every TV, whatever density it reports. */
class TvCanvasTest {
    @Test
    fun aTenEightyTvIsTwoPixelsADp() {
        // What most TVs and sticks report anyway (1920 × 1080 at 320 dpi).
        assertEquals(2f, tvCanvasDensity(1920, 1080, 1f)!!.density, 0.0001f)
    }

    @Test
    fun otherResolutionsScaleTheSameCanvas() {
        assertEquals(4f / 3f, tvCanvasDensity(1280, 720, 1f)!!.density, 0.0001f)
        assertEquals(4f, tvCanvasDensity(3840, 2160, 1f)!!.density, 0.0001f)
    }

    @Test
    fun aScreenThatIsNotSixteenByNineFitsItsTighterSide() {
        // 21:9: the height decides, and the canvas grows wider than 960.
        assertEquals(2f, tvCanvasDensity(2560, 1080, 1f)!!.density, 0.0001f)
        // 4:3: the width decides, and the canvas grows taller than 540.
        assertEquals(1.5f, tvCanvasDensity(1440, 1080, 1f)!!.density, 0.0001f)
    }

    @Test
    fun theTextSizeSettingAppliesUpToItsLimit() {
        assertEquals(1.15f, tvCanvasDensity(1920, 1080, 1.15f)!!.fontScale, 0.0001f)
        assertEquals(MAX_FONT_SCALE, tvCanvasDensity(1920, 1080, 2f)!!.fontScale, 0.0001f)
        assertEquals(0.85f, tvCanvasDensity(1920, 1080, 0.85f)!!.fontScale, 0.0001f)
    }

    @Test
    fun aWindowNotMeasuredYetKeepsTheSystemDensity() {
        assertNull(tvCanvasDensity(0, 0, 1f))
    }
}
