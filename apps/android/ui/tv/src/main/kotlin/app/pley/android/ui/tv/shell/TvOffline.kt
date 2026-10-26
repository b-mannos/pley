package app.pley.android.ui.tv.shell

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.data.Loadable
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.network.OfflineException
import app.pley.android.ui.tv.R

/**
 * Whether what the TV shows came from what was saved, the server out of
 * reach: the phone's offline banner (design spec §3.4). Screens [report]
 * their loads; one the server couldn't answer raises it, a fresh one
 * lowers it. Its events bring the screens back on their own once the
 * server answers again.
 */
@Stable
internal class TvOfflineState {
    var offline by mutableStateOf(false)
        private set

    fun report(load: Loadable<*>) {
        when (load) {
            is Loadable.Ready -> if (load.fresh) offline = false
            is Loadable.Failed -> if (load.error is OfflineException) offline = true
            Loadable.Loading -> Unit
        }
    }

    /** A screen's state as fresh or not, and the error that kept it from the server's answer. */
    fun report(fresh: Boolean, error: Throwable?) = report(if (error != null) Loadable.Failed(error, Unit) else Loadable.Ready(Unit, fresh))
}

/** The shell's; outside it (a screen on its own, in tests) one nobody shows. */
internal val LocalTvOffline = staticCompositionLocalOf { TvOfflineState() }

/** Reports [load] to the shell's [TvOfflineState] whenever it changes. */
@Composable
internal fun ReportOffline(load: Loadable<*>?) {
    val offline = LocalTvOffline.current
    LaunchedEffect(load, offline) { load?.let(offline::report) }
}

/**
 * "Showing what pley had saved. Reconnecting…" at the top right while
 * [state] holds: the server-off icon and the line on surface3. Never
 * focusable, as there is nothing to do but wait; announced when it shows.
 */
@Composable
internal fun BoxScope.TvOfflineNotice(state: TvOfflineState) {
    AnimatedVisibility(
        visible = state.offline,
        modifier = Modifier.align(Alignment.TopEnd).padding(top = TvSafeArea.Vertical, end = TvSafeArea.Horizontal),
        enter = fadeIn(PleyMotion.reveal()),
        exit = fadeOut(PleyMotion.reveal()),
    ) {
        Row(
            Modifier
                .background(PleyColors.Surface3, PleyShape.Pill)
                .padding(horizontal = 20.dp, vertical = 10.dp)
                .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite }
                .testTag(TAG_OFFLINE),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(PleyIcons.ServerOff, contentDescription = null, size = 20.dp, tint = PleyColors.Muted)
            BasicText(stringResource(R.string.tv_shell_offline), style = PleyType.Tv.Label.copy(color = PleyColors.Fg))
        }
    }
}

/** How far what sits at the top right (toasts) moves down while the notice shows. */
internal val OfflineNoticeStep: Dp = 56.dp

/** How far [state] moves what sits at the top right down. */
internal fun offlineStep(state: TvOfflineState): Dp = if (state.offline) OfflineNoticeStep else 0.dp

internal const val TAG_OFFLINE = "tv.offline"
