@file:OptIn(ExperimentalCoroutinesApi::class, FlowPreview::class)

package app.pley.android.screens.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.pley.android.data.Loadable
import app.pley.android.data.SearchRepository
import app.pley.android.model.ItemKind
import app.pley.android.model.ItemSummary
import app.pley.android.model.PersonSummary
import app.pley.android.model.SearchResults
import app.pley.android.screens.PleyKey
import app.pley.android.screens.ScreensGraph
import app.pley.android.screens.library.LibraryViewModel
import app.pley.android.screens.library.TileFactory
import app.pley.android.screens.library.TileLook
import java.util.IdentityHashMap
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.flow.runningFold
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** What search reads: the server's answers, recent searches, and the server's address for pictures. */
class SearchDeps(
    val search: SearchRepository,
    val origin: Flow<String?>,
    val compute: CoroutineDispatcher = Dispatchers.Default,
) {
    companion object {
        fun of(graph: ScreensGraph): SearchDeps = SearchDeps(graph.data.search, graph.data.servers.active)
    }
}

/**
 * The tiles of search results: [TileFactory]'s for items, and a person's
 * (a circle with their photo and name).
 */
interface SearchTiles<T : Any> {
    fun item(item: ItemSummary, look: TileLook, origin: String): T

    fun person(person: PersonSummary, origin: String): T

    companion object {
        /** Items from [items], people from [person]. */
        fun <T : Any> of(items: TileFactory<T>, person: (PersonSummary, String) -> T): SearchTiles<T> = object : SearchTiles<T> {
            override fun item(item: ItemSummary, look: TileLook, origin: String): T = items.item(item, look, origin)

            override fun person(person: PersonSummary, origin: String): T = person(person, origin)
        }
    }
}

/**
 * The groups results come in, in the order they show (the web's
 * SearchPage): people first, then each kind of item. [look] is how its
 * items are drawn; people are circles.
 */
enum class SearchGroupKind(val look: TileLook, internal val kind: ItemKind?) {
    People(TileLook.Poster, null),
    Movies(TileLook.Poster, ItemKind.Movie),
    Shows(TileLook.Poster, ItemKind.Show),
    Seasons(TileLook.Poster, ItemKind.Season),
    Episodes(TileLook.Landscape, ItemKind.Episode),
    Artists(TileLook.Square, ItemKind.Artist),
    Albums(TileLook.Square, ItemKind.Album),
    Tracks(TileLook.Square, ItemKind.Track),
}

/** One group of results: its [tiles], [ids] beside them. */
data class SearchGroup<T : Any>(
    val kind: SearchGroupKind,
    val tiles: List<T>,
    val ids: List<Long>,
) {
    val count: Int get() = tiles.size
}

/** What the results part of the page shows. */
sealed interface SearchResultsState<out T : Any> {
    /** Nothing typed: recent searches, or what search finds. */
    data object Idle : SearchResultsState<Nothing>

    /** The first answer for [query] is on its way, with nothing earlier to keep showing. */
    data class Loading(val query: String) : SearchResultsState<Nothing>

    /**
     * Results for [query], grouped. While [loading], a newer query's answer
     * is on its way and these stay until it comes.
     */
    data class Ready<T : Any>(val query: String, val groups: List<SearchGroup<T>>, val loading: Boolean) : SearchResultsState<T>

    /** `No results for "north"`. */
    data class Empty(val query: String) : SearchResultsState<Nothing>

    /** "Search failed", with the error's sentence. */
    data class Failed(val query: String, val error: Throwable) : SearchResultsState<Nothing>
}

/** The page: what is typed, the profile's recent searches (newest first) and the results. */
data class SearchUiState<T : Any>(
    val query: String,
    val recent: List<String>,
    val results: SearchResultsState<T>,
)

/**
 * Search (apps/web/src/pages/SearchPage.tsx; design spec §4.5), shared by
 * phones and TVs.
 *
 * What is typed is asked for once typing has paused 250 ms, and only the
 * latest query's answer counts: a newer one replaces a request still on
 * its way. The results already shown stay while the next ones load.
 * Results come in groups: People, Movies, Shows, Seasons, Episodes,
 * Artists, Albums, Tracks, each with its count. A query is kept among the
 * profile's last 10 searches when it is submitted or one of its results
 * is opened.
 */
class SearchViewModel<T : Any>(
    initialQuery: String,
    private val deps: SearchDeps,
    private val tiles: SearchTiles<T>,
) : ViewModel() {
    private val typed = MutableStateFlow(initialQuery)

    /** What is typed now, as the field shows it. */
    val query: StateFlow<String> = typed.asStateFlow()

    /** The items and people of the results shown, by id, for opening them. */
    @Volatile
    private var shown: Shown = Shown("", emptyMap(), emptySet())

    private class Shown(val query: String, val items: Map<Long, ItemSummary>, val people: Set<Long>)

    private val results: Flow<SearchResultsState<T>> = typed
        .map { it.trim() }
        .distinctUntilChanged()
        .debounce { if (it.isEmpty()) 0L else DEBOUNCE_MS }
        .flatMapLatest { q ->
            if (q.isEmpty()) {
                flowOf<Step<T>>(Step.Idle)
            } else {
                combine(deps.search.search(q), deps.origin.map { it.orEmpty() }.distinctUntilChanged()) { load, origin -> step(q, load, origin) }
                    .onStart { emit(Step.Asking(q)) }
            }
        }
        .runningFold<Step<T>, SearchResultsState<T>>(SearchResultsState.Idle) { before, step -> next(before, step) }
        .distinctUntilChanged()
        .flowOn(deps.compute)

    val state: StateFlow<SearchUiState<T>> = combine(typed, deps.search.recent, results) { q, recent, results ->
        SearchUiState(q, recent, results)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(STOP_AFTER_MS), SearchUiState(initialQuery, emptyList(), SearchResultsState.Idle))

    /** The field changed. */
    fun onQueryChange(text: String) {
        typed.value = text
    }

    /** Search was pressed on the keyboard: the query is kept among the recent ones. */
    fun submit() {
        val q = typed.value.trim()
        if (q.isNotEmpty()) viewModelScope.launch { deps.search.remember(q) }
    }

    /** A recent search was chosen: it is searched again, and moves to the top. */
    fun searchAgain(query: String) {
        typed.value = query
        submit()
    }

    fun clearRecent() {
        viewModelScope.launch { deps.search.clearRecent() }
    }

    /**
     * Where opening the result [id] of [group] goes; the query is kept
     * among the recent searches, since one of its results was wanted.
     */
    fun open(group: SearchGroupKind, id: Long): PleyKey? {
        val now = shown
        val key = if (group == SearchGroupKind.People) {
            PleyKey.Person(id).takeIf { id in now.people }
        } else {
            now.items[id]?.let(::opens)
        }
        if (key != null && now.query.isNotEmpty()) viewModelScope.launch { deps.search.remember(now.query) }
        return key
    }

    // The steps from what is typed to what shows.

    private sealed interface Step<out T : Any> {
        data object Idle : Step<Nothing>

        data class Asking(val query: String) : Step<Nothing>

        data class Answer<T : Any>(val query: String, val groups: List<SearchGroup<T>>, val stillLoading: Boolean) : Step<T>

        data class Failed(val query: String, val error: Throwable) : Step<Nothing>
    }

    private fun step(q: String, load: Loadable<SearchResults>, origin: String): Step<T> = when (load) {
        Loadable.Loading -> Step.Asking(q)
        is Loadable.Ready -> Step.Answer(q, groups(q, load.value, origin), stillLoading = false)
        is Loadable.Failed -> load.stale?.let { Step.Answer(q, groups(q, it, origin), stillLoading = false) } ?: Step.Failed(q, load.error)
    }

    private fun next(before: SearchResultsState<T>, step: Step<T>): SearchResultsState<T> = when (step) {
        Step.Idle -> SearchResultsState.Idle
        is Step.Asking -> when (before) {
            // Keep showing what is there while the new answer comes.
            is SearchResultsState.Ready -> before.copy(loading = true)
            else -> SearchResultsState.Loading(step.query)
        }
        is Step.Answer -> if (step.groups.isEmpty()) {
            SearchResultsState.Empty(step.query)
        } else {
            SearchResultsState.Ready(step.query, step.groups, loading = step.stillLoading)
        }
        is Step.Failed -> SearchResultsState.Failed(step.query, step.error)
    }

    /** Tiles already made, by the item or person they show, so a new answer only makes what is new. */
    private var made = IdentityHashMap<Any, T>()
    private var madeFor = ""

    private fun groups(q: String, results: SearchResults, origin: String): List<SearchGroup<T>> {
        if (madeFor != origin) {
            made = IdentityHashMap()
            madeFor = origin
        }
        val next = IdentityHashMap<Any, T>()
        val grouped = searchGroups(results)
        val out = grouped.map { (kind, entries) ->
            val ids = ArrayList<Long>(entries.size)
            val built = entries.map { entry ->
                val tile = made[entry] ?: when (entry) {
                    is PersonSummary -> tiles.person(entry, origin)
                    is ItemSummary -> tiles.item(entry, kind.look, origin)
                    else -> error("not a result: $entry")
                }
                next[entry] = tile
                ids += if (entry is PersonSummary) entry.id else (entry as ItemSummary).id
                tile
            }
            SearchGroup(kind, built, ids)
        }
        made = next
        shown = Shown(q, results.items.associateBy { it.id }, results.people.mapTo(HashSet()) { it.id })
        return out
    }

    companion object {
        /** How long typing pauses before what is typed is searched. */
        const val DEBOUNCE_MS = 250L

        /** How long the results keep being followed after the page stops showing. */
        const val STOP_AFTER_MS = 5_000L

        /**
         * [results] in their groups and the groups' order (people, movies,
         * shows, seasons, episodes, artists, albums, tracks), each keeping
         * the server's order of relevance; empty groups left out. Items of
         * other kinds (extras) aren't shown, as on the web.
         */
        fun searchGroups(results: SearchResults): List<Pair<SearchGroupKind, List<Any>>> = buildList {
            if (results.people.isNotEmpty()) add(SearchGroupKind.People to results.people)
            val byKind = results.items.groupBy { it.kind }
            for (kind in SearchGroupKind.entries) {
                val items = kind.kind?.let(byKind::get).orEmpty()
                if (items.isNotEmpty()) add(kind to items)
            }
        }

        /** Where a result opens: artists and albums their music pages, a track its album, the rest their item page. */
        fun opens(item: ItemSummary): PleyKey = when (item.kind) {
            ItemKind.Track -> item.parentId?.let { PleyKey.Album(it) } ?: PleyKey.Item(item.id)
            else -> LibraryViewModel.opens(item)
        }
    }
}
