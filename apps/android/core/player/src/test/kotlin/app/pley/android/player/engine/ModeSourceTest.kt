package app.pley.android.player.engine

import app.pley.android.fixtures.FixtureIds
import app.pley.android.model.DynamicRange
import app.pley.android.player.PlayRequest
import app.pley.android.player.session.SessionHarness
import app.pley.android.testing.caps.TestDevices
import app.pley.android.testing.caps.capsOf
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * What the engine gets to choose a TV's display mode by: the session
 * passes the video stream as it will arrive and the item's length, before
 * the first frame (TV addendum §5.5).
 */
@RunWith(RobolectricTestRunner::class)
class ModeSourceTest {
    @Test
    fun `the engine learns the video's frame rate, range and length before it plays`() {
        SessionHarness(caps = capsOf(TestDevices.sdrPhone)).use { harness ->
            harness.play(PlayRequest(FixtureIds.SAMPLE_1080P))
            val source = harness.engine.source
            assertNotNull(source)
            val video = source!!.video
            assertNotNull(video)
            assertEquals(24.0, video!!.frameRate!!, 0.001)
            assertEquals(DynamicRange.Sdr, video.dynamicRange ?: DynamicRange.Sdr)
            assertTrue("the item's length: ${source.durationMs}", (source.durationMs ?: 0) > 0)
        }
    }
}
