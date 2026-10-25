package app.pley.android.ui.phone.shell

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SheetState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.NavKey
import androidx.navigation3.scene.OverlayScene
import androidx.navigation3.scene.Scene
import androidx.navigation3.scene.SceneStrategy
import androidx.navigation3.scene.SceneStrategyScope
import app.pley.android.screens.PleyKey
import app.pley.android.screens.isSheet
import app.pley.android.ui.phone.kit.PleySheet

/** An entry's metadata saying it shows as a bottom sheet over the screen under it. */
internal const val SHEET = "pley.sheet"

/**
 * Shows sheet entries (an item's actions, adding to a playlist: [isSheet])
 * as a [PleySheet] over the screen they were opened from, which stays
 * composed under it. Back or a drag down closes the sheet: the entry
 * leaves the back stack after the sheet has slid away.
 */
internal class SheetSceneStrategy : SceneStrategy<NavKey> {
    override fun SceneStrategyScope<NavKey>.calculateScene(entries: List<NavEntry<NavKey>>): Scene<NavKey>? {
        val last = entries.lastOrNull() ?: return null
        if (last.metadata[SHEET] != true) return null
        val under = entries.dropLast(1)
        return SheetScene(last, under, onBack)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
private class SheetScene(
    private val entry: NavEntry<NavKey>,
    override val previousEntries: List<NavEntry<NavKey>>,
    private val onBack: () -> Unit,
) : OverlayScene<NavKey> {
    private var sheet: SheetState? = null

    override val key: Any = entry.contentKey
    override val entries: List<NavEntry<NavKey>> = listOf(entry)
    override val overlaidEntries: List<NavEntry<NavKey>> = previousEntries

    override val content: @Composable () -> Unit = {
        val state = rememberModalBottomSheetState(skipPartiallyExpanded = true)
        sheet = state
        PleySheet(onDismiss = onBack, state = state) { entry.Content() }
    }

    override suspend fun onRemove() {
        sheet?.hide()
    }

    override fun equals(other: Any?): Boolean = other is SheetScene && other.key == key && other.previousEntries == previousEntries

    override fun hashCode(): Int = 31 * key.hashCode() + previousEntries.hashCode()
}

/** [entry] marked as a sheet when [key] is one, keeping its content and the rest of its metadata. */
internal fun asSheetIfOne(key: NavKey, entry: NavEntry<NavKey>): NavEntry<NavKey> {
    if (key !is PleyKey || !key.isSheet || entry.metadata[SHEET] == true) return entry
    return NavEntry(key, entry.contentKey, entry.metadata + (SHEET to true)) { entry.Content() }
}
