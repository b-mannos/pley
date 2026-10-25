@file:OptIn(ExperimentalCoroutinesApi::class)

package app.pley.android.player.trickplay

import android.graphics.Bitmap
import app.pley.android.model.TrickplayInfo
import app.pley.android.network.ApiException
import app.pley.android.testing.network.FakePleyApi
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Seek previews: the tile arithmetic, asking again while they are made, and the tile cache. */
@RunWith(RobolectricTestRunner::class)
class TrickplayTest {
    private val info = TrickplayInfo(id = "v1", intervalMs = 10_000, width = 320, height = 180, columns = 10, rows = 10, count = 250, sheets = 3)

    @Test
    fun `tile k is on sheet k over columns times rows, at its column and row`() {
        assertEquals(TileRegion(0, 0, 0, 320, 180), TrickplayTiles.regionOf(info, 0))
        assertEquals(TileRegion(0, 320 * 9, 0, 3_200, 180), TrickplayTiles.regionOf(info, 9))
        assertEquals(TileRegion(0, 0, 180, 320, 360), TrickplayTiles.regionOf(info, 10))
        assertEquals(TileRegion(0, 320 * 9, 180 * 9, 3_200, 1_800), TrickplayTiles.regionOf(info, 99))
        assertEquals(TileRegion(1, 0, 0, 320, 180), TrickplayTiles.regionOf(info, 100))
        assertEquals(TileRegion(2, 320 * 3, 180 * 4, 320 * 4, 180 * 5), TrickplayTiles.regionOf(info, 243))
        // Positions to thumbnails: k × interval, clamped to the last one.
        assertEquals(0, info.thumbnailAt(9_999))
        assertEquals(1, info.thumbnailAt(10_000))
        assertEquals(249, info.thumbnailAt(99_000_000))
    }

    @Test
    fun `while the server makes them it is asked every 20 s, for 10 minutes at most`() {
        val scheduler = TestCoroutineScheduler()
        val scope = CoroutineScope(StandardTestDispatcher(scheduler) + SupervisorJob())
        val api = FakePleyApi()
        api.answer("trickplay", null)
        val thumbs = TrickplayThumbnails(scope, api, 7, { ByteArray(0) })
        thumbs.start()
        scheduler.runCurrent()
        assertEquals(1, api.calls("trickplay").size)
        scheduler.advanceTimeBy(20_001)
        assertEquals(2, api.calls("trickplay").size)
        // Failures count as "not yet" too.
        api.fail("trickplay", ApiException(500, "internal", "boom"))
        scheduler.advanceTimeBy(20_000)
        assertEquals(3, api.calls("trickplay").size)
        api.reset()
        api.answer("trickplay", info)
        scheduler.advanceTimeBy(20_000)
        assertEquals(info, thumbs.info.value)
        scheduler.advanceTimeBy(600_000)
        assertEquals(1, api.calls("trickplay").size)

        val never = FakePleyApi().apply { answer("trickplay", null) }
        TrickplayThumbnails(scope, never, 7, { ByteArray(0) }).start()
        scheduler.advanceTimeBy(3_600_000)
        assertEquals(31, never.calls("trickplay").size)
    }

    @Test
    fun `tiles are cut from sheets downloaded once, cached, in RGB_565 on the smallest TVs`() = runTest {
        val api = FakePleyApi().apply { answer("trickplay", info) }
        val downloaded = mutableListOf<String>()
        val cut = mutableListOf<Pair<TileRegion, Boolean>>()
        val closed = mutableListOf<Int>()
        val decoder = object : SheetDecoder {
            override fun open(bytes: ByteArray): SheetDecoder.Sheet {
                val sheet = bytes.single().toInt()
                return object : SheetDecoder.Sheet {
                    override fun decode(region: TileRegion, lowMemory: Boolean): Bitmap {
                        cut += region to lowMemory
                        return Bitmap.createBitmap(320, 180, if (lowMemory) Bitmap.Config.RGB_565 else Bitmap.Config.ARGB_8888)
                    }

                    override fun close() {
                        closed += sheet
                    }
                }
            }
        }
        val thumbs = TrickplayThumbnails(
            backgroundScope, api, 7,
            { url -> downloaded += url; byteArrayOf(url.substringAfterLast('/').substringBefore('.').toByte()) },
            decoder, lowMemory = true, work = StandardTestDispatcher(testScheduler),
        )
        assertNull(thumbs.thumbnail(0))
        thumbs.start()
        testScheduler.runCurrent()
        val first = thumbs.thumbnail(15_000)!!
        assertSame(first, thumbs.thumbnail(19_999))
        assertEquals(Bitmap.Config.RGB_565, first.config)
        assertEquals(listOf("http://127.0.0.1:7539/api/files/7/trickplay/0.jpg?v=v1"), downloaded)
        assertEquals(listOf(TrickplayTiles.regionOf(info, 1) to true), cut)
        thumbs.thumbnail(1_000_000)
        thumbs.thumbnail(2_000_000)
        // Two sheets stay open; the first is let go.
        assertEquals(3, downloaded.size)
        assertEquals(listOf(0), closed)
        // 16 tiles on the smallest TVs.
        for (k in 0 until 20) thumbs.thumbnail(k * 10_000L)
        assertEquals(listOf(0, 1), closed.take(2))
        thumbs.close()
    }
}
