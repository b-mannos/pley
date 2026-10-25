package app.pley.android.ui.phone.music

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.player.MusicControl
import app.pley.android.player.MusicPosition
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.music.AlbumRow
import app.pley.android.ui.phone.R
import kotlinx.coroutines.delay

/**
 * Where the track is, refreshed every [everyMs] while [playing] and once
 * otherwise. Read it while drawing (a progress line), or in a small
 * composable of its own (the times), so the rest of the screen doesn't
 * recompose with it.
 */
@Composable
internal fun rememberMusicPosition(position: () -> MusicPosition, playing: Boolean, key: Any?, everyMs: Long = 500): State<MusicPosition> {
    val state = remember { mutableStateOf(position()) }
    LaunchedEffect(playing, key) {
        state.value = position()
        while (playing) {
            delay(everyMs)
            state.value = position()
        }
    }
    return state
}

/** The app's music, for the mini player; `null` in tests whose Application holds no graph. */
@Composable
internal fun rememberMusicControl(): MusicControl? {
    val app = LocalContext.current.applicationContext
    return remember(app) { (app as? PleyGraphOwner)?.screensGraph?.player?.music }
}

/**
 * A line of an album's tracks (MusicViews.tsx TrackRow): the number, or
 * the playing glyph in [accent][PleyColors.Accent] for the track playing;
 * the title, its own artist when it isn't the album's, and the length.
 * The track playing reads in accent; the one the album was opened from is
 * tinted.
 */
@Composable
internal fun TrackLine(
    row: AlbumRow.Track,
    current: Boolean,
    playing: Boolean,
    highlighted: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val tint = when {
        current -> PleyColors.Fg.copy(alpha = 0.05f)
        highlighted -> PleyColors.Accent.copy(alpha = 0.12f)
        else -> androidx.compose.ui.graphics.Color.Transparent
    }
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clip(PleyShape.Field)
            .background(tint)
            .semantics { selected = current }
            .clickable(role = Role.Button, onClickLabel = null, onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(32.dp), contentAlignment = Alignment.CenterEnd) {
            if (current) {
                Icon(if (playing) PleyIcons.AudioLines else PleyIcons.Play, contentDescription = null, size = 18.dp, tint = PleyColors.Accent)
            } else {
                Text(row.number, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Subtle, textAlign = TextAlign.End)
            }
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                row.title,
                style = MaterialTheme.typography.titleMedium,
                color = if (current) PleyColors.AccentBright else PleyColors.Fg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (row.artist != null) {
                Text(row.artist!!, style = MaterialTheme.typography.bodySmall, color = PleyColors.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Spacer(Modifier.width(12.dp))
        Text(row.duration, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted)
    }
}

/** "Disc 2", over its tracks. */
@Composable
internal fun DiscHeader(number: Int, modifier: Modifier = Modifier) {
    Text(
        androidx.compose.ui.res.stringResource(R.string.music_disc, number),
        style = MaterialTheme.typography.labelLarge,
        color = PleyColors.Subtle,
        modifier = modifier.padding(start = 8.dp, top = 16.dp, bottom = 4.dp),
    )
}

/** A path from the server made absolute on [origin]. */
internal fun absolute(path: String, origin: String): String =
    if (path.startsWith("http://") || path.startsWith("https://")) path else origin.trimEnd('/') + "/" + path.trimStart('/')
