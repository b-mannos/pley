package app.pley.android.ui.phone.downloads

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import app.pley.android.data.DownloadEntry
import app.pley.android.data.DownloadState
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.ui.phone.R

/**
 * A download's state as one glyph, [size] square: the arrow when there is
 * none, a ring filling as it comes (the arrow inside, faint while it
 * waits), a tick in accent when it is whole, a warning when it failed.
 */
@Composable
internal fun DownloadGlyph(entry: DownloadEntry?, size: Dp = 24.dp) {
    when (entry?.state) {
        null -> Icon(PleyIcons.Download, contentDescription = null, size = size)
        DownloadState.Done -> Icon(PleyIcons.CircleCheck, contentDescription = null, size = size, tint = PleyColors.AccentBright)
        DownloadState.Failed -> Icon(PleyIcons.CircleAlert, contentDescription = null, size = size, tint = PleyColors.DangerText)
        DownloadState.Queued, DownloadState.Waiting, DownloadState.Downloading -> {
            // While the server prepares it, the ring is how far the server has come.
            val fraction = entry.converting ?: if (entry.state == DownloadState.Downloading) entry.fraction else 0f
            Box(
                Modifier
                    .size(size)
                    .drawBehind {
                        val stroke = 2.dp.toPx()
                        val inset = stroke / 2f
                        val arc = Size(this.size.width - stroke, this.size.height - stroke)
                        drawArc(PleyColors.Fg.copy(alpha = 0.25f), 0f, 360f, false, Offset(inset, inset), arc, style = Stroke(stroke))
                        if (fraction > 0f) {
                            drawArc(PleyColors.Accent, -90f, 360f * fraction, false, Offset(inset, inset), arc, style = Stroke(stroke, cap = StrokeCap.Round))
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    PleyIcons.Download,
                    contentDescription = null,
                    size = size * 0.55f,
                    tint = if (entry.state == DownloadState.Downloading) PleyColors.Fg else PleyColors.Muted,
                )
            }
        }
    }
}

/** What a download's button says to screen readers about [title]. */
@Composable
internal fun downloadDescription(entry: DownloadEntry?, title: String): String = when (entry?.state) {
    null -> stringResource(R.string.downloads_download_item, title)
    DownloadState.Done -> stringResource(R.string.downloads_downloaded_item, title)
    DownloadState.Failed -> stringResource(R.string.downloads_failed_toast, entry.error.orEmpty())
    else -> entry.converting?.let { stringResource(R.string.downloads_preparing_item, title, (it * 100).toInt()) }
        ?: stringResource(R.string.downloads_downloading_item, title, (entry.fraction * 100).toInt())
}

/**
 * An episode row's download button: 48 dp around the glyph. Tapping
 * downloads, retries a failed one, or opens [onMore] for one there is
 * (cancel or delete it, play it).
 */
@Composable
internal fun DownloadIconButton(
    entry: DownloadEntry?,
    title: String,
    onDownload: () -> Unit,
    onRetry: () -> Unit,
    onMore: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val description = downloadDescription(entry, title)
    Box(
        modifier
            .size(48.dp)
            .clip(PleyShape.Circle)
            .clickable(role = Role.Button) {
                when (entry?.state) {
                    null -> onDownload()
                    DownloadState.Failed -> onRetry()
                    else -> onMore()
                }
            }
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        DownloadGlyph(entry)
    }
}

/**
 * The item page's Download action (design spec §3.3's action column): the
 * glyph over "Download", "45 %", "Waiting", "Downloaded" or "Try again".
 */
@Composable
internal fun DownloadAction(
    entry: DownloadEntry?,
    title: String,
    onDownload: () -> Unit,
    onRetry: () -> Unit,
    onMore: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val label = when (entry?.state) {
        null -> stringResource(R.string.downloads_download)
        DownloadState.Done -> stringResource(R.string.downloads_downloaded)
        DownloadState.Failed -> stringResource(R.string.downloads_try_again)
        DownloadState.Downloading -> "${(entry.fraction * 100).toInt()}%"
        DownloadState.Queued -> entry.converting?.let { "${(it * 100).toInt()}%" } ?: stringResource(R.string.downloads_queued_short)
        DownloadState.Waiting -> stringResource(R.string.downloads_waiting_short)
    }
    val description = downloadDescription(entry, title)
    Column(
        modifier
            .width(72.dp)
            .defaultMinSize(minHeight = 56.dp)
            .clickable(role = Role.Button) {
                when (entry?.state) {
                    null -> onDownload()
                    DownloadState.Failed -> onRetry()
                    else -> onMore()
                }
            }
            .semantics(mergeDescendants = true) { contentDescription = description },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) { DownloadGlyph(entry) }
        Spacer(Modifier.height(2.dp))
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = PleyColors.Fg,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/**
 * Runs [download] once the app may show the downloads' notification
 * (asked once, on Android 13 and later): downloading goes on either way,
 * only quieter when it may not.
 */
@Composable
internal fun rememberDownloadStarter(download: (List<Long>) -> Unit): (List<Long>) -> Unit {
    val context = LocalContext.current
    val current = rememberUpdatedState(download)
    val pending = remember { arrayOfNulls<List<Long>>(1) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        pending[0]?.let { ids -> current.value(ids) }
        pending[0] = null
    }
    return remember(context, launcher) {
        { ids ->
            val asked = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            if (asked) {
                pending[0] = ids
                launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            } else {
                current.value(ids)
            }
        }
    }
}

/**
 * What can be done with a download there is, under its button: play it
 * (when whole) and delete it, or cancel it while it comes.
 */
@Composable
internal fun DownloadMenu(
    entry: DownloadEntry?,
    expanded: Boolean,
    onDismiss: () -> Unit,
    onPlay: (DownloadEntry) -> Unit,
    onRemove: (DownloadEntry) -> Unit,
) {
    if (entry == null) return
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss, containerColor = PleyColors.Surface3) {
        if (entry.state == DownloadState.Done) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.downloads_play)) },
                leadingIcon = { Icon(PleyIcons.Play, contentDescription = null, size = 20.dp) },
                onClick = {
                    onDismiss()
                    onPlay(entry)
                },
            )
        }
        DropdownMenuItem(
            text = {
                Text(stringResource(if (entry.state == DownloadState.Done) R.string.downloads_delete else R.string.downloads_cancel))
            },
            leadingIcon = { Icon(if (entry.state == DownloadState.Done) PleyIcons.Trash2 else PleyIcons.X, contentDescription = null, size = 20.dp) },
            onClick = {
                onDismiss()
                onRemove(entry)
            },
        )
    }
}
