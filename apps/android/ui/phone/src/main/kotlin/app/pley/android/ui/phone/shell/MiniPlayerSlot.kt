package app.pley.android.ui.phone.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.player.NowPlaying
import app.pley.android.screens.PleyKey
import app.pley.android.ui.phone.R

/**
 * Where the mini player sits (design spec §3.6): a 72 dp bar above the
 * navigation bar while music plays, opening Now Playing. A stub until the
 * music package fills it (the art, Play/Pause and Next, the progress
 * line); nothing shows while nothing plays.
 */
@Composable
internal fun MiniPlayerSlot(nowPlaying: NowPlaying?, modifier: Modifier = Modifier) {
    if (nowPlaying == null) return
    val navigator = LocalPhoneNavigator.current
    Row(
        modifier
            .fillMaxWidth()
            .height(72.dp)
            .background(PleyColors.Surface2)
            .clickable(role = Role.Button, onClickLabel = stringResource(R.string.shell_now_playing)) { navigator.open(PleyKey.NowPlaying) }
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(56.dp).background(PleyColors.Surface3, PleyShape.TilePhone), contentAlignment = Alignment.Center) {
            Icon(PleyIcons.Music, contentDescription = null, tint = PleyColors.Muted)
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(nowPlaying.title, style = MaterialTheme.typography.titleSmall, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
            nowPlaying.artist?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = PleyColors.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}
