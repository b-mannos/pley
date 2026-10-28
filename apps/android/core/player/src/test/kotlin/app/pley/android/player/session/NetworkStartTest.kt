@file:OptIn(ExperimentalCoroutinesApi::class)

package app.pley.android.player.session

import app.pley.android.model.NetworkReport
import app.pley.android.model.PlaybackNetwork
import app.pley.android.model.SpeedSample
import app.pley.android.network.ApiException
import app.pley.android.network.OfflineException
import app.pley.android.network.PleyApi
import app.pley.android.testing.network.FakePleyApi
import java.net.ConnectException
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Automatic's start from what the server knows of the network (docs/QUALITY.md), on virtual time. */
class NetworkStartTest {
    private val fake = FakePleyApi()

    private fun TestScope.start(api: PleyApi = fake) =
        NetworkStart(api, { "android-test" }, SessionReporter(backgroundScope), { testScheduler.currentTime })

    private suspend fun NetworkStart.ceiling(): StartCeiling = resolve(lookUp())

    private fun reports(): List<NetworkReport> = fake.calls("reportNetwork").map { it.args[0] as NetworkReport }

    @Test
    fun `the server's ceiling, or no limit, takes the device's place`() = runTest {
        fake.answer("playbackNetwork", PlaybackNetwork(ceilingKbps = 8_000, source = "playback"))
        assertEquals(StartCeiling.Server(8_000), start().ceiling())
        fake.answer("playbackNetwork", PlaybackNetwork(local = true))
        assertEquals(StartCeiling.Server(null), start().ceiling())
        assertEquals("android-test", fake.calls("playbackNetwork").first().args[0])
        assertTrue(fake.calls("speedTest").isEmpty())
    }

    @Test
    fun `an older server or one out of reach leaves it to the device, and an older one is asked nothing more`() = runTest {
        fake.fail("playbackNetwork", OfflineException(ConnectException("away")))
        val away = start()
        assertEquals(StartCeiling.Device, away.ceiling())
        away.report(4_000)
        runCurrent()
        assertEquals(1, reports().size)

        fake.reset()
        fake.fail("playbackNetwork", ApiException(404, "not_found", "no such session"))
        val older = start()
        assertEquals(StartCeiling.Device, older.ceiling())
        older.report(4_000)
        runCurrent()
        assertTrue(reports().isEmpty())
        assertEquals(StartCeiling.Device, older.ceiling())
        assertEquals(1, fake.calls("playbackNetwork").size)
    }

    @Test
    fun `a lookup slower than 1_5 s leaves it to the device`() = runTest {
        val slow = object : PleyApi by fake {
            override suspend fun playbackNetwork(clientId: String): PlaybackNetwork {
                delay(5_000)
                return PlaybackNetwork(ceilingKbps = 2_000)
            }
        }
        val n = start(slow)
        val got = async { n.ceiling() }
        advanceTimeBy(NetworkStart.LOOKUP_MS + 1)
        runCurrent()
        assertEquals(StartCeiling.Device, got.await())
    }

    @Test
    fun `a speed test that couldn't bring it all keeps the server's 70 %, reported`() = runTest {
        fake.answer("playbackNetwork", PlaybackNetwork(probe = true))
        // 5 MB in 2 s: 20 Mbps.
        fake.answer("speedTest", SpeedSample(bytes = 5_000_000, ms = 2_000, complete = false))
        fake.answer("reportNetwork", PlaybackNetwork(ceilingKbps = 14_000, source = "probe"))
        assertEquals(StartCeiling.Server(14_000), start().ceiling())
        val asked = fake.calls("speedTest").single().args
        assertEquals(NetworkStart.PROBE_BYTES, asked[0])
        assertEquals(NetworkStart.READ_MS, asked[1])
        assertTrue((asked[2] as Long) <= NetworkStart.BUDGET_MS)
        assertEquals(listOf<NetworkReport>(NetworkReport.Probe("android-test", 20_000, complete = false)), reports())
    }

    @Test
    fun `all of it within the time is no limit`() = runTest {
        fake.answer("playbackNetwork", PlaybackNetwork(probe = true))
        fake.answer("speedTest", SpeedSample(bytes = NetworkStart.PROBE_BYTES, ms = 900, complete = true))
        assertEquals(StartCeiling.Server(null), start().ceiling())
        assertTrue((reports().single() as NetworkReport.Probe).complete)
    }

    @Test
    fun `under 256 KiB says nothing, and starts as if nothing were known`() = runTest {
        fake.answer("playbackNetwork", PlaybackNetwork(probe = true))
        fake.answer("speedTest", SpeedSample(bytes = 100_000, ms = 2_000, complete = false))
        assertEquals(StartCeiling.Server(null), start().ceiling())
        assertTrue(reports().isEmpty())
        // A test that failed altogether, the same.
        fake.fail("speedTest", OfflineException(ConnectException("away")))
        assertEquals(StartCeiling.Server(null), start().ceiling())
        assertTrue(reports().isEmpty())
    }

    @Test
    fun `a late answer to the report doesn't hold the start, 70 % of what was measured`() = runTest {
        val late = object : PleyApi by fake {
            override suspend fun reportNetwork(report: NetworkReport): PlaybackNetwork {
                delay(10_000)
                return PlaybackNetwork(ceilingKbps = 1)
            }
        }
        fake.answer("playbackNetwork", PlaybackNetwork(probe = true))
        fake.answer("speedTest", SpeedSample(bytes = 5_000_000, ms = 2_000, complete = false))
        val n = start(late)
        val began = testScheduler.currentTime
        val got = async { n.ceiling() }
        advanceTimeBy(NetworkStart.BUDGET_MS + 1)
        runCurrent()
        assertEquals(StartCeiling.Server(14_000), got.await())
        assertTrue(testScheduler.currentTime - began <= NetworkStart.BUDGET_MS + 1)
    }

    @Test
    fun `on the server's own network there is never a test`() = runTest {
        fake.answer("playbackNetwork", PlaybackNetwork(local = true, probe = true))
        assertEquals(StartCeiling.Server(null), start().ceiling())
        assertTrue(fake.calls("speedTest").isEmpty())
    }

    @Test
    fun `ceilings go to the server, null forgetting`() = runTest {
        val n = start()
        n.report(8_000)
        n.report(null)
        runCurrent()
        assertEquals(
            listOf<NetworkReport>(NetworkReport.Ceiling("android-test", 8_000), NetworkReport.Ceiling("android-test", null)),
            reports(),
        )
        assertFalse(reports().any { it is NetworkReport.Probe })
    }
}
