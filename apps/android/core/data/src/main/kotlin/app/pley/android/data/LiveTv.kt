package app.pley.android.data

import app.pley.android.common.NotBuiltYet
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveGuide
import app.pley.android.model.LiveSummary
import app.pley.android.model.LiveWatching
import app.pley.android.model.WatchRequest
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
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
        /** The guide is fetched in chunks of six hours. */
        const val GUIDE_CHUNK_S = 6 * 3600L

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
 * snapshots, so the entry to it and the channels show at once; the guide
 * is held in memory only, by chunk, the least recently used going first.
 */
internal class CachedLiveTv(private val cache: ResourceCache) : LiveTvRepository {
    private val summary = Resource(SnapshotKeys.LIVE, LiveSummary.serializer()) { api, _ -> api.liveSummary() }

    private val channels = Resource(SnapshotKeys.LIVE_CHANNELS, ListSerializer(LiveChannel.serializer())) { api, _ -> api.liveChannels() }

    private fun guideOf(from: Long) =
        Resource<LiveGuide>(SnapshotKeys.liveGuide(from), serializer = null, detail = true) { api, _ ->
            api.liveGuide(from, from + LiveTvRepository.GUIDE_CHUNK_S)
        }

    override fun summary(): Flow<Loadable<LiveSummary>> = cache.observe(summary)

    override fun channels(): Flow<Loadable<List<LiveChannel>>> = cache.observe(channels)

    override fun guide(from: Long): Flow<Loadable<LiveGuide>> = cache.observe(guideOf(from))

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
}
