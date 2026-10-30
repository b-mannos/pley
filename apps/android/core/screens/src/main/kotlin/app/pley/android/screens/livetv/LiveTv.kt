package app.pley.android.screens.livetv

import app.pley.android.data.LiveTvRepository
import app.pley.android.data.Loadable
import app.pley.android.data.valueOrNull
import app.pley.android.model.LiveCategory
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveChannelPage
import app.pley.android.model.LiveGroup
import app.pley.android.model.LiveGuide
import app.pley.android.model.LiveProgram
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

/** How a Live TV page shows the channels: the guide (channels by time) or cards. */
enum class LiveView { Guide, Channels }

/**
 * An entry of the categories' list (the web's CategoryRail): a group, or
 * a region ("UK") gathering its groups ("UK| Sport", "UK| News"…), so
 * hundreds of groups read as a few dozen.
 */
sealed interface GroupEntry {
    data class Single(val group: LiveGroup) : GroupEntry

    data class Region(val region: String, val groups: List<LiveGroup>, val count: Int) : GroupEntry
}

/**
 * A category's channels as far as they are loaded: [total] in the list
 * (`null` until its first page is in), and the pages in, by number.
 */
data class ChannelPages(
    val total: Int? = null,
    val pages: Map<Int, List<LiveChannel>> = emptyMap(),
    /** The first page failed, with nothing to show. */
    val error: Throwable? = null,
) {
    /** The channel at [index], once its page is in. */
    operator fun get(index: Int): LiveChannel? = pages[index / LiveTvRepository.PAGE]?.getOrNull(index % LiveTvRepository.PAGE)

    /** Every channel loaded, in order. */
    val loaded: List<LiveChannel> get() = pages.toSortedMap().values.flatten()
}

/** Live TV's rules, shared by its pages and its player, the web's (apps/web/src/pages/livetv). */
object LiveTv {
    /** The guide's columns: half an hour each. */
    const val SLOT_S = 30 * 60L

    /** How far ahead the guide goes from where it starts. */
    const val GUIDE_SPAN_S = 48 * 3600L

    /** Groups in the lineup's order, those of a region with two or more gathered under it. */
    fun groupTree(groups: List<LiveGroup>): List<GroupEntry> {
        val byRegion = groups.filter { it.region != null }.groupBy { it.region!! }
        val placed = HashSet<String>()
        return buildList {
            for (g in groups) {
                val name = g.region
                val region = name?.let(byRegion::get)
                when {
                    name == null || region == null || region.size < 2 -> add(GroupEntry.Single(g))
                    placed.add(name) -> add(GroupEntry.Region(name, region, region.sumOf { it.count }))
                }
            }
        }
    }

    /**
     * A name's region and the rest, when it begins with one the way
     * providers write them ("UK| Sport", "[UK] Sport", "UK: Sport"), as the
     * server splits group names.
     */
    fun splitRegion(name: String): Pair<String, String>? {
        val n = name.trim().replace(LEADING_BARS, "")
        val m = BRACKETED.matchEntire(n) ?: BARRED.matchEntire(n) ?: COLON.matchEntire(n) ?: return null
        val region = m.groupValues[1].trim()
        val rest = m.groupValues[2].replace(LEADING_MARKS, "").trim()
        return if (region.isNotEmpty() && rest.isNotEmpty()) region to rest else null
    }

    /**
     * A channel's name as a list shows it: in a group, without the region
     * the group already says ("Sky Sports 1" in UK · Sport, not "UK| Sky
     * Sports 1").
     */
    fun channelName(channel: LiveChannel, category: LiveCategory?): String {
        if (category !is LiveCategory.Group) return channel.name
        val name = splitRegion(channel.name) ?: return channel.name
        val group = splitRegion(channel.group) ?: return channel.name
        return if (name.first.equals(group.first, ignoreCase = true)) name.second else channel.name
    }

    /** "UK · Sport" for a group of a region; its name otherwise. */
    fun groupName(group: LiveGroup?, name: String): String =
        if (group?.region != null) "${group.region} · ${group.label}" else group?.label ?: name

    /** Where the guide starts for [now] (unix seconds): the start of the half hour before the one it is in. */
    fun guideStart(now: Long): Long = Math.floorDiv(now, SLOT_S) * SLOT_S - SLOT_S

    /**
     * The guide's chunks that cover [from] to [to], and one either side, so
     * what comes into view next is there already; none before [start] or
     * past its span.
     */
    fun chunks(start: Long, from: Long, to: Long): List<Long> {
        val size = LiveTvRepository.GUIDE_CHUNK_S
        val first = Math.floorDiv(maxOf(from, start), size) * size - size
        val last = Math.floorDiv(minOf(to, start + GUIDE_SPAN_S), size) * size + size
        val floor = Math.floorDiv(start, size) * size
        return (first..last step size).filter { it >= floor && it < start + GUIDE_SPAN_S }
    }

    /** The programmes of [guides] by channel, each list by start, one programme per start. */
    fun merge(guides: List<LiveGuide>): Map<Long, List<LiveProgram>> {
        val out = HashMap<Long, MutableMap<Long, LiveProgram>>()
        for (guide in guides) {
            for ((channel, programs) in guide.programs) {
                val known = out.getOrPut(channel) { HashMap() }
                for (p in programs) known.putIfAbsent(p.start, p)
            }
        }
        return out.mapValues { (_, byStart) -> byStart.values.sortedBy { it.start } }
    }

    /** The programmes that show between [from] and [to] (unix seconds). */
    fun between(programs: List<LiveProgram>?, from: Long, to: Long): List<LiveProgram> =
        programs.orEmpty().filter { it.stop > from && it.start < to }

    /** A programme's rest at [now], as hours and minutes ("1 h 5 min left"). */
    fun left(program: LiveProgram, now: Long): Pair<Int, Int> {
        val minutes = maxOf(1, Math.round((program.stop - now) / 60.0).toInt())
        return minutes / 60 to minutes % 60
    }

    private val LEADING_BARS = Regex("^\\|+\\s*")
    private val BRACKETED = Regex("^\\[([^\\]]{1,16})]\\s*(.+)$")
    private val BARRED = Regex("^([^|]{1,16})\\|\\s*(.+)$")
    private val COLON = Regex("^([A-Z]{1,4}):\\s*(.+)$")
    private val LEADING_MARKS = Regex("^[|:\\-\\s]+")
}

/**
 * The channel last watched: a list opens back at it, coming back from the
 * player. For as long as the app runs, as the web keeps it for the tab.
 */
object LastWatched {
    val channel = MutableStateFlow<Long?>(null)
}

/**
 * A category's channels a page at a time, as a list scrolls ([showing]):
 * its first page (which says how many there are) and the pages covering
 * the rows in view, so a lineup of tens of thousands costs what's on
 * screen.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ChannelPaging(
    private val live: LiveTvRepository,
    category: Flow<LiveCategory>,
    query: Flow<String>,
    scope: CoroutineScope,
) {
    private val rows = MutableStateFlow(0..0)

    val pages: StateFlow<ChannelPages> = combine(category.distinctUntilChanged(), query.map { it.trim() }.distinctUntilChanged(), ::Pair)
        .flatMapLatest { (c, q) ->
            rows.map { pagesFor(it) }.distinctUntilChanged().flatMapLatest { wanted ->
                combine(wanted.map { p -> live.page(c, q, p).map { p to it } }) { loads -> pagesOf(loads.toList()) }
            }
        }
        .stateIn(scope, SharingStarted.WhileSubscribed(LiveTvViewModel.STOP_AFTER_MS), ChannelPages())

    /** Rows [first] to [last] are in view (a few either side are asked for with them). Cheap: call it as the list scrolls. */
    fun showing(first: Int, last: Int) {
        rows.value = first.coerceAtLeast(0)..last.coerceAtLeast(first)
    }

    private fun pagesFor(range: IntRange): List<Int> {
        val from = (range.first - MARGIN).coerceAtLeast(0) / LiveTvRepository.PAGE
        val to = (range.last + MARGIN) / LiveTvRepository.PAGE
        return (setOf(0) + (from..to)).sorted()
    }

    private fun pagesOf(loads: List<Pair<Int, Loadable<LiveChannelPage>>>): ChannelPages {
        val first = loads.firstOrNull { it.first == 0 }?.second
        val total = first?.valueOrNull?.total
        // A page past the end (the list got shorter) shows nothing.
        val pages = loads.mapNotNull { (p, load) -> load.valueOrNull?.let { page -> p to page.channels } }.toMap()
        return ChannelPages(total, pages, (first as? Loadable.Failed)?.takeIf { total == null }?.error)
    }

    private companion object {
        /** Rows asked for either side of what shows. */
        const val MARGIN = 20
    }
}

/**
 * Where Live TV's changes run (a favourite set): to their end on the main
 * thread's loop, whatever happens to the screen that asked.
 */
internal val LiveWork: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
