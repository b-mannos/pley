@file:OptIn(ExperimentalMaterial3Api::class)

package app.pley.android.ui.phone.home

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.data.Loadable
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.designsystem.tile.rememberSkeleton
import app.pley.android.designsystem.tile.skeleton
import app.pley.android.model.LibraryKind
import app.pley.android.screens.PleyKey
import app.pley.android.screens.home.LibrariesState
import app.pley.android.screens.home.LibrariesViewModel
import app.pley.android.screens.home.LibraryEntry
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.kit.EmptyState
import app.pley.android.ui.phone.kit.ErrorState
import app.pley.android.ui.phone.kit.PleyTopAppBar
import app.pley.android.ui.phone.shell.LocalPhoneNavigator
import app.pley.android.ui.phone.shell.LocalPhoneShell
import app.pley.android.ui.phone.theme.LocalPhoneMetrics
import kotlinx.coroutines.delay

/**
 * The Libraries tab: every library this profile sees, in the server's
 * order, each with its kind's icon, its name, how much it holds and a
 * spinner while the server scans it. A library opens its page. Pull to
 * refresh asks the server again.
 */
@Composable
internal fun PhoneLibrariesScreen(vm: LibrariesViewModel = viewModel(factory = LibrariesViewModel.Factory)) {
    val state by vm.state.collectAsStateWithLifecycle()
    val navigator = LocalPhoneNavigator.current
    val shell = LocalPhoneShell.current
    val list = rememberLazyListState()
    LaunchedEffect(state) {
        // The offline banner: up while the list is what was saved, down once the server answers.
        when (val s = state) {
            is LibrariesState.Ready -> shell.report(s.error?.let { Loadable.Failed(it, Unit) } ?: Loadable.Ready(Unit, s.fresh))
            is LibrariesState.Failed -> shell.report(Loadable.Failed<Unit>(s.error))
            LibrariesState.Loading -> Unit
        }
    }
    Column(Modifier.fillMaxSize()) {
        PleyTopAppBar(title = stringResource(R.string.home_libraries_title), scrolled = { list.canScrollBackward })
        PullToRefreshBox(
            isRefreshing = (state as? LibrariesState.Ready)?.refreshing == true,
            onRefresh = vm::refresh,
            modifier = Modifier.weight(1f).fillMaxWidth(),
        ) {
            when (val s = state) {
                LibrariesState.Loading -> LibrariesSkeleton()
                is LibrariesState.Failed -> Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                    ErrorState(stringResource(R.string.home_libraries_failed), s.error, onRetry = vm::refresh)
                }
                is LibrariesState.Ready -> if (s.libraries.isEmpty()) {
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        EmptyState(
                            stringResource(R.string.home_no_libraries),
                            stringResource(R.string.home_no_libraries_body),
                            icon = PleyIcons.FolderPlus,
                        )
                    }
                } else {
                    LazyColumn(state = list, modifier = Modifier.fillMaxSize(), contentPadding = PaddingValues(vertical = 8.dp)) {
                        items(s.libraries, key = { it.library.id }, contentType = { "library" }) { entry ->
                            LibraryRow(entry, onClick = { navigator.open(PleyKey.Library(entry.library.id)) })
                        }
                    }
                }
            }
        }
    }
}

/** One library: a 40 dp tile with its kind's icon, the name and count, a spinner while it is scanned, else a chevron. */
@Composable
private fun LibraryRow(entry: LibraryEntry, onClick: () -> Unit) {
    val metrics = LocalPhoneMetrics.current
    val scanning = stringResource(R.string.home_libraries_scanning)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(start = metrics.marginStart, end = metrics.marginEnd, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(40.dp).background(PleyColors.Surface3, IconShape), contentAlignment = Alignment.Center) {
            Icon(kindIcon(entry.library.kind), contentDescription = null, size = 22.dp, tint = PleyColors.Fg)
        }
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(entry.library.name, style = MaterialTheme.typography.titleMedium, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(entry.count, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted, maxLines = 1)
        }
        Spacer(Modifier.width(12.dp))
        if (entry.scanning) {
            CircularProgressIndicator(
                modifier = Modifier.size(20.dp).semantics { contentDescription = scanning },
                color = PleyColors.Accent,
                trackColor = PleyColors.Surface4,
                strokeWidth = 2.dp,
            )
        } else {
            Icon(PleyIcons.ChevronRight, contentDescription = null, size = 20.dp, tint = PleyColors.Muted)
        }
    }
}

/** Rows in the list's shape while nothing was saved, after a moment. */
@Composable
private fun LibrariesSkeleton() {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(400)
        shown = true
    }
    val loading = stringResource(R.string.home_libraries_loading)
    Column(Modifier.fillMaxSize().padding(vertical = 8.dp).semantics { contentDescription = loading }) {
        if (!shown) return@Column
        val state = rememberSkeleton(animated = true)
        val metrics = LocalPhoneMetrics.current
        repeat(4) {
            Row(Modifier.fillMaxWidth().height(72.dp).padding(start = metrics.marginStart, end = metrics.marginEnd), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(40.dp).skeleton(state, IconShape))
                Spacer(Modifier.width(16.dp))
                Column {
                    Box(Modifier.size(140.dp, 16.dp).skeleton(state, BarShape))
                    Spacer(Modifier.height(8.dp))
                    Box(Modifier.size(80.dp, 12.dp).skeleton(state, BarShape))
                }
            }
        }
    }
}

/** A library's icon by kind, as the TV's rail draws it. */
internal fun kindIcon(kind: LibraryKind): PleyIcon = when (kind) {
    LibraryKind.Movies -> PleyIcons.Clapperboard
    LibraryKind.Shows -> PleyIcons.Tv
    LibraryKind.Music -> PleyIcons.Music
    LibraryKind.Unknown -> PleyIcons.Film
}

private val IconShape = RoundedCornerShape(10.dp)
private val BarShape = RoundedCornerShape(4.dp)
