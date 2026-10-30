package app.pley.android.screens.livetv

import app.pley.android.data.LiveTvRepository
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveGuide
import app.pley.android.model.LiveProgram
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

/**
 * Which channels a Live TV page lists: every one, the favourites, or one
 * group's (the web's chips, apps/web/src/pages/livetv/common.tsx).
 */
sealed interface ChannelFilter {
    /** How it is saved: "all", "favorites" or "group:News". */
    val key: String

    data object All : ChannelFilter {
        override val key: String get() = "all"
    }

    data object Favorites : ChannelFilter {
        override val key: String get() = "favorites"
    }

    data class Group(val name: String) : ChannelFilter {
        override val key: String get() = "group:$name"
    }

    companion object {
        fun parse(key: String?): ChannelFilter = when {
            key == "favorites" -> Favorites
            key != null && key.startsWith("group:") -> Group(key.removePrefix("group:"))
            else -> All
        }
    }
}

/** A group of channels, and how many it has. */
data class ChannelGroup(val name: String, val count: Int)

/** How a Live TV page shows the channels: the guide (channels by time) or cards. */
enum class LiveView { Guide, Channels }

/** Live TV's rules, shared by its pages and its player, the web's (apps/web/src/pages/livetv). */
object LiveTv {
    /** The guide's columns: half an hour each. */
    const val SLOT_S = 30 * 60L

    /** How far ahead the guide goes from where it starts. */
    const val GUIDE_SPAN_S = 48 * 3600L

    /** At most this many channels in Recently watched. */
    const val RECENT = 6

    /** The groups [channels] are in, in their order, with how many each has. */
    fun groups(channels: List<LiveChannel>): List<ChannelGroup> {
        val counts = LinkedHashMap<String, Int>()
        for (c in channels) if (c.group.isNotEmpty()) counts[c.group] = (counts[c.group] ?: 0) + 1
        return counts.map { (name, count) -> ChannelGroup(name, count) }
    }

    /** [filter], or All when what it names is gone (a group emptied, the last favourite taken off). */
    fun effective(filter: ChannelFilter, groups: List<ChannelGroup>, favorites: Int): ChannelFilter = when (filter) {
        is ChannelFilter.Group -> if (groups.any { it.name == filter.name }) filter else ChannelFilter.All
        ChannelFilter.Favorites -> if (favorites > 0) filter else ChannelFilter.All
        ChannelFilter.All -> filter
    }

    /** The channels that pass [filter] and match [query]: a name, a number (or its start), or what's on now. */
    fun filtered(channels: List<LiveChannel>, filter: ChannelFilter, query: String): List<LiveChannel> {
        val q = query.trim().lowercase()
        return channels.filter { c ->
            val passes = when (filter) {
                ChannelFilter.All -> true
                ChannelFilter.Favorites -> c.favorite
                is ChannelFilter.Group -> c.group == filter.name
            }
            passes && (
                q.isEmpty() ||
                    c.name.lowercase().contains(q) ||
                    c.number?.startsWith(q) == true ||
                    c.now?.title?.lowercase()?.contains(q) == true
                )
        }
    }

    /** The channels of [ids] (the summary's recent, newest first) still in [channels], [RECENT] at most. */
    fun recent(channels: List<LiveChannel>, ids: List<Long>): List<LiveChannel> {
        val byId = channels.associateBy { it.id }
        return ids.mapNotNull(byId::get).take(RECENT)
    }

    /** The channel [by] places from [currentId] in [channels], round the ends; `null` when it isn't there. */
    fun step(channels: List<LiveChannel>, currentId: Long?, by: Int): LiveChannel? {
        if (channels.isEmpty()) return null
        val index = channels.indexOfFirst { it.id == currentId }
        if (index < 0) return null
        return channels[Math.floorMod(index + by, channels.size)]
    }

    /** The channel numbered [digits] exactly. */
    fun numbered(channels: List<LiveChannel>, digits: String): LiveChannel? = channels.firstOrNull { it.number == digits }

    /** Where the guide starts for [now] (unix seconds): the start of the half hour before the one now is in. */
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

    /** "12 min left", "1 h 5 min left": as numbers, [hours] and [minutes] of a programme's rest at [now]. */
    fun left(program: LiveProgram, now: Long): Pair<Int, Int> {
        val minutes = maxOf(1, ((program.stop - now) / 60.0).let { Math.round(it).toInt() })
        return minutes / 60 to minutes % 60
    }
}

/**
 * Where Live TV's changes run (a favourite set): to their end on the main
 * thread's loop, whatever happens to the screen that asked.
 */
internal val LiveWork: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
