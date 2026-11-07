package app.pley.android.ui.phone.admin.transcoder

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.common.errorSentence
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.text.asString
import app.pley.android.designsystem.text.formatWords
import app.pley.android.model.admin.HwAccel
import app.pley.android.model.admin.TranscodeFolder
import app.pley.android.model.format.formatBytes
import app.pley.android.screens.admin.transcoder.FitNote
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.admin.transcoder.TranscodeChoices
import app.pley.android.screens.admin.transcoder.TranscoderAdminViewModel
import app.pley.android.screens.admin.transcoder.TranscoderPage
import app.pley.android.screens.admin.transcoder.TranscoderTest
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.account.SettingsRow
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.Entry
import app.pley.android.ui.phone.admin.TextEntryDialog
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.admin.folders.FolderPickerSheet
import app.pley.android.ui.phone.admin.numberIn
import app.pley.android.ui.phone.admin.numberProblem
import app.pley.android.ui.phone.kit.DelayedSpinner
import app.pley.android.ui.phone.shell.LocalPhoneShell

/** What the transcoder's page has open over it. */
private enum class TranscoderDialog { Accel, Preset, Tonemap, MaxTranscodes }

/**
 * Settings → Transcoder: the transcoder and how to test it, the folder it
 * works in (picked on the server), then how video is encoded, decoded and
 * tone mapped, and how many transcodes run and how. Each choice is saved
 * as it is made; what can't work here is greyed out with why, as the
 * server's FFmpeg says. [id] names nothing: the part is one page.
 */
@Composable
internal fun TranscoderScreen(@Suppress("UNUSED_PARAMETER") id: String?) {
    val vm: TranscoderAdminViewModel = viewModel(factory = TranscoderAdminViewModel.factory)
    val load by vm.page.state.collectAsStateWithLifecycle()
    val test by vm.test.collectAsStateWithLifecycle()
    val resources = LocalResources.current
    val shell = LocalPhoneShell.current
    var dialog by rememberSaveable { mutableStateOf<TranscoderDialog?>(null) }
    AdminEvents(vm) { done ->
        if (done.what == TranscoderAdminViewModel.FITTED) done.args.filterIsInstance<FitNote>().joinToString(" ") { fitText(resources, it) } else null
    }
    LaunchedEffect(vm) {
        vm.folderNotices.collect { done ->
            val saved = if (done.what == TranscoderAdminViewModel.FOLDER_NEXT_START) R.string.admin_transcoder_folder_saved_next_start else R.string.admin_transcoder_folder_saved
            shell.toasts.show(resources.getString(saved))
        }
    }

    AdminPage(title = stringResource(R.string.admin_transcoder_title), load = load, onRetry = vm.page::reload, tag = TRANSCODER_PAGE) { page ->
        item(key = "intro") {
            Text(
                stringResource(R.string.admin_transcoder_description),
                style = MaterialTheme.typography.bodyMedium,
                color = PleyColors.Muted,
                modifier = Modifier.adminWidth.padding(start = 4.dp),
            )
        }
        item(key = "transcoder") {
            TranscoderGroup(
                page = page,
                test = test,
                onTest = vm::testTranscoder,
                onChangeFolder = { vm.folders.open(it) },
                onDefaultFolder = { vm.chooseFolder(null) },
            )
        }
        item(key = "video") {
            VideoGroup(page, onOpen = { dialog = it }, onDecoding = vm::setHardwareDecoding)
        }
        item(key = "load") {
            LoadGroup(page, onMax = { dialog = TranscoderDialog.MaxTranscodes }, onThrottle = vm::setThrottle, onMemory = vm::setInMemory)
        }
    }

    FolderPickerSheet(vm.folders, onSelect = { vm.chooseFolder(it) })
    val page = load.valueOrNull ?: return
    val settings = page.settings
    when (dialog) {
        TranscoderDialog.Accel -> ReasonChoiceSheet(
            title = stringResource(R.string.admin_transcoder_accel),
            choices = TranscodeChoices.accelChoices(settings.hwaccel, page.system).map {
                ReasonChoice(it.accel, accelLabel(resources, it), it.reason?.let { why -> reasonText(resources, why) })
            },
            selected = settings.hwaccel,
            onPick = vm::chooseAccel,
            onDismiss = { dialog = null },
        )
        TranscoderDialog.Preset -> ReasonChoiceSheet(
            title = stringResource(R.string.admin_transcoder_preset),
            choices = TranscodeChoices.PRESETS.map { ReasonChoice(it, presetLabel(resources, it)) },
            selected = settings.transcodePreset,
            onPick = vm::setPreset,
            onDismiss = { dialog = null },
        )
        TranscoderDialog.Tonemap -> ReasonChoiceSheet(
            title = stringResource(R.string.admin_transcoder_tonemap),
            choices = TranscodeChoices.tonemapChoices(page.system, settings.hwaccel).map {
                ReasonChoice(it.mode, tonemapLabel(resources, it), it.reason?.let { why -> reasonText(resources, why) })
            },
            selected = settings.tonemapping,
            onPick = vm::setTonemapping,
            onDismiss = { dialog = null },
        )
        TranscoderDialog.MaxTranscodes -> TextEntryDialog(
            title = stringResource(R.string.admin_transcoder_max),
            initial = settings.maxTranscodes.toString(),
            entry = Entry.Number,
            check = numberProblem(TranscodeChoices.MAX_TRANSCODES),
            onSave = { typed ->
                numberIn(typed, TranscodeChoices.MAX_TRANSCODES)?.let { vm.setMaxTranscodes(it.toInt()) }
                dialog = null
            },
            onDismiss = { dialog = null },
        )
        null -> Unit
    }
}

/**
 * The transcoder: whether it runs (its version, FFmpeg's and the encoder
 * new transcodes use) and where, Test, and the folder it works in, which
 * a transcoder elsewhere keeps for itself.
 */
@Composable
private fun TranscoderGroup(
    page: TranscoderPage,
    test: TranscoderTest,
    onTest: () -> Unit,
    onChangeFolder: (String) -> Unit,
    onDefaultFolder: () -> Unit,
) {
    val tc = page.system?.transcoder
    val status = when {
        tc == null -> null
        tc.available -> stringResource(
            R.string.admin_transcoder_running,
            tc.version.orEmpty(),
            tc.ffmpegVersion ?: "?",
            tc.activeEncoder ?: stringResource(R.string.admin_transcoder_no_encoder),
        )
        tc.error != null -> stringResource(R.string.admin_transcoder_unavailable_because, tc.error.orEmpty())
        else -> stringResource(R.string.admin_transcoder_unavailable)
    }
    SettingsGroup(stringResource(R.string.admin_transcoder_title), Modifier.adminWidth) {
        DetailRow(
            stringResource(R.string.admin_transcoder_connection),
            value = if (tc?.managed == false) stringResource(R.string.admin_transcoder_external, tc.url ?: "?") else stringResource(R.string.admin_transcoder_managed),
            hint = status,
        )
        RowDivider(inset = false)
        val (result, color) = when (test) {
            is TranscoderTest.Done -> {
                val ms = stringResource(R.string.admin_transcoder_test_ms, test.result.ms)
                (if (test.result.ok) "${test.result.message} $ms" else test.result.message) to
                    (if (test.result.ok) PleyColors.Success else PleyColors.DangerText)
            }
            is TranscoderTest.Failed -> errorSentence(test.error).asString() to PleyColors.DangerText
            else -> null to PleyColors.Muted
        }
        DetailRow(
            stringResource(R.string.admin_transcoder_test),
            value = result,
            valueColor = color,
            onClick = onTest,
            trailing = if (test == TranscoderTest.Testing) ({ DelayedSpinner(stringResource(R.string.admin_transcoder_test), size = 20.dp) }) else null,
            modifier = Modifier.testTag(TRANSCODER_TEST),
        )
        val folder = page.system?.transcodeDir
        if (folder != null && tc?.managed != false) {
            RowDivider(inset = false)
            FolderRows(folder, onChangeFolder, onDefaultFolder)
        }
    }
}

/** Where the transcoder works now, what Pley's next start changes, and choosing another. */
@Composable
private fun FolderRows(folder: TranscodeFolder, onChange: (String) -> Unit, onDefault: () -> Unit) {
    DetailRow(
        stringResource(R.string.admin_transcoder_folder_label),
        value = folder.inUse,
        mono = true,
        hint = stringResource(R.string.admin_transcoder_folder_hint),
    )
    folder.problem?.let { NoteLine(stringResource(R.string.admin_transcoder_folder_problem, it.folder, it.reason), notice = true) }
    folder.next?.let { NoteLine(stringResource(R.string.admin_transcoder_folder_next, it)) }
    if (folder.fixed) {
        NoteLine(stringResource(R.string.admin_transcoder_folder_fixed))
        return
    }
    RowDivider()
    SettingsRow(
        stringResource(R.string.admin_transcoder_folder_change),
        icon = PleyIcons.FolderOpen,
        onClick = { onChange(folder.chosen) },
        modifier = Modifier.testTag(TRANSCODER_FOLDER),
    )
    if (folder.chosen != folder.default) {
        RowDivider()
        SettingsRow(stringResource(R.string.admin_transcoder_folder_use_default), icon = PleyIcons.RotateCcw, onClick = onDefault)
    }
}

/** How video is encoded, decoded and tone mapped, each with what goes and doesn't here. */
@Composable
private fun VideoGroup(page: TranscoderPage, onOpen: (TranscoderDialog) -> Unit, onDecoding: (Boolean) -> Unit) {
    val resources = LocalResources.current
    val settings = page.settings
    val system = page.system
    val tc = system?.transcoder
    val accel = settings.hwaccel
    SettingsGroup(stringResource(R.string.admin_transcoder_video_title), Modifier.adminWidth, footer = stringResource(R.string.admin_transcoder_video_description)) {
        val chosen = TranscodeChoices.accelChoices(accel, system).firstOrNull { it.accel == accel }
        DetailRow(
            stringResource(R.string.admin_transcoder_accel),
            value = chosen?.let { accelLabel(resources, it) } ?: accelName(resources, accel),
            hint = if (tc != null && tc.encoders.isNotEmpty()) {
                stringResource(R.string.admin_transcoder_detected, tc.encoders.joinToString(", "))
            } else {
                stringResource(R.string.admin_transcoder_accel_hint)
            },
            onClick = { onOpen(TranscoderDialog.Accel) },
            modifier = Modifier.testTag(TRANSCODER_ACCEL),
        )
        TranscodeChoices.accelReason(accel, system)?.let {
            NoteLine(stringResource(R.string.admin_transcoder_software_instead, reasonSentence(resources, it)), notice = true)
        }
        RowDivider(inset = false)

        val decodingProblem = TranscodeChoices.decodingReason(accel, system)
        // Vulkan decodes on the GPU: hardware decoding turns on with it, not off.
        val vulkan = accel == HwAccel.Vulkan
        ToggleRow(
            stringResource(R.string.admin_transcoder_decoding),
            checked = settings.hardwareDecoding,
            onChange = onDecoding,
            hint = stringResource(R.string.admin_transcoder_decoding_hint),
            enabled = decodingProblem == null && !(vulkan && settings.hardwareDecoding),
            modifier = Modifier.testTag(TRANSCODER_DECODING),
        )
        decodingProblem?.let { NoteLine(stringResource(R.string.admin_transcoder_decoded_on_cpu, reasonSentence(resources, it))) }
        if (vulkan) {
            if (settings.hardwareDecoding) {
                NoteLine(stringResource(R.string.admin_transcoder_vulkan_always_on))
            } else {
                NoteLine(stringResource(R.string.admin_transcoder_vulkan_needs_it), notice = true)
            }
        }
        RowDivider(inset = false)

        DetailRow(
            stringResource(R.string.admin_transcoder_preset),
            value = presetLabel(resources, settings.transcodePreset),
            hint = stringResource(R.string.admin_transcoder_preset_hint),
            onClick = { onOpen(TranscoderDialog.Preset) },
        )
        TranscodeChoices.presetNote(accel, system)?.let { NoteLine(presetText(resources, it)) }
        RowDivider(inset = false)

        val tonemaps = TranscodeChoices.tonemapChoices(system, accel)
        val tonemap = tonemaps.firstOrNull { it.mode == settings.tonemapping }
        val available = if (tc != null && tc.tonemapping.isNotEmpty()) {
            val list = tc.tonemapping.joinToString(", ") { tonemapperName(it) }
            stringResource(if (tc.dolbyVision) R.string.admin_transcoder_tonemap_available_dv else R.string.admin_transcoder_tonemap_available, list)
        } else {
            stringResource(R.string.admin_transcoder_tonemap_hint)
        }
        val dolbyVision = if (tc?.available == true && !tc.dolbyVision) {
            stringResource(R.string.admin_transcoder_dolby_vision_needs, builds(TranscodeChoices.transcoderOs(system)))
        } else {
            null
        }
        DetailRow(
            stringResource(R.string.admin_transcoder_tonemap),
            value = tonemap?.let { tonemapLabel(resources, it) } ?: tonemapName(resources, settings.tonemapping),
            hint = listOfNotNull(available, dolbyVision).joinToString("\n"),
            onClick = { onOpen(TranscoderDialog.Tonemap) },
        )
        val tonemapProblem = TranscodeChoices.tonemapReason(settings.tonemapping, system)
        // A combination saved before: the transcoder doesn't use Vulkan then.
        val clash = if (tonemapProblem == null) TranscodeChoices.tonemapAccelReason(settings.tonemapping, accel, system) else null
        when {
            tonemapProblem != null -> {
                val fallback = TranscodeChoices.tonemapFallback(system)
                    ?.let { stringResource(R.string.admin_transcoder_tonemapped_with, tonemapperName(it)) }
                    ?: stringResource(R.string.admin_transcoder_no_tonemapper)
                NoteLine(stringResource(R.string.admin_transcoder_tonemap_instead, reasonSentence(resources, tonemapProblem), fallback), notice = true)
            }
            clash != null -> NoteLine(
                stringResource(
                    if (vulkan) R.string.admin_transcoder_software_instead else R.string.admin_transcoder_tonemap_with_another,
                    reasonSentence(resources, clash),
                ),
                notice = true,
            )
        }
    }
}

/** How many transcodes run at once, whether they're held back, and whether their segments stay in memory. */
@Composable
private fun LoadGroup(page: TranscoderPage, onMax: () -> Unit, onThrottle: (Boolean) -> Unit, onMemory: (Boolean) -> Unit) {
    val resources = LocalResources.current
    val settings = page.settings
    val system = page.system
    val tc = system?.transcoder
    SettingsGroup(stringResource(R.string.admin_transcoder_load_title), Modifier.adminWidth, footer = stringResource(R.string.admin_transcoder_load_description)) {
        DetailRow(
            stringResource(R.string.admin_transcoder_max),
            value = settings.maxTranscodes.toString(),
            onClick = onMax,
            modifier = Modifier.testTag(TRANSCODER_MAX),
        )
        if (tc != null && tc.available && tc.maxJobs > 0 && settings.maxTranscodes > tc.maxJobs) {
            NoteLine(stringResource(if (tc.managed) R.string.admin_transcoder_max_jobs_managed else R.string.admin_transcoder_max_jobs_external, tc.maxJobs))
        }
        RowDivider(inset = false)

        val noThrottling = TranscodeChoices.known(system)?.throttling == false
        ToggleRow(
            stringResource(R.string.admin_transcoder_throttle),
            checked = settings.throttleTranscodes,
            onChange = onThrottle,
            hint = stringResource(R.string.admin_transcoder_throttle_hint),
            enabled = !noThrottling,
        )
        if (noThrottling) NoteLine(stringResource(R.string.admin_transcoder_no_throttling))
        RowDivider(inset = false)

        val memoryProblem = TranscodeChoices.memoryReason(system, settings.throttleTranscodes)
        val budget = tc?.memory?.takeIf { it.available }?.budgetBytes ?: 0
        ToggleRow(
            stringResource(R.string.admin_transcoder_memory),
            checked = settings.transcodeInMemory,
            onChange = onMemory,
            hint = if (budget > 0) {
                stringResource(R.string.admin_transcoder_memory_hint_up_to, formatBytes(budget, formatWords()))
            } else {
                stringResource(R.string.admin_transcoder_memory_hint)
            },
            enabled = memoryProblem == null,
        )
        memoryProblem?.let { NoteLine(stringResource(R.string.admin_transcoder_to_disk, reasonSentence(resources, it))) }
    }
}

/** FFmpeg builds with libplacebo for the transcoder's system (every one's, when it runs elsewhere). */
@Composable
private fun builds(os: String?): String = when (os) {
    "windows" -> stringResource(R.string.admin_transcoder_builds_windows)
    "linux" -> "jellyfin-ffmpeg"
    "macos" -> stringResource(R.string.admin_transcoder_builds_macos)
    else -> stringResource(R.string.admin_transcoder_builds_any)
}

internal const val TRANSCODER_PAGE = "pley.admin.transcoder"
internal const val TRANSCODER_TEST = "pley.admin.transcoder.test"
internal const val TRANSCODER_FOLDER = "pley.admin.transcoder.folder"
internal const val TRANSCODER_ACCEL = "pley.admin.transcoder.accel"
internal const val TRANSCODER_DECODING = "pley.admin.transcoder.decoding"
internal const val TRANSCODER_MAX = "pley.admin.transcoder.max"
