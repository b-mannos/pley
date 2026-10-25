package app.pley.android.caps

import android.os.Build
import android.os.Looper
import android.os.StrictMode
import android.os.SystemClock
import android.os.strictmode.Violation
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.pley.android.common.DeviceProfile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CopyOnWriteArrayList

/** The real probe on a device or emulator: off the main thread, quickly, with nothing StrictMode objects to. */
@RunWith(AndroidJUnit4::class)
class ProbeOnDeviceTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext

    @Test
    fun theProbeRunsOffTheMainThreadInUnder250Ms() = runBlocking {
        assumeTrue("StrictMode's listener needs Android 9", Build.VERSION.SDK_INT >= Build.VERSION_CODES.P)
        val violations = CopyOnWriteArrayList<Violation>()
        instrumentation.runOnMainSync {
            StrictMode.setThreadPolicy(
                StrictMode.ThreadPolicy.Builder().detectAll().penaltyListener(Runnable::run) { violations += it }.build(),
            )
        }
        val profile = DeviceProfile.of(context)
        var probedOn: Thread? = null
        val parts = linkedMapOf<String, Long>()
        fun <T> timed(name: String, block: () -> T): T {
            val start = SystemClock.elapsedRealtime()
            return block().also { parts[name] = SystemClock.elapsedRealtime() - start }
        }
        val display = AndroidDisplayInfo(context)
        val audio = AndroidAudioInfo(context)
        val probe = ProbingDeviceCapabilities(
            device = { ProbingDeviceCapabilities.deviceFacts(context, profile) },
            codecs = CodecInventory {
                probedOn = Thread.currentThread()
                timed("codecs") { AndroidCodecInventory().decoders() }
            },
            display = DisplayInfo { timed("display") { display.read() } },
            audio = AudioInfo { timed("audio") { audio.read() } },
            builtInQuirks = { timed("quirks") { QuirkTable.load(context.resources) } },
            describe = { timed("describe") { CapsDescription.lines(it, context.resources) } },
        )
        val started = SystemClock.elapsedRealtime()
        // Asked from the main thread, as the app does after its first frame.
        val snapshot = withContext(Dispatchers.Main) { probe.warmUp() }
        val took = SystemClock.elapsedRealtime() - started

        android.util.Log.i("pley.caps", "cold probe took $took ms: $parts")
        assertTrue("took $took ms: $parts", took < 250)
        assertTrue("probed on ${probedOn?.name}", probedOn != null && probedOn != Looper.getMainLooper().thread)
        assertEquals("StrictMode: $violations", emptyList<Violation>(), violations.toList())
        assertTrue(snapshot.report.videoCodecs.isNotEmpty())
        assertTrue(snapshot.lines.isNotEmpty())
        assertEquals(snapshot, probe.snapshot.value)
        probe.close()
    }

    @Test
    fun aSavedProbeIsReadBack() = runBlocking {
        val profile = DeviceProfile.of(context)
        val file = java.io.File(context.cacheDir, "caps-test/snapshot.json").also { it.delete() }
        fun probe(codecs: CodecInventory) = ProbingDeviceCapabilities(
            device = { ProbingDeviceCapabilities.deviceFacts(context, profile) },
            codecs = codecs,
            display = AndroidDisplayInfo(context),
            audio = AndroidAudioInfo(context),
            builtInQuirks = { QuirkTable.load(context.resources) },
            cache = CapsCache(file),
            describe = { CapsDescription.lines(it, context.resources) },
        )
        val first = probe(AndroidCodecInventory()).warmUp()
        var waited = 0
        while (!file.isFile && waited++ < 50) Thread.sleep(20)
        var asked = false
        val started = SystemClock.elapsedRealtime()
        val again = probe(CodecInventory { asked = true; emptyList() }).warmUp()
        val took = SystemClock.elapsedRealtime() - started
        android.util.Log.i("pley.caps", "saved probe read back in $took ms")
        assertEquals(first.report, again.report)
        assertTrue("the decoders came from the saved probe", !asked)
        assertTrue("took $took ms", took < 250)
    }
}
