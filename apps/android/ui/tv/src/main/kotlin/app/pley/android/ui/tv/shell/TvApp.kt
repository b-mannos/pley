package app.pley.android.ui.tv.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import androidx.tv.material3.Button
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.pley.android.common.TestTags
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.TvSafeArea
import app.pley.android.screens.PleyKey
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.account.accountEntries
import app.pley.android.ui.tv.home.homeEntries
import app.pley.android.ui.tv.item.itemEntries
import app.pley.android.ui.tv.library.libraryEntries
import app.pley.android.ui.tv.music.musicEntries
import app.pley.android.ui.tv.onboarding.onboardingEntries
import app.pley.android.ui.tv.playlists.playlistsEntries
import app.pley.android.ui.tv.search.searchEntries
import app.pley.android.ui.tv.theme.TvTheme

/**
 * The TV app (Android TV, Google TV, Fire TV). A placeholder for now: one
 * back stack whose screens each area adds, a placeholder for any screen not
 * built yet, and the TV's safe area.
 */
@Composable
fun TvApp() {
    TvTheme {
        Box(Modifier.fillMaxSize().background(PleyColors.Bg).testTag(TestTags.TV_ROOT)) {
            val backStack = rememberNavBackStack(PleyKey.Home)
            NavDisplay(
                backStack = backStack,
                onBack = { backStack.removeLastOrNull() },
                entryProvider = entryProvider(fallback = { key -> NavEntry(key) { Placeholder(it) { next -> backStack.add(next) } } }) {
                    onboardingEntries()
                    homeEntries()
                    libraryEntries()
                    itemEntries()
                    searchEntries()
                    playlistsEntries()
                    accountEntries()
                    musicEntries()
                },
            )
        }
    }
}

/** A screen that isn't built yet: its name, and a focused button, as every TV screen starts with focus. */
@Composable
private fun Placeholder(key: NavKey, open: (NavKey) -> Unit) {
    val focus = remember { FocusRequester() }
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = TvSafeArea.Horizontal, vertical = TvSafeArea.Vertical)
            .testTag(TestTags.PLACEHOLDER),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("pley", fontSize = 36.sp, fontWeight = FontWeight.ExtraBold, color = PleyColors.Fg)
        Text(
            stringResource(R.string.tv_shell_placeholder_title, key.javaClass.simpleName),
            style = MaterialTheme.typography.headlineMedium,
            color = PleyColors.Fg,
        )
        Text(
            stringResource(R.string.tv_shell_placeholder_body),
            style = MaterialTheme.typography.bodyLarge,
            color = PleyColors.Muted,
            modifier = Modifier.widthIn(max = 640.dp),
        )
        val next = if (key == PleyKey.Home) PleyKey.Libraries else PleyKey.Search()
        Button(onClick = { open(next) }, modifier = Modifier.focusRequester(focus)) {
            Text(stringResource(R.string.tv_shell_placeholder_next))
        }
    }
    LaunchedEffect(key) { focus.requestFocus() }
}
