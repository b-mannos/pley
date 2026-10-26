package app.pley.android.ui.tv.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.LocalContentColor
import androidx.tv.material3.Surface
import app.pley.android.data.Loadable
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.LibraryKind
import app.pley.android.screens.PleyKey
import app.pley.android.screens.home.LibrariesState
import app.pley.android.screens.home.LibrariesViewModel
import app.pley.android.screens.home.LibraryEntry
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.rememberFocusMemory
import app.pley.android.ui.tv.kit.rememberedFocus
import app.pley.android.ui.tv.shell.LocalTvNavigator
import app.pley.android.ui.tv.shell.ReportOffline
import app.pley.android.ui.tv.shell.failedBody
import app.pley.android.ui.tv.shell.failedTitle

/**
 * Every library (design spec §2.1): the rail shows the first three, and
 * "Libraries" opens this page. The page title, then a card per library in
 * the server's order: its kind's icon, its name, how much it holds, and
 * "Scanning…" while the server scans it. Initial focus on the first card,
 * or on the one opened last when coming back.
 */
@Composable
internal fun TvLibrariesScreen(vm: LibrariesViewModel = viewModel(factory = LibrariesViewModel.Factory)) {
    val state by vm.state.collectAsStateWithLifecycle()
    val navigator = LocalTvNavigator.current
    ReportOffline(
        when (val s = state) {
            is LibrariesState.Ready -> s.error?.let { Loadable.Failed(it, Unit) } ?: Loadable.Ready(Unit, s.fresh)
            is LibrariesState.Failed -> Loadable.Failed(s.error, Unit)
            LibrariesState.Loading -> null
        },
    )
    when (val s = state) {
        LibrariesState.Loading -> Unit
        is LibrariesState.Failed -> TvHomeNotice(failedTitle(s.error, stringResource(R.string.tv_libraries_failed)), failedBody(s.error), retry = vm::refresh)
        is LibrariesState.Ready -> if (s.libraries.isEmpty()) {
            TvHomeNotice(stringResource(R.string.tv_home_no_libraries), stringResource(R.string.tv_home_no_libraries_body))
        } else {
            val memory = rememberFocusMemory()
            Column(Modifier.fillMaxSize().padding(top = TvSafeArea.Vertical)) {
                BasicText(
                    stringResource(R.string.tv_libraries_title),
                    style = PleyType.Tv.PageTitle.copy(color = PleyColors.Fg),
                    modifier = Modifier.padding(start = PleyTv.Gutter).semantics { heading() },
                )
                Spacer(Modifier.height(20.dp))
                LazyVerticalGrid(
                    columns = GridCells.Fixed(COLUMNS),
                    state = rememberLazyGridState(),
                    contentPadding = PaddingValues(start = PleyTv.Gutter, end = TvSafeArea.Horizontal, top = 8.dp, bottom = TvSafeArea.Vertical),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    itemsIndexed(s.libraries, key = { _, entry -> entry.library.id }, contentType = { _, _ -> "library" }) { index, entry ->
                        LibraryCard(
                            entry,
                            onClick = { navigator.open(PleyKey.Library(entry.library.id)) },
                            modifier = Modifier.rememberedFocus(memory, "library/${entry.library.id}", initial = index == 0),
                        )
                    }
                }
            }
        }
    }
}

/**
 * One library: a [surface2][PleyColors.Surface2] card; focused, the accent
 * fill with [onAccent][PleyColors.OnAccent] content at 1.05 (design spec
 * §5.2 rule 1).
 */
@Composable
private fun LibraryCard(entry: LibraryEntry, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(PleyShape.CardRadius)
    Surface(
        onClick = onClick,
        modifier = modifier.fillMaxWidth().height(CardHeight),
        shape = ClickableSurfaceDefaults.shape(shape),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = PleyColors.Surface2,
            contentColor = PleyColors.Fg,
            focusedContainerColor = PleyColors.Accent,
            focusedContentColor = PleyColors.OnAccent,
            pressedContainerColor = PleyColors.AccentBright,
            pressedContentColor = PleyColors.OnAccent,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.05f, pressedScale = 1f),
    ) {
        val content = LocalContentColor.current
        Row(Modifier.fillMaxSize().padding(horizontal = 18.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(PleyTv.ButtonHeight).drawBehind { drawRoundRect(content.copy(alpha = 0.12f), cornerRadius = CornerRadius(PleyShape.ControlRadius.toPx())) },
                contentAlignment = Alignment.Center,
            ) {
                Icon(kindIcon(entry.library.kind), contentDescription = null, size = PleyTv.IconSize, tint = content)
            }
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                BasicText(
                    entry.library.name,
                    style = PleyType.Tv.Title.copy(color = content),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(2.dp))
                val line = if (entry.scanning) "${entry.count} · ${stringResource(R.string.tv_libraries_scanning)}" else entry.count
                BasicText(line, style = PleyType.Tv.Caption.copy(color = content.copy(alpha = 0.75f)), maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** A library's icon by kind, as the rail draws it. */
internal fun kindIcon(kind: LibraryKind): PleyIcon = when (kind) {
    LibraryKind.Movies -> PleyIcons.Clapperboard
    LibraryKind.Shows -> PleyIcons.Tv
    LibraryKind.Music -> PleyIcons.Music
    LibraryKind.Unknown -> PleyIcons.Film
}

private const val COLUMNS = 3
private val CardHeight = 80.dp
