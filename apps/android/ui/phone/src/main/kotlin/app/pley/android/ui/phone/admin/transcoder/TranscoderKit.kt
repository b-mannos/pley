package app.pley.android.ui.phone.admin.transcoder

import android.content.res.Resources
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.admin.HwAccel
import app.pley.android.model.admin.ToneMapping
import app.pley.android.screens.admin.transcoder.AccelChoice
import app.pley.android.screens.admin.transcoder.ChoiceReason
import app.pley.android.screens.admin.transcoder.FitNote
import app.pley.android.screens.admin.transcoder.PresetNote
import app.pley.android.screens.admin.transcoder.TonemapChoice
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.kit.PleySheet

/*
 * The rows of Settings → Transcoder, which the account kit's don't cover:
 * a value with a hint under it, a switch that can be greyed out, the line
 * under a choice that says why it is greyed out or what happens instead,
 * and a sheet whose choices can be greyed out; and the words of the
 * choices, as the web's transcodeChoices.ts says them.
 */

/**
 * A row that shows a value: the [title], its [value] (in [valueColor],
 * [mono] for a path) and a [hint] under it; opens something when there
 * is an [onClick], else only says. [trailing] takes the chevron's place.
 */
@Composable
internal fun DetailRow(
    title: String,
    modifier: Modifier = Modifier,
    value: String? = null,
    hint: String? = null,
    onClick: (() -> Unit)? = null,
    valueColor: Color = PleyColors.Muted,
    mono: Boolean = false,
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = PleyColors.Fg)
            if (!value.isNullOrEmpty()) {
                Text(
                    value,
                    style = MaterialTheme.typography.bodyMedium,
                    color = valueColor,
                    fontFamily = if (mono) FontFamily.Monospace else null,
                )
            }
            if (!hint.isNullOrEmpty()) {
                Text(hint, style = MaterialTheme.typography.bodySmall, color = PleyColors.Subtle, modifier = Modifier.padding(top = 2.dp))
            }
        }
        if (trailing != null) {
            Spacer(Modifier.width(12.dp))
            trailing()
        } else if (onClick != null) {
            Spacer(Modifier.width(12.dp))
            Icon(PleyIcons.ChevronRight, contentDescription = null, size = 20.dp, tint = PleyColors.Subtle)
        }
    }
}

/** A switch row with a [hint]; greyed out and left as it is while not [enabled] (the note under it says why). */
@Composable
internal fun ToggleRow(
    title: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    hint: String? = null,
    enabled: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = if (enabled) PleyColors.Fg else PleyColors.Disabled)
            if (hint != null) Text(hint, style = MaterialTheme.typography.bodySmall, color = PleyColors.Subtle, modifier = Modifier.padding(top = 2.dp))
        }
        Spacer(Modifier.width(12.dp))
        Switch(
            checked = checked,
            onCheckedChange = null,
            enabled = enabled,
            colors = SwitchDefaults.colors(
                checkedThumbColor = PleyColors.OnAccent,
                checkedTrackColor = PleyColors.Accent,
                uncheckedThumbColor = PleyColors.Control,
                uncheckedTrackColor = PleyColors.Surface3,
                uncheckedBorderColor = PleyColors.Control,
            ),
        )
    }
}

/**
 * Under a row: why its choice is greyed out here, or ([notice]) what
 * happens instead of a choice saved before that can't work here, as the
 * web's Note.
 */
@Composable
internal fun NoteLine(text: String, notice: Boolean = false) {
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp)) {
        Icon(
            if (notice) PleyIcons.CircleAlert else PleyIcons.Info,
            contentDescription = null,
            size = 14.dp,
            tint = if (notice) PleyColors.DangerText else PleyColors.Subtle,
            modifier = Modifier.padding(top = 2.dp),
        )
        Spacer(Modifier.width(6.dp))
        Text(text, style = MaterialTheme.typography.bodySmall, color = if (notice) PleyColors.Fg else PleyColors.Subtle)
    }
}

/** A choice of a [ReasonChoiceSheet]; greyed out with its [reason] under it when it can't work here. */
internal data class ReasonChoice<T>(val value: T, val label: String, val reason: String? = null)

/**
 * The account kit's choice sheet, whose choices that can't work here are
 * greyed out with why, as the web's menus grey them out. The one chosen
 * stays checked even then (it was saved before).
 */
@Composable
internal fun <T> ReasonChoiceSheet(
    title: String,
    choices: List<ReasonChoice<T>>,
    selected: T,
    onPick: (T) -> Unit,
    onDismiss: () -> Unit,
) {
    PleySheet(onDismiss = onDismiss, title = title) {
        val state = rememberLazyListState()
        LazyColumn(Modifier.fillMaxWidth().heightIn(max = 520.dp), state = state) {
            itemsIndexed(choices, key = { i, _ -> i }) { _, choice ->
                val on = choice.value == selected
                val enabled = choice.reason == null
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 56.dp)
                        .selectable(selected = on, enabled = enabled, role = Role.RadioButton) {
                            onPick(choice.value)
                            onDismiss()
                        }
                        .padding(horizontal = 24.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            choice.label,
                            style = MaterialTheme.typography.bodyLarge,
                            color = when {
                                on -> PleyColors.AccentBright
                                enabled -> PleyColors.Fg
                                else -> PleyColors.Disabled
                            },
                        )
                        if (choice.reason != null) {
                            Text(choice.reason, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Disabled)
                        }
                    }
                    Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                        if (on) Icon(PleyIcons.Check, contentDescription = null, size = 22.dp, tint = PleyColors.Accent)
                    }
                }
            }
        }
    }
}

/** The hardware accelerations' names; Automatic and software are said in the language shown. */
internal fun accelName(resources: Resources, accel: HwAccel?): String = when (accel) {
    null -> resources.getString(R.string.admin_transcoder_software_encoding)
    HwAccel.Auto -> resources.getString(R.string.admin_transcoder_automatic)
    HwAccel.None -> resources.getString(R.string.admin_transcoder_software_only)
    HwAccel.Nvenc -> "NVIDIA NVENC"
    HwAccel.Qsv -> "Intel Quick Sync"
    HwAccel.Amf -> "AMD AMF"
    HwAccel.Vaapi -> "VAAPI"
    HwAccel.Videotoolbox -> "VideoToolbox"
    HwAccel.Vulkan -> "Vulkan"
    HwAccel.Unknown -> accel.wire
}

/** A tone mapper's name by its wire name ("opencl" is OpenCL). */
internal fun tonemapperName(wire: String): String = when (wire) {
    ToneMapping.Opencl.wire -> "OpenCL"
    else -> wire
}

internal fun tonemapName(resources: Resources, mode: ToneMapping): String = when (mode) {
    ToneMapping.Auto -> resources.getString(R.string.admin_transcoder_automatic)
    ToneMapping.None -> resources.getString(R.string.admin_transcoder_disabled)
    else -> tonemapperName(mode.wire)
}

/** "Automatic · NVIDIA NVENC" once the transcoder says what Automatic picks; else the choice's name. */
internal fun accelLabel(resources: Resources, choice: AccelChoice): String = choice.automatic
    ?.let { resources.getString(R.string.admin_transcoder_automatic_with, accelName(resources, it.pick)) }
    ?: accelName(resources, choice.accel)

internal fun tonemapLabel(resources: Resources, choice: TonemapChoice): String = choice.automatic
    ?.let { auto ->
        val name = auto.pick?.let(::tonemapperName) ?: resources.getString(R.string.admin_transcoder_none_available)
        resources.getString(R.string.admin_transcoder_automatic_with, name)
    }
    ?: tonemapName(resources, choice.mode)

/** Why a choice can't work, as the web words it ("needs an NVIDIA graphics card"). */
internal fun reasonText(resources: Resources, reason: ChoiceReason): String = when (reason) {
    ChoiceReason.LinuxOnly -> resources.getString(R.string.admin_transcoder_linux_only)
    ChoiceReason.MacosOnly -> resources.getString(R.string.admin_transcoder_macos_only)
    ChoiceReason.NotOnMacos -> resources.getString(R.string.admin_transcoder_not_on_macos)
    is ChoiceReason.AccelNeeds -> resources.getString(
        when (reason.accel) {
            HwAccel.Qsv -> R.string.admin_transcoder_needs_qsv
            HwAccel.Amf -> R.string.admin_transcoder_needs_amf
            HwAccel.Vaapi -> R.string.admin_transcoder_needs_vaapi
            HwAccel.Videotoolbox -> R.string.admin_transcoder_needs_videotoolbox
            else -> R.string.admin_transcoder_needs_nvenc
        },
    )
    is ChoiceReason.TonemapNeeds -> resources.getString(
        when (reason.mode) {
            ToneMapping.Opencl -> R.string.admin_transcoder_needs_opencl
            ToneMapping.Zscale -> R.string.admin_transcoder_needs_zscale
            else -> R.string.admin_transcoder_needs_libplacebo
        },
    )
    is ChoiceReason.NotWith -> resources.getString(R.string.admin_transcoder_not_with, accelName(resources, reason.accel))
    ChoiceReason.NoGpuDecoder -> resources.getString(R.string.admin_transcoder_no_gpu_decoder)
    ChoiceReason.NewerTranscoder -> resources.getString(R.string.admin_transcoder_newer_transcoder)
    ChoiceReason.NotAvailableHere -> resources.getString(R.string.admin_transcoder_not_available_here)
    ChoiceReason.NeedsThrottling -> resources.getString(R.string.admin_transcoder_needs_throttling)
    is ChoiceReason.Said -> reason.text
}

/** [reasonText] starting a sentence. */
internal fun reasonSentence(resources: Resources, reason: ChoiceReason): String =
    reasonText(resources, reason).replaceFirstChar { it.uppercase() }

/** What changed with a hardware acceleration chosen: "Set to Automatic: Vulkan tone maps with libplacebo only." */
internal fun fitText(resources: Resources, note: FitNote): String = when (note) {
    is FitNote.LibplaceboOnly -> resources.getString(R.string.admin_transcoder_fit_libplacebo_only, accelName(resources, note.accel))
    is FitNote.Unreached -> resources.getString(
        R.string.admin_transcoder_fit_unreached,
        tonemapName(resources, note.tonemap),
        accelName(resources, note.accel),
    )
    is FitNote.Decoding -> resources.getString(R.string.admin_transcoder_fit_decoding, accelName(resources, note.accel))
}

internal fun presetText(resources: Resources, note: PresetNote): String {
    val name = accelName(resources, note.accel)
    return when (note) {
        is PresetNote.Hevc -> resources.getString(R.string.admin_transcoder_preset_hevc, name)
        is PresetNote.Hdr -> resources.getString(R.string.admin_transcoder_preset_hdr, name)
        is PresetNote.Software -> resources.getString(R.string.admin_transcoder_preset_software, name)
    }
}

/** A preset by its own name; Automatic is Pley's. */
internal fun presetLabel(resources: Resources, preset: String): String =
    if (preset == "auto") resources.getString(R.string.admin_transcoder_preset_auto) else preset
