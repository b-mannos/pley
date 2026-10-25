@file:OptIn(ExperimentalCoroutinesApi::class)

package app.pley.android.screens.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.pley.android.data.AppSettings
import app.pley.android.data.CollectionsRepository
import app.pley.android.data.GridDensity
import app.pley.android.data.HubsRepository
import app.pley.android.data.LibrariesRepository
import app.pley.android.data.LibraryItems
import app.pley.android.data.LibraryItemsRepository
import app.pley.android.data.Loadable
import app.pley.android.data.map
import app.pley.android.data.valueOrNull
import app.pley.android.model.Hub
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.Library
import app.pley.android.model.LibraryFilters
import app.pley.android.model.LibraryKind
import app.pley.android.model.LibrarySort
import app.pley.android.model.SortOrder
import app.pley.android.screens.LibraryTab
import app.pley.android.screens.PleyKey
import app.pley.android.screens.ScreensGraph
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapLatest
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.scan
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** What a library page reads, and from where. */
class LibraryDeps(
    val libraries: LibrariesRepository,
    val items: LibraryItemsRepository,
    val hubs: HubsRepository,
    val collections: CollectionsRepository,
    val settings: AppSettings,
    /** The server in use (its origin), for the pictures. */
    val origin: Flow<String?>,
    /** Where tiles and the A to Z are worked out. */
    val compute: CoroutineDispatcher = Dispatchers.Default,
) {
    companion object {
        fun of(graph: ScreensGraph): LibraryDeps = LibraryDeps(
            libraries = graph.data.libraries,
            items = graph.data.libraryItems,
            hubs = graph.data.hubs,
            collections = graph.data.collections,
            settings = graph.data.settings,
            origin = graph.data.servers.active,
        )
    }
}

/**
 * A row of tiles: one of the library's hubs, or its collections
 * ([COLLECTIONS], which the interface titles). [opens] says where each
 * tile goes, by position.
 */
data class TileRow<T : Any>(
    val key: String,
    /** The server's title; `null` for [COLLECTIONS]. */
    val title: String?,
    val look: TileLook,
    val tiles: List<T>,
    val opens: List<PleyKey>,
) {
    companion object {
        /** The key of the library's collections row. */
        const val COLLECTIONS = "collections"
    }
}

/** A collection's tile, with its count for the badge on its poster. */
data class CollectionTile<T : Any>(val id: Long, val count: Int, val tile: T)

/** The Library tab's grid: its cells, the A to Z when it has one, how its tiles look. */
data class LibraryGrid<T : Any>(
    val cells: Loadable<GridCells<T>>,
    val look: TileLook,
    /** Sorted by title, every item loaded, more than 30 of them. */
    val index: AlphaIndex?,
)

/**
 * A library page (design spec §4.2): its name, the tabs, and each tab's
 * content. [library] is `null` until known; [missing] when the server has
 * no such library for this profile.
 */
data class LibraryUiState<T : Any>(
    val libraryId: Long,
    val library: Library? = null,
    val missing: Boolean = false,
    /** Why the library couldn't be read, while nothing saved shows. */
    val libraryError: Throwable? = null,
    val params: LibraryParams = LibraryParams(),
    val tabs: List<LibraryTab> = LibraryTab.entries,
    val sorts: List<LibrarySort> = LibraryParams.sortsFor(LibraryKind.Movies),
    /** Genres and years the listing can be filtered by (none for music). */
    val filters: LibraryFilters = LibraryFilters(),
    val grid: LibraryGrid<T> = LibraryGrid(Loadable.Loading, TileLook.Poster, null),
    val density: GridDensity = GridDensity.Medium,
    /** The Recommended tab's rows, its hubs and then the collections. */
    val recommended: Loadable<List<TileRow<T>>> = Loadable.Loading,
    val collections: Loadable<List<CollectionTile<T>>> = Loadable.Loading,
) {
    val kind: LibraryKind? get() = library?.kind

    /** The listing is narrowed by a filter. */
    val filtered: Boolean get() = params.filtered

    /** Where a tile of the grid goes: an artist's page in music, else the item's. */
    fun opens(itemId: Long): PleyKey = if (kind == LibraryKind.Music) PleyKey.Artist(itemId) else PleyKey.Item(itemId)
}

/**
 * One library's page, shared by phones and TVs (apps/web/src/pages/LibraryPage.tsx):
 *
 * - **Recommended:** the library's hubs, then its collections.
 * - **Library:** every item, in a grid sized from the listing's total;
 *   sorted, ordered and filtered by watched state, genre and year as the
 *   web does, with the A to Z when sorted by title. Sorted by bitrate,
 *   tiles say what the files are like.
 * - **Collections:** the library's collections (not for music).
 *
 * [params] are the tab, sort and filters, which the screen keeps across
 * process death ([LibraryParams.toQueryString] of [LibraryUiState.params]),
 * handing them back as [saved]; a page opened anew starts on [tab].
 * Each tab reads only once it has been shown. Tiles and the A to Z are
 * worked out off the main thread.
 */
class LibraryViewModel<T : Any>(
    val libraryId: Long,
    tab: LibraryTab,
    saved: String?,
    private val deps: LibraryDeps,
    private val tiles: TileFactory<T>,
) : ViewModel() {
    private val params = MutableStateFlow(saved?.let { LibraryParams.parse(it, tab) } ?: LibraryParams(tab = tab))
    private val retry = MutableStateFlow(0)
    private val builder = GridBuilder(tiles)

    private val libraryLoad: Flow<Loadable<Library?>> = retry.flatMapLatest {
        deps.libraries.libraries().map { loadable -> loadable.map { list -> list.firstOrNull { it.id == libraryId } } }
    }

    private val library: Flow<Library?> = libraryLoad.map { it.valueOrNull }.distinctUntilChanged()

    private val kind: Flow<LibraryKind> = library.filterNotNull().map { it.kind }.distinctUntilChanged()

    private val origin: Flow<String> = deps.origin.map { it.orEmpty() }.distinctUntilChanged()

    /** The params a library of the kind can show, once the kind is known. */
    private val shown: Flow<LibraryParams> = combine(params, library) { p, lib -> lib?.let { p.validFor(it.kind) } ?: p }
        .distinctUntilChanged()

    /** Tabs shown so far: each reads from the first time it shows, and keeps reading after. */
    private val visited: Flow<Set<LibraryTab>> = shown.map { it.tab }.scan(emptySet<LibraryTab>()) { seen, tab -> seen + tab }

    private fun opened(tab: LibraryTab): Flow<Boolean> = visited.map { tab in it }.distinctUntilChanged()

    private val grid: Flow<LibraryGrid<T>> = opened(LibraryTab.Library).flatMapLatest { open ->
        if (!open) return@flatMapLatest flowOf(LibraryGrid<T>(Loadable.Loading, TileLook.Poster, null))
        combine(kind, shown, origin, retry) { kind, p, origin, _ -> GridKey(kind, p, origin) }
            .distinctUntilChanged()
            .flatMapLatest { key ->
                deps.items.items(libraryId, key.params.query())
                    .mapLatest { loadable -> grid(loadable, key) }
                    .onStart { emit(LibraryGrid(Loadable.Loading, key.look, null)) }
            }
            .onStart { emit(LibraryGrid(Loadable.Loading, TileLook.Poster, null)) }
    }.flowOn(deps.compute)

    private val filters: Flow<LibraryFilters> = opened(LibraryTab.Library).flatMapLatest { open ->
        if (!open) return@flatMapLatest flowOf(LibraryFilters())
        kind.flatMapLatest { kind ->
            if (kind == LibraryKind.Music) flowOf(LibraryFilters()) else deps.items.filters(libraryId).map { it.valueOrNull ?: LibraryFilters() }
        }.onStart { emit(LibraryFilters()) }
    }.distinctUntilChanged()

    private val collectionTiles: Flow<Loadable<List<CollectionTile<T>>>> = combine(library, origin) { lib, o -> lib to o }
        .flatMapLatest { (lib, origin) ->
            if (lib == null || lib.kind == LibraryKind.Music) return@flatMapLatest flowOf(Loadable.Ready(emptyList(), fresh = true))
            deps.collections.collections(libraryId).map { loadable ->
                loadable.map { list -> list.map { CollectionTile(it.id, it.itemCount, tiles.collection(it, lib.kind, origin)) } }
            }
        }
        .flowOn(deps.compute)

    private val collections: Flow<Loadable<List<CollectionTile<T>>>> =
        combine(opened(LibraryTab.Collections), opened(LibraryTab.Recommended)) { a, b -> a || b }
            .distinctUntilChanged()
            .flatMapLatest { open -> if (open) collectionTiles else flowOf(Loadable.Loading) }

    private val recommended: Flow<Loadable<List<TileRow<T>>>> = opened(LibraryTab.Recommended).flatMapLatest { open ->
        if (!open) return@flatMapLatest flowOf(Loadable.Loading)
        combine(kind, origin) { k, o -> k to o }.flatMapLatest { (kind, origin) ->
            combine(deps.hubs.library(libraryId), collections) { hubs, collections ->
                hubs.map { value ->
                    val rows = value.hubs.filter { it.items.isNotEmpty() }.map { row(it, kind, origin) }
                    val shelf = collections.valueOrNull.orEmpty()
                    if (shelf.isEmpty()) {
                        rows
                    } else {
                        rows + TileRow(
                            key = TileRow.COLLECTIONS,
                            title = null,
                            look = TileLook.Poster,
                            tiles = shelf.map { it.tile },
                            opens = shelf.map { PleyKey.Collection(it.id) },
                        )
                    }
                }
            }
        }.onStart { emit(Loadable.Loading) }
    }.flowOn(deps.compute)

    private val density: Flow<GridDensity> = deps.settings.settings.map { it.gridDensity }.distinctUntilChanged()

    val state: StateFlow<LibraryUiState<T>> = combine(
        combine(libraryLoad, shown) { load, p -> load to p },
        grid,
        filters,
        density,
        combine(recommended, collections) { r, c -> r to c },
    ) { (load, p), grid, filters, density, (recommended, collections) ->
        val lib = load.valueOrNull
        LibraryUiState(
            libraryId = libraryId,
            library = lib,
            missing = load is Loadable.Ready && lib == null,
            libraryError = (load as? Loadable.Failed)?.takeIf { lib == null }?.error,
            params = p,
            tabs = LibraryParams.tabsFor(lib?.kind),
            sorts = LibraryParams.sortsFor(lib?.kind ?: LibraryKind.Movies),
            filters = filters,
            grid = grid,
            density = density,
            recommended = recommended,
            collections = collections,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_AFTER_MS), LibraryUiState(libraryId, params = params.value))

    /** Shows [tab]; its content starts reading the first time. */
    fun select(tab: LibraryTab) = params.update { it.copy(tab = tab) }

    /** Sorts by [sort] in its own order (titles A to Z, the rest newest or best first). */
    fun sortBy(sort: LibrarySort) = params.update { if (it.sort == sort) it else it.sortedBy(sort) }

    fun order(order: SortOrder) = params.update { it.copy(order = order) }

    fun flipOrder() = params.update { it.flipped() }

    /** `false` unwatched only, `true` watched only, `null` both. */
    fun watched(watched: Boolean?) = params.update { it.copy(watched = watched) }

    fun genre(genre: String?) = params.update { it.copy(genre = genre) }

    fun year(year: Int?) = params.update { it.copy(year = year) }

    fun clearFilters() = params.update { it.cleared() }

    /** Cards smaller or larger, for every grid on this device. */
    fun density(density: GridDensity) {
        viewModelScope.launch { deps.settings.update { it.copy(gridDensity = density) } }
    }

    /** Reads what failed again. */
    fun retry() {
        retry.update { it + 1 }
        viewModelScope.launch { runCatching { deps.libraries.refresh() } }
    }

    /** The grid shows [position] now: a big library's window follows it. Cheap; call it as the grid scrolls. */
    fun showing(position: Int) {
        val p = state.value.params
        deps.items.showing(libraryId, p.query(), position)
    }

    /** "Save as…": the smart playlist sheet, with the listing's filters in Plex's syntax. */
    fun saveAs(): PleyKey = PleyKey.SaveSmartPlaylist(libraryId, SmartFilter.of(state.value.params))

    private fun grid(loadable: Loadable<LibraryItems>, key: GridKey): LibraryGrid<T> {
        val cells = loadable.map { builder.build(it, key.look, key.origin) }
        val items = loadable.valueOrNull
        val index = if (key.params.sort == LibrarySort.Title && items != null && items.complete && items.total > AlphaIndex.MIN_ITEMS) {
            AlphaIndex.of(items.items)
        } else {
            null
        }
        return LibraryGrid(cells, key.look, index)
    }

    private fun row(hub: Hub, kind: LibraryKind, origin: String): TileRow<T> {
        val look = when {
            hub.key.startsWith("continue_watching") -> TileLook.Landscape
            kind == LibraryKind.Music -> TileLook.Square
            else -> TileLook.Poster
        }
        return TileRow(
            key = hub.key,
            title = hub.title,
            look = look,
            tiles = hub.items.map { tiles.item(it, look, origin) },
            opens = hub.items.map { opens(it) },
        )
    }

    private data class GridKey(val kind: LibraryKind, val params: LibraryParams, val origin: String) {
        val look: TileLook
            get() = when {
                kind == LibraryKind.Music -> TileLook.Square
                params.sort == LibrarySort.Bitrate -> TileLook.Bitrate
                else -> TileLook.Poster
            }
    }

    companion object {
        /** How long a page's reads outlive it being on screen (a rotation, a quick look elsewhere). */
        const val STOP_AFTER_MS = 5_000L

        /** Where opening [item] goes: artists and albums have pages of their own. */
        fun opens(item: ItemSummary): PleyKey = when (item.kind) {
            ItemKind.Artist -> PleyKey.Artist(item.id)
            ItemKind.Album -> PleyKey.Album(item.id)
            else -> PleyKey.Item(item.id)
        }
    }
}
