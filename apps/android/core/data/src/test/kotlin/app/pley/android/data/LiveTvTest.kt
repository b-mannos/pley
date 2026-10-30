package app.pley.android.data

import app.pley.android.model.ServerEvent
import app.pley.android.model.WatchRequest
import app.pley.android.network.ApiException
import app.pley.android.network.LiveEvent
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Live TV's channels, favourites, guide chunks and watching. */
class LiveTvTest {
    @get:Rule
    val folder = TemporaryFolder()

    @Test
    fun `a favourite shows at once, and is put back when refused`() = runTest {
        val data = signedIn(folder.newFolder())
        val channels = collect(data.graph.liveTv.channels())
        testScheduler.runCurrent()
        assertFalse(channels.last().valueOrNull!!.first { it.id == 1L }.favorite)

        data.api.hold()
        val change = launch { data.graph.liveTv.setFavorite(1, true) }
        testScheduler.runCurrent()
        assertTrue(channels.last().valueOrNull!!.first { it.id == 1L }.favorite)
        data.api.release()
        change.join()
        assertEquals(listOf<Any?>(1L, true), data.api.calls("setFavoriteChannel").single().args)

        val refused = ApiException(500, "internal", "the server failed")
        data.api.fail("setFavoriteChannel", refused)
        try {
            data.graph.liveTv.setFavorite(2, false)
            fail("refused")
        } catch (e: ApiException) {
            assertSame(refused, e)
        }
        assertTrue(channels.last().valueOrNull!!.first { it.id == 2L }.favorite)
    }

    @Test
    fun `watching sends this device's id, answers an absolute address and leads Recently watched`() = runTest {
        val data = signedIn(folder.newFolder())
        val summary = collect(data.graph.liveTv.summary())
        testScheduler.runCurrent()
        assertEquals(listOf(2L, 1L), summary.last().valueOrNull!!.recent)

        val watching = data.graph.liveTv.watch(5, "android-1")

        assertEquals(listOf<Any?>(5L, WatchRequest("android-1")), data.api.calls("watchChannel").single().args)
        assertEquals("${data.issuer}/api/live/sessions/live-5/index.m3u8", watching.url)
        assertEquals(5L, summary.last().valueOrNull!!.recent.first())
    }

    @Test
    fun `leaving outlives whoever asked, and a failure to is no one's`() = runTest {
        val data = signedIn(folder.newFolder())
        data.graph.liveTv.leave("s1")
        testScheduler.runCurrent()
        assertEquals(listOf<Any?>("s1"), data.api.calls("leaveChannel").single().args)

        data.api.fail("leaveChannel", ApiException(500, "internal", "no"))
        data.graph.liveTv.leave("s2")
        testScheduler.runCurrent()
        assertEquals(2, data.api.calls("leaveChannel").size)
    }

    @Test
    fun `the guide is asked for by chunk, once each`() = runTest {
        val data = signedIn(folder.newFolder())
        val from = 1761742800L - 1761742800L % LiveTvRepository.GUIDE_CHUNK_S
        val first = collect(data.graph.liveTv.guide(from))
        collect(data.graph.liveTv.guide(from))
        testScheduler.runCurrent()
        assertEquals(from, first.last().valueOrNull!!.from)
        assertEquals(listOf<Any?>(from, from + LiveTvRepository.GUIDE_CHUNK_S), data.api.calls("liveGuide").single().args)
    }

    @Test
    fun `the server's Live TV changes ask again for the channels, the summary and the guide`() = runTest {
        val data = signedIn(folder.newFolder())
        collect(data.graph.liveTv.summary())
        collect(data.graph.liveTv.channels())
        collect(data.graph.liveTv.guide(0))
        collect(data.graph.hubs.home())
        data.graph.liveUpdates.start()
        testScheduler.runCurrent()

        data.events.send(LiveEvent.Event(ServerEvent.LiveChannelsChanged))
        testScheduler.runCurrent()
        assertEquals(2, data.api.calls("liveSummary").size)
        assertEquals(2, data.api.calls("liveChannels").size)
        assertEquals(2, data.api.calls("liveGuide").size)
        assertEquals(1, data.api.calls("homeHubs").size)

        // Who watches what: nothing a viewer shows.
        data.events.send(LiveEvent.Event(ServerEvent.LiveTunersChanged))
        testScheduler.runCurrent()
        assertEquals(2, data.api.calls("liveChannels").size)
    }
}
