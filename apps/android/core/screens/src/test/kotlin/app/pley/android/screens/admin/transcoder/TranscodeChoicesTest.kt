package app.pley.android.screens.admin.transcoder

import app.pley.android.model.admin.HwAccel
import app.pley.android.model.admin.ToneMapping
import app.pley.android.model.admin.TranscoderAccelUse
import app.pley.android.model.admin.TranscoderMemory
import app.pley.android.model.admin.TranscoderSettings
import app.pley.android.model.admin.TranscoderState
import app.pley.android.model.admin.TranscoderSystem
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** What Settings → Transcoder offers, as the web's transcodeChoices.ts and the server's checks decide. */
class TranscodeChoicesTest {
    private fun system(
        os: String = "linux",
        accels: List<HwAccel>? = listOf(HwAccel.Vaapi),
        tonemapping: List<String> = listOf("opencl", "zscale"),
        withAccel: Map<String, TranscoderAccelUse> = emptyMap(),
        decodes: Boolean = true,
        throttling: Boolean = true,
        memory: TranscoderMemory? = TranscoderMemory(available = true, budgetBytes = 1024),
        managed: Boolean = true,
    ) = TranscoderSystem(
        os = os,
        transcoder = TranscoderState(
            available = true,
            managed = managed,
            encoders = listOf("h264_vaapi", "libx264"),
            accels = accels,
            hardwareDecoding = decodes,
            withAccel = withAccel,
            tonemapping = tonemapping,
            throttling = throttling,
            memory = memory,
        ),
    )

    @Test
    fun accelerationsThisSystemNeverHasAreLeftOutAndTheOthersSayWhatTheyNeed() {
        val choices = TranscodeChoices.accelChoices(HwAccel.Auto, system())
        assertEquals(
            listOf(HwAccel.Auto, HwAccel.Nvenc, HwAccel.Qsv, HwAccel.Amf, HwAccel.Vaapi, HwAccel.None),
            choices.map { it.accel },
        )
        assertEquals(AutoPick(HwAccel.Vaapi), choices.first().automatic)
        assertEquals(ChoiceReason.AccelNeeds(HwAccel.Nvenc), choices[1].reason)
        assertNull(choices.first { it.accel == HwAccel.Vaapi }.reason)
        // Saved before on another system, it stays offered, with why it can't work.
        val saved = TranscodeChoices.accelChoices(HwAccel.Videotoolbox, system())
        assertEquals(ChoiceReason.MacosOnly, saved.first { it.accel == HwAccel.Videotoolbox }.reason)
        // Vulkan only when saved, before Software only.
        assertEquals(HwAccel.Vulkan, TranscodeChoices.accelChoices(HwAccel.Vulkan, system()).let { it[it.size - 2].accel })
    }

    @Test
    fun nothingIsGreyedOutWhileNobodyKnows() {
        val unknown = TranscoderSystem(os = "windows", transcoder = TranscoderState(available = false))
        assertNull(TranscodeChoices.known(unknown))
        assertNull(TranscodeChoices.accelReason(HwAccel.Nvenc, unknown))
        assertNull(TranscodeChoices.tonemapReason(ToneMapping.Libplacebo, unknown))
        assertNull(TranscodeChoices.decodingReason(HwAccel.Auto, unknown))
        // The system says what it never has, even then.
        assertEquals(ChoiceReason.LinuxOnly, TranscodeChoices.accelReason(HwAccel.Vaapi, unknown))
        // An external transcoder's system isn't known: nothing is left out.
        assertEquals(7, TranscodeChoices.accelChoices(HwAccel.Auto, system(os = "windows", managed = false)).size)
        assertEquals(null, TranscodeChoices.accelChoices(HwAccel.Auto, null).first().automatic)
    }

    @Test
    fun toneMappersSayWhyTheyCantWorkHereOrWithTheAcceleration() {
        val vaapi = system(withAccel = mapOf("vaapi" to TranscoderAccelUse(tonemappers = listOf("zscale"))))
        val choices = TranscodeChoices.tonemapChoices(vaapi, HwAccel.Vaapi).associateBy { it.mode }
        assertEquals(AutoPick("opencl"), choices.getValue(ToneMapping.Auto).automatic)
        assertEquals(ChoiceReason.TonemapNeeds(ToneMapping.Libplacebo), choices.getValue(ToneMapping.Libplacebo).reason)
        assertEquals(ChoiceReason.NotWith(HwAccel.Vaapi), choices.getValue(ToneMapping.Opencl).reason)
        assertNull(choices.getValue(ToneMapping.Zscale).reason)
        assertEquals(ChoiceReason.NotWith(HwAccel.Vulkan), TranscodeChoices.tonemapAccelReason(ToneMapping.Zscale, HwAccel.Vulkan, null))
        assertEquals("opencl", TranscodeChoices.tonemapFallback(vaapi))
    }

    @Test
    fun hardwareDecodingSaysWhenItChangesNothing() {
        assertEquals(ChoiceReason.NoGpuDecoder, TranscodeChoices.decodingReason(HwAccel.Auto, system(decodes = false)))
        val noneDecodes = system(withAccel = mapOf("none" to TranscoderAccelUse(hardwareDecoding = false)))
        assertEquals(ChoiceReason.NotWith(null), TranscodeChoices.decodingReason(HwAccel.None, noneDecodes))
        assertNull(TranscodeChoices.decodingReason(HwAccel.Vaapi, noneDecodes))
        assertNull(TranscodeChoices.decodingReason(HwAccel.Vulkan, system(decodes = false)))
    }

    @Test
    fun choosingAnAccelerationFitsWhatGoesWithIt() {
        val settings = TranscoderSettings(tonemapping = ToneMapping.Opencl, hardwareDecoding = false)
        val vulkan = TranscodeChoices.fitToAccel(settings, HwAccel.Vulkan, system())
        assertEquals(
            AccelFit(ToneMapping.Auto, true, listOf(FitNote.LibplaceboOnly(HwAccel.Vulkan), FitNote.Decoding(HwAccel.Vulkan))),
            vulkan,
        )
        val vaapi = system(withAccel = mapOf("vaapi" to TranscoderAccelUse(tonemappers = listOf("zscale"))))
        assertEquals(
            AccelFit(ToneMapping.Auto, null, listOf(FitNote.Unreached(ToneMapping.Opencl, HwAccel.Vaapi))),
            TranscodeChoices.fitToAccel(settings, HwAccel.Vaapi, vaapi),
        )
        assertEquals(AccelFit(), TranscodeChoices.fitToAccel(settings, HwAccel.None, system()))
    }

    @Test
    fun thePresetSaysWhatItAppliesToWhileAGpuEncodes() {
        assertNull(TranscodeChoices.presetNote(HwAccel.None, system()))
        assertEquals(PresetNote.Software(HwAccel.Vaapi), TranscodeChoices.presetNote(HwAccel.Auto, system()))
        val hevc = system(withAccel = mapOf("vaapi" to TranscoderAccelUse(software = listOf("hevc"))))
        assertEquals(PresetNote.Hevc(HwAccel.Vaapi), TranscodeChoices.presetNote(HwAccel.Vaapi, hevc))
        // A GPU the transcoder lacks encodes nothing: software does.
        assertNull(TranscodeChoices.presetNote(HwAccel.Nvenc, system()))
    }

    @Test
    fun memoryNeedsATranscoderThatCanAndThrottling() {
        assertNull(TranscodeChoices.memoryReason(system(), throttle = true))
        assertEquals(ChoiceReason.NeedsThrottling, TranscodeChoices.memoryReason(system(), throttle = false))
        assertEquals(ChoiceReason.NeedsThrottling, TranscodeChoices.memoryReason(system(throttling = false), throttle = true))
        assertEquals(ChoiceReason.NewerTranscoder, TranscodeChoices.memoryReason(system(memory = null), throttle = true))
        assertEquals(
            ChoiceReason.Said("works on Linux only"),
            TranscodeChoices.memoryReason(system(memory = TranscoderMemory(available = false, reason = "works on Linux only")), throttle = true),
        )
        assertEquals(ChoiceReason.NotAvailableHere, TranscodeChoices.memoryReason(system(memory = TranscoderMemory(available = false)), throttle = true))
    }
}
