package app.pley.android.ui.phone.shell

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
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
import app.pley.android.common.TestTags
import app.pley.android.designsystem.PleyColors
import app.pley.android.screens.PleyKey
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.accountEntries
import app.pley.android.ui.phone.home.homeEntries
import app.pley.android.ui.phone.item.itemEntries
import app.pley.android.ui.phone.library.libraryEntries
import app.pley.android.ui.phone.music.musicEntries
import app.pley.android.ui.phone.onboarding.onboardingEntries
import app.pley.android.ui.phone.playlists.playlistsEntries
import app.pley.android.ui.phone.search.searchEntries
import app.pley.android.ui.phone.theme.PhoneTheme

/**
 * The phone and tablet app. A placeholder for now: one back stack whose
 * screens each area adds (its <Area>Entries), and a placeholder for any
 * screen not built yet.
 */
@Composable
fun PhoneApp() {
    PhoneTheme {
        Surface(color = PleyColors.Bg, modifier = Modifier.fillMaxSize().testTag(TestTags.PHONE_ROOT)) {
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

/** A screen that isn't built yet: its name, and a way to the next placeholder. */
@Composable
private fun Placeholder(key: NavKey, open: (NavKey) -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .safeDrawingPadding()
            .padding(24.dp)
            .testTag(TestTags.PLACEHOLDER),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("pley", fontSize = 28.sp, fontWeight = FontWeight.ExtraBold, color = PleyColors.Fg)
        Text(
            stringResource(R.string.shell_placeholder_title, key.javaClass.simpleName),
            style = MaterialTheme.typography.titleLarge,
            color = PleyColors.Fg,
        )
        Text(stringResource(R.string.shell_placeholder_body), color = PleyColors.Muted)
        if (key == PleyKey.Home) {
            Button(onClick = { open(PleyKey.Libraries) }) { Text(stringResource(R.string.shell_placeholder_next)) }
        }
    }
}
