package app.pley.android.screens.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.LibrariesRepository
import app.pley.android.data.Loadable
import app.pley.android.data.ScanState
import app.pley.android.data.valueOrNull
import app.pley.android.model.Library
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** The list of libraries (the phone's Libraries tab, the TV's Libraries page). */
sealed interface LibrariesState {
    /** Nothing known yet. */
    data object Loading : LibrariesState

    /** The list couldn't be loaded and none was saved: "Couldn't load your libraries". */
    data class Failed(val error: Throwable) : LibrariesState

    /** The libraries in the server's order; empty: "No libraries yet". */
    data class Ready(
        val libraries: List<LibraryEntry>,
        val fresh: Boolean,
        /** The server couldn't be asked again: the list is what was saved. */
        val error: Throwable? = null,
        val refreshing: Boolean = false,
    ) : LibrariesState
}

/**
 * The libraries this profile sees, in the server's order, each with its
 * kind, its name, how much it holds and whether the server is scanning it
 * (the server's events, and the list's own `scanning`).
 */
class LibrariesViewModel(
    private val libraries: () -> LibrariesRepository,
    private val scans: () -> StateFlow<List<ScanState>>,
    private val compute: CoroutineDispatcher = Dispatchers.Default,
) : ViewModel() {
    private val refreshing = MutableStateFlow(false)

    val state: StateFlow<LibrariesState> = combine(
        flow { emitAll(libraries().libraries()) },
        flow { emitAll(scans()) },
        refreshing,
        ::librariesState,
    )
        .distinctUntilChanged()
        .flowOn(compute)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), LibrariesState.Loading)

    /** Asks the server again (pull to refresh, Retry). */
    fun refresh() {
        if (refreshing.value) return
        refreshing.value = true
        viewModelScope.launch(compute) {
            try {
                libraries().refresh()
            } finally {
                refreshing.value = false
            }
        }
    }

    companion object {
        val Factory: ViewModelProvider.Factory = viewModelFactory {
            initializer {
                val graph = screensGraph()
                LibrariesViewModel({ graph.data.libraries }, { graph.data.liveUpdates.scans })
            }
        }
    }
}

/** The list from what the repository holds and the scans running. */
fun librariesState(libraries: Loadable<List<Library>>, scans: List<ScanState>, refreshing: Boolean = false): LibrariesState {
    val list = libraries.valueOrNull
        ?: return if (libraries is Loadable.Failed) LibrariesState.Failed(libraries.error) else LibrariesState.Loading
    val scanning = scans.mapNotNull { it.libraryId }.toSet()
    // A scan the server didn't say the library of is shown on none: the list's own flag says which.
    return LibrariesState.Ready(
        libraries = list.map { LibraryEntry(it, libraryCount(it), it.scanning || it.id in scanning) },
        fresh = libraries is Loadable.Ready && libraries.fresh,
        error = (libraries as? Loadable.Failed)?.error,
        refreshing = refreshing,
    )
}
