package app.pley.android.data

import app.pley.android.common.NotBuiltYet
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveGuide
import app.pley.android.model.LiveProgram
import app.pley.android.model.LiveSummary
import app.pley.android.model.LiveWatching
import app.pley.android.model.WatchRequest
import java.util.concurrent.atomic.AtomicInteger
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer

/**
 * Live TV (docs/API.md, "Live TV"): whether to show it, the channels with
 * what's on, the guide, favourites, and watching a channel.
 */
interface LiveTvRepository {
    /** Whether Live TV is open to this profile and has channels, and what was watched last. */
    fun summary(): Flow<Loadable<LiveSummary>>

    /** The channels shown, by number, each with what's on now and next. */
    fun channels(): Flow<Loadable<List<LiveChannel>>>

    /**
     * The guide from [from] for [GUIDE_CHUNK_S] (unix seconds; [from] a
     * multiple of it, so the same chunk is asked for however it is
     * scrolled to).
     */
    fun guide(from: Long): Flow<Loadable<LiveGuide>>

    /** Asks for the channels and the summary again now: a programme ended, or the page came back. */
    fun refresh()

    /** A favourite channel or not: at once on screen, put back if the server refuses. */
    suspend fun setFavorite(channelId: Long, favorite: Boolean)

    /**
     * Starts watching [channelId] on this device, [clientId] ending what
     * it watched before. [LiveWatching.url] is absolute. Throws the
     * server's refusal (`409`: every connection of its source in use).
     */
    suspend fun watch(channelId: Long, clientId: String): LiveWatching

    /** Tells the server a paused player still watches [sessionId]. */
    suspend fun keepWatching(sessionId: String)

    /** Stops watching [sessionId], in the background: it outlives the screen that asks. */
    fun leave(sessionId: String)

    companion object {
        /**
         * The guide is fetched in chunks of two hours: a provider of 50,000
         * channels has some 30,000 programmes in two hours, 11 MB of JSON.
         */
        const val GUIDE_CHUNK_S = 2 * 3600L

        /** No Live TV: tests' graphs, which have none. */
        val None: LiveTvRepository = object : LiveTvRepository {
            override fun summary(): Flow<Loadable<LiveSummary>> = flowOf(Loadable.Ready(LiveSummary(), fresh = true))

            override fun channels(): Flow<Loadable<List<LiveChannel>>> = flowOf(Loadable.Ready(emptyList(), fresh = true))

            override fun guide(from: Long): Flow<Loadable<LiveGuide>> =
                flowOf(Loadable.Ready(LiveGuide(from, from + GUIDE_CHUNK_S), fresh = true))

            override fun refresh() = Unit

            override suspend fun setFavorite(channelId: Long, favorite: Boolean) = throw NotBuiltYet("Live TV")

            override suspend fun watch(channelId: Long, clientId: String): LiveWatching = throw NotBuiltYet("Live TV")

            override suspend fun keepWatching(sessionId: String) = Unit

            override fun leave(sessionId: String) = Unit
        }
    }
}

/**
 * Live TV through the cache: the summary and the channels are kept as
 * snapshots (the channels without what the guide says of their
 * programmes, which a big lineup makes megabytes of), so the entry to it
 * and the channels show at once. The guide is held in memory only, by
 * chunk: the [HELD_CHUNKS] used last keep their programmes, the others let
 * them go and ask again when shown, so scrolling through two days of a
 * big lineup's guide never holds it all.
 */
internal class CachedLiveTv(private val cache: ResourceCache) : LiveTvRepository {
    private val summary = Resource(SnapshotKeys.LIVE, LiveSummary.serializer()) { api, _ -> api.liveSummary() }

    private val channels = Resource(
        SnapshotKeys.LIVE_CHANNELS,
        ListSerializer(LiveChannel.serializer()),
        trim = { list -> list.map { c -> c.copy(now = c.now?.let(::brief), next = c.next?.let(::brief)) } },
    ) { api, _ -> api.liveChannels() }

    /** The chunks holding programmes, the one used last at the end. */
    private val held = ArrayDeque<GuideChunk>()

    override fun summary(): Flow<Loadable<LiveSummary>> = cache.observe(summary)

    override fun channels(): Flow<Loadable<List<LiveChannel>>> = cache.observe(channels)

    override fun guide(from: Long): Flow<Loadable<LiveGuide>> = cache.whileReady { profile ->
        flow {
            val key = SnapshotKeys.liveGuide(from)
            val chunk = cache.node(profile, key, detail = false) { GuideChunk(key, from, profile) }
            chunk.users.incrementAndGet()
            try {
                if (chunk.stale()) chunk.load()
                emitAll(chunk.state)
            } finally {
                chunk.users.decrementAndGet()
            }
        }
    }

    override fun refresh() = cache.revalidate { key, _ -> key == SnapshotKeys.LIVE || key == SnapshotKeys.LIVE_CHANNELS }

    override suspend fun setFavorite(channelId: Long, favorite: Boolean) {
        val patched = cache.patchKey(SnapshotKeys.LIVE_CHANNELS) { value ->
            @Suppress("UNCHECKED_CAST")
            val list = value as List<LiveChannel>
            if (list.none { it.id == channelId && it.favorite != favorite }) null else list.map { if (it.id == channelId) it.copy(favorite = favorite) else it }
        }
        undoing(patched) { cache.api(cache.profile()).setFavoriteChannel(channelId, favorite) }
    }

    override suspend fun watch(channelId: Long, clientId: String): LiveWatching {
        val api = cache.api(cache.profile())
        val watching = api.watchChannel(channelId, WatchRequest(clientId))
        // Recently watched leads with it now; the server says so too, next time it is asked.
        cache.patchKey(SnapshotKeys.LIVE) { value ->
            val live = value as LiveSummary
            if (live.recent.firstOrNull() == channelId) null else live.copy(recent = listOf(channelId) + live.recent.filter { it != channelId })
        }
        cache.markStale { key, _ -> key == SnapshotKeys.LIVE }
        return watching.copy(url = api.url(watching.url))
    }

    override suspend fun keepWatching(sessionId: String) = cache.api(cache.profile()).keepWatching(sessionId)

    override fun leave(sessionId: String) {
        cache.app.launch {
            try {
                cache.api(cache.profile()).leaveChannel(sessionId)
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
                // Out of reach: the server lets the session go by itself after 45 s.
            }
        }
    }

    /** [chunk] has programmes now: the eldest chunks nobody shows let theirs go. */
    private fun keep(chunk: GuideChunk) = synchronized(held) {
        held.remove(chunk)
        held.addLast(chunk)
        var over = held.size - HELD_CHUNKS
        for (old in held.filter { !it.active }) {
            if (over <= 0) break
            held.remove(old)
            old.drop()
            over--
        }
    }

    /**
     * One chunk of the guide, as the cache holds it (asked again on Live
     * TV's events, forgotten with the profile), its programmes let go by
     * [keep] when others were used since.
     */
    private inner class GuideChunk(override val key: String, private val from: Long, private val profile: ProfileScope) : CacheNode {
        val state = MutableStateFlow<Loadable<LiveGuide>>(Loadable.Loading)
        val users = AtomicInteger()
        private var job: Job? = null

        @Volatile
        override var fetchedAt: Long = ResourceCache.NEVER
            private set

        override val active: Boolean get() = users.get() > 0
        override val current: Any? get() = state.value.valueOrNull

        fun stale(): Boolean = fetchedAt == ResourceCache.NEVER || cache.clock() - fetchedAt >= GUIDE_STALE_MS

        fun load() = synchronized(this) {
            if (job?.isActive == true) return@synchronized
            job = cache.app.launch {
                try {
                    val guide = cache.api(profile).liveGuide(from, from + LiveTvRepository.GUIDE_CHUNK_S)
                    fetchedAt = cache.clock()
                    state.value = Loadable.Ready(guide, fresh = true)
                    keep(this@GuideChunk)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    state.update { Loadable.Failed(e, it.valueOrNull) }
                }
            }
        }

        /** Lets the programmes go: asked for again when shown. */
        fun drop() {
            fetchedAt = ResourceCache.NEVER
            state.value = Loadable.Loading
        }

        override fun patch(transform: (Any) -> Any?): (() -> Unit)? = null

        override fun refresh() {
            load()
        }

        override fun markStale() {
            fetchedAt = ResourceCache.NEVER
        }
    }

    private companion object {
        /** Chunks that keep their programmes: what a guide shows and either side of it, and a little more. */
        const val HELD_CHUNKS = 5

        /** A chunk shown again is asked for again when older than this: a source read again says so by an event. */
        const val GUIDE_STALE_MS = 10 * 60_000L

        /** A programme as the channels' snapshot keeps it: what the cards show. */
        fun brief(p: LiveProgram) = p.copy(description = null, categories = emptyList(), image = null)
    }
}
