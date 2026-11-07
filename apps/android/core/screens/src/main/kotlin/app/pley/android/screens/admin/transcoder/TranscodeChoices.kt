package app.pley.android.screens.admin.transcoder

import app.pley.android.model.admin.HwAccel
import app.pley.android.model.admin.ToneMapping
import app.pley.android.model.admin.TranscoderSettings
import app.pley.android.model.admin.TranscoderState
import app.pley.android.model.admin.TranscoderSystem

/** Why a choice can't work on this server, or with the hardware chosen; the interface words it. */
sealed interface ChoiceReason {
    data object LinuxOnly : ChoiceReason

    data object MacosOnly : ChoiceReason

    data object NotOnMacos : ChoiceReason

    /** [accel] needs a GPU the transcoder found no working encoder for. */
    data class AccelNeeds(val accel: HwAccel) : ChoiceReason

    /** The transcoder's FFmpeg has no [mode]. */
    data class TonemapNeeds(val mode: ToneMapping) : ChoiceReason

    /** Not with [accel]; `null`: not with software encoding. */
    data class NotWith(val accel: HwAccel?) : ChoiceReason

    data object NoGpuDecoder : ChoiceReason

    data object NewerTranscoder : ChoiceReason

    data object NotAvailableHere : ChoiceReason

    data object NeedsThrottling : ChoiceReason

    /** The server's own words ("works on Linux only"). */
    data class Said(val text: String) : ChoiceReason
}

/**
 * A hardware acceleration to offer, with why it can't work here ([reason]).
 * Automatic's [automatic] is what it picks here, once the transcoder says:
 * a GPU, or software encoding (`null` in it).
 */
data class AccelChoice(val accel: HwAccel, val reason: ChoiceReason?, val automatic: AutoPick<HwAccel>? = null)

/** A tone mapper to offer, as [AccelChoice]; Automatic's pick is a tone mapper's name. */
data class TonemapChoice(val mode: ToneMapping, val reason: ChoiceReason?, val automatic: AutoPick<String>? = null)

/** What Automatic picks on this server; `null`: software encoding, or no tone mapper. */
data class AutoPick<T>(val pick: T?)

/** What changed with a hardware acceleration chosen, and why. */
sealed interface FitNote {
    /** Tone mapping set to Automatic: [accel] tone maps with libplacebo only. */
    data class LibplaceboOnly(val accel: HwAccel) : FitNote

    /** Tone mapping set to Automatic: [tonemap]'s frames don't reach [accel]'s encoder here. */
    data class Unreached(val tonemap: ToneMapping, val accel: HwAccel) : FitNote

    /** Hardware decoding turned on: [accel] decodes on the GPU. */
    data class Decoding(val accel: HwAccel) : FitNote
}

/** What follows from choosing a hardware acceleration: the settings that change with it, and the notes saying so. */
data class AccelFit(
    val tonemapping: ToneMapping? = null,
    val hardwareDecoding: Boolean? = null,
    val notes: List<FitNote> = emptyList(),
)

/** What the software encoder preset applies to while [accel] encodes on a GPU. */
sealed interface PresetNote {
    val accel: HwAccel

    /** [accel] encodes H.264: the preset is for HEVC, and when software takes over. */
    data class Hevc(override val accel: HwAccel) : PresetNote

    /** [accel] encodes, but not HDR kept in 10-bit HEVC. */
    data class Hdr(override val accel: HwAccel) : PresetNote

    /** [accel] encodes everything: the preset is for when software takes over. */
    data class Software(override val accel: HwAccel) : PresetNote
}

/**
 * What Settings → Transcoder offers on this server, as the web's
 * transcodeChoices.ts, and as the server judges when saving (settings.rs,
 * `hwaccel_problem`, `tonemap_problem` and `fit_to_accel`): each choice
 * with why it can't work here, or with the hardware acceleration chosen. A
 * choice saved before (another GPU, another FFmpeg) stays chosen and says
 * what happens instead.
 */
object TranscodeChoices {
    /** The menu's order; Vulkan, and what this app doesn't know, only when saved. */
    private val ACCELS = listOf(HwAccel.Auto, HwAccel.Nvenc, HwAccel.Qsv, HwAccel.Amf, HwAccel.Vaapi, HwAccel.Videotoolbox, HwAccel.None)

    /** The accelerations that need a GPU or an FFmpeg of their own. */
    private val NEEDS_HARDWARE = setOf(HwAccel.Nvenc, HwAccel.Qsv, HwAccel.Amf, HwAccel.Vaapi, HwAccel.Videotoolbox)

    val TONEMAPS = listOf(ToneMapping.Auto, ToneMapping.Libplacebo, ToneMapping.Opencl, ToneMapping.Zscale, ToneMapping.None)

    /** x264's and x265's presets, by their own names; "auto" is Pley's. */
    val PRESETS = listOf("auto", "ultrafast", "superfast", "veryfast", "faster", "fast", "medium")

    /** The most transcodes at once the server takes. */
    val MAX_TRANSCODES = 1L..64L

    /** The transcoder, when it runs and its FFmpeg says what it can do; `null` while nobody knows. */
    fun known(system: TranscoderSystem?): TranscoderState? {
        val tc = system?.transcoder ?: return null
        // An FFmpeg that encodes nothing isn't there: what the GPU could do is unknown.
        return tc.takeIf { it.available && it.accels != null && it.encoders.isNotEmpty() }
    }

    /** The transcoder's operating system: Pley's when Pley starts it, unknown when it runs elsewhere. */
    fun transcoderOs(system: TranscoderSystem?): String? =
        system?.os?.takeIf { it.isNotEmpty() && system.transcoder.managed }

    /** Why [accel] can never work on [os], or `null`. */
    private fun osReason(accel: HwAccel, os: String?): ChoiceReason? = when {
        os == null -> null
        accel == HwAccel.Vaapi && os != "linux" -> ChoiceReason.LinuxOnly
        accel == HwAccel.Videotoolbox && os != "macos" -> ChoiceReason.MacosOnly
        accel in setOf(HwAccel.Nvenc, HwAccel.Qsv, HwAccel.Amf) && os == "macos" -> ChoiceReason.NotOnMacos
        else -> null
    }

    /** Why [accel] can't work on this server, or `null` (as far as anyone knows). */
    fun accelReason(accel: HwAccel, system: TranscoderSystem?): ChoiceReason? {
        if (accel == HwAccel.Auto || accel == HwAccel.None || accel == HwAccel.Vulkan || accel == HwAccel.Unknown) return null
        osReason(accel, transcoderOs(system))?.let { return it }
        val tc = known(system) ?: return null
        return if (accel !in tc.accels.orEmpty() && accel in NEEDS_HARDWARE) ChoiceReason.AccelNeeds(accel) else null
    }

    /**
     * The hardware accelerations to offer: those this system never has are
     * left out (VideoToolbox on Windows), unless [saved]; those it lacks the
     * GPU or FFmpeg for are offered with the reason.
     */
    fun accelChoices(saved: HwAccel, system: TranscoderSystem?): List<AccelChoice> {
        val tc = known(system)
        val os = transcoderOs(system)
        val values = if (saved in ACCELS) ACCELS else ACCELS.dropLast(1) + saved + HwAccel.None
        return values
            .filter { it == saved || osReason(it, os) == null }
            .map { accel ->
                val automatic = if (accel == HwAccel.Auto && tc != null) AutoPick(tc.accels.orEmpty().firstOrNull()) else null
                AccelChoice(accel, accelReason(accel, system), automatic)
            }
    }

    /** The hardware acceleration transcodes get with [accel] chosen: `null` for software, or while unknown. */
    fun accelInUse(accel: HwAccel, system: TranscoderSystem?): HwAccel? {
        val tc = known(system) ?: return null
        return when {
            accel == HwAccel.None -> null
            accel == HwAccel.Auto -> tc.accels.orEmpty().firstOrNull()
            accelReason(accel, system) != null -> null
            else -> accel
        }
    }

    /** The tone mappers to offer with [accel] chosen, each with why it can't work here or with it. */
    fun tonemapChoices(system: TranscoderSystem?, accel: HwAccel): List<TonemapChoice> {
        val tc = known(system)
        return TONEMAPS.map { mode ->
            val reason = tonemapReason(mode, system) ?: tonemapAccelReason(mode, accel, system)
            val automatic = if (mode == ToneMapping.Auto && tc != null) AutoPick(tc.tonemapping.firstOrNull()) else null
            TonemapChoice(mode, reason, automatic)
        }
    }

    /** Why the tone mapper [mode] can't work here, or `null`. */
    fun tonemapReason(mode: ToneMapping, system: TranscoderSystem?): ChoiceReason? {
        val tc = known(system) ?: return null
        if (mode == ToneMapping.Auto || mode == ToneMapping.None || mode == ToneMapping.Unknown) return null
        return if (mode.wire in tc.tonemapping) null else ChoiceReason.TonemapNeeds(mode)
    }

    /**
     * Whether the tone mapper [mode] goes with [accel]: Vulkan encodes on the
     * GPU only what libplacebo tone maps there (the transcoder's Vulkan
     * pipeline), and the rest in software. The others tone map before
     * their encoder, with any tone mapper.
     */
    fun tonemapGoesWith(accel: HwAccel, mode: ToneMapping): Boolean =
        accel != HwAccel.Vulkan || mode == ToneMapping.Auto || mode == ToneMapping.Libplacebo

    /**
     * Why the tone mapper [mode] doesn't go with [accel], or `null`:
     * Vulkan's rule, and the tone mappers whose frames didn't reach a VAAPI
     * encoder in the transcoder's test (it tone maps with another then).
     */
    fun tonemapAccelReason(mode: ToneMapping, accel: HwAccel, system: TranscoderSystem?): ChoiceReason? {
        if (!tonemapGoesWith(accel, mode)) return ChoiceReason.NotWith(accel)
        val works = known(system)?.with(accel)?.tonemappers ?: return null
        if (mode == ToneMapping.Auto || mode == ToneMapping.None || mode.wire in works) return null
        return ChoiceReason.NotWith(accel)
    }

    /**
     * Why hardware decoding changes nothing with [accel] chosen, or `null`:
     * as the transcoder says each choice decodes here (a software encoder
     * still gets CUDA's decoder on an NVIDIA computer). Vulkan decodes on
     * the GPU, so hardware decoding is on with it.
     */
    fun decodingReason(accel: HwAccel, system: TranscoderSystem?): ChoiceReason? {
        val tc = known(system) ?: return null
        if (accel == HwAccel.Vulkan) return null
        if (!tc.hardwareDecoding) return ChoiceReason.NoGpuDecoder
        if (tc.with(accel)?.hardwareDecoding != false) return null
        return ChoiceReason.NotWith(accelInUse(accel, system))
    }

    /**
     * What [settings] change to with [accel] chosen, as the server does when
     * it saves (`fit_to_accel`), and the web before it: a tone mapper that
     * doesn't go with it gives way to Automatic, and hardware decoding is
     * turned on for Vulkan.
     */
    fun fitToAccel(settings: TranscoderSettings, accel: HwAccel, system: TranscoderSystem?): AccelFit {
        var tonemapping: ToneMapping? = null
        var decoding: Boolean? = null
        val notes = mutableListOf<FitNote>()
        if (!tonemapGoesWith(accel, settings.tonemapping)) {
            tonemapping = ToneMapping.Auto
            notes += FitNote.LibplaceboOnly(accel)
        } else if (tonemapAccelReason(settings.tonemapping, accel, system) != null) {
            tonemapping = ToneMapping.Auto
            notes += FitNote.Unreached(settings.tonemapping, accel)
        }
        if (accel == HwAccel.Vulkan && !settings.hardwareDecoding) {
            decoding = true
            notes += FitNote.Decoding(accel)
        }
        return AccelFit(tonemapping, decoding, notes)
    }

    /**
     * What the software encoder preset applies to with [accel] chosen, when
     * a GPU encodes: what the transcoder leaves to software with it (HEVC,
     * or HDR kept in 10-bit HEVC), and the transcodes software takes over.
     * `null` with software encoding, where it applies to everything.
     */
    fun presetNote(accel: HwAccel, system: TranscoderSystem?): PresetNote? {
        val encodes = accelInUse(accel, system) ?: return null
        val software = known(system)?.with(accel)?.software.orEmpty()
        return when {
            "hevc" in software -> PresetNote.Hevc(encodes)
            "hdr" in software -> PresetNote.Hdr(encodes)
            else -> PresetNote.Software(encodes)
        }
    }

    /**
     * Why transcodes' segments can't be kept in memory here, or `null`: as
     * the transcoder says (nobody knows while it isn't running), and only
     * throttled transcodes are, since their lead on the player is bounded.
     */
    fun memoryReason(system: TranscoderSystem?, throttle: Boolean): ChoiceReason? {
        val tc = system?.transcoder
        if (tc?.available == true) {
            val memory = tc.memory ?: return ChoiceReason.NewerTranscoder
            if (!memory.available) return memory.reason?.let(ChoiceReason::Said) ?: ChoiceReason.NotAvailableHere
        }
        return if (!throttle || known(system)?.throttling == false) ChoiceReason.NeedsThrottling else null
    }

    /** The tone mapper Automatic uses here, by its name, instead of a saved one that can't work: `null` for none. */
    fun tonemapFallback(system: TranscoderSystem?): String? = known(system)?.tonemapping?.firstOrNull()
}
