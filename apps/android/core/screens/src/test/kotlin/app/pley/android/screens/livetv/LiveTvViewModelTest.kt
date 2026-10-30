package app.pley.android.screens.livetv

import android.os.Looper
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.viewmodel.CreationExtras
import app.pley.android.data.Loadable
import app.pley.android.model.LiveCategory
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveGuide
import app.pley.android.model.LiveProgram
import app.pley.android.network.ApiException
import app.pley.android.player.PlayRequest
import app.pley.android.player.live.LivePlayback
import app.pley.android.screens.PlayerLauncher
import app.pley.android.testing.MainDispatcherRule
import app.pley.android.testing.data.FakeLiveTv
import app.pley.android.testing.player.engine.FakePlayerEngine
import kotlin.reflect.KClass
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf

/** The Live TV page and the live player's view models. */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class LiveTvViewModelTest {
    @get:Rule
    val main = MainDispatcherRule(StandardTestDispatcher())

    /** Holds the view models made, cleared after each test: their tickers end with them. */
    private val store = ViewModelStore()

    @After
    fun clear() {
        store.clear()
        LastWatched.channel.value = null
    }

    /**
     * A test on the main dispatcher, its view models cleared before it ends:
     * runTest runs what is due until idle, and their tickers never are.
     */
    private fun liveTest(block: suspend TestScope.() -> Unit) = runTest(main.dispatcher) {
        try {
            block()
        } finally {
            store.clear()
        }
    }

    private fun <T : ViewModel> held(make: () -> T): T {
        val factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <V : ViewModel> create(modelClass: KClass<V>, extras: CreationExtras): V = make() as V
        }
        return ViewModelProvider.create(store, factory)["vm${store.keys().size}", ViewModel::class] as T
    }

    /** The fixtures' three channels, then 200 more: three pages. */
    private val live = FakeLiveTv().apply {
        lineup.value = lineup.value + (1..200).map { LiveChannel(1000L + it, name = "Filler $it", number = "${1000 + it}", group = "Filler") }
    }
    private val watched = mutableListOf<Pair<Long, String>>()
    private val launcher = object : PlayerLauncher {
        override fun play(request: PlayRequest) = Unit

        override fun watchLive(channelId: Long, category: String) {
            watched += channelId to category
        }
    }

    /** 15:40 UTC, while the fixtures' briefing is on. */
    private var clock = 1_761_752_400_000L

    private fun TestScope.page(saved: SavedStateHandle = SavedStateHandle()): LiveTvViewModel {
        val vm = held { LiveTvViewModel(LiveTvDeps(live, launcher, flowOf("http://nas:7539"), clock = { clock }, compute = main.dispatcher, work = backgroundScope), saved) }
        backgroundScope.launch(main.dispatcher) { vm.state.collect { } }
        backgroundScope.launch(main.dispatcher) { vm.channels.collect { } }
        backgroundScope.launch(main.dispatcher) { vm.guide.collect { } }
        backgroundScope.launch(main.dispatcher) { vm.landing.collect { } }
        runCurrent()
        return vm
    }

    @Test
    fun `the page lists a category's channels a page at a time`() = liveTest {
        val vm = page()
        assertEquals(5, vm.state.value.groups?.groups?.size)
        // The first page says how many there are.
        assertEquals(203, vm.channels.value.total)
        assertEquals(100, vm.channels.value.loaded.size)
        assertNull(vm.channels.value[150])
        vm.showingRows(150, 160)
        runCurrent()
        assertEquals(1148L, vm.channels.value[150]?.id)

        vm.setCategory(LiveCategory.Group("News"))
        vm.setQuery("weather")
        runCurrent()
        assertEquals(listOf(5L), vm.channels.value.loaded.map { it.id })
        assertEquals(1, vm.channels.value.total)
    }

    @Test
    fun `the view and the category are kept with the page`() = liveTest {
        val saved = SavedStateHandle()
        val vm = page(saved)
        vm.setView(LiveView.Channels)
        vm.setCategory(LiveCategory.Favorites)
        runCurrent()
        val again = page(saved)
        assertEquals(LiveView.Channels, again.state.value.view)
        assertEquals(LiveCategory.Favorites, again.state.value.category)
    }

    @Test
    fun `the list opens at the channel last watched, when it is in the category`() = liveTest {
        LastWatched.channel.value = 5
        val vm = page()
        assertEquals(Landing(LiveCategory.All, "", 2), vm.landing.value)
        vm.setCategory(LiveCategory.Favorites)
        runCurrent()
        assertEquals(Landing(LiveCategory.Favorites, "", null), vm.landing.value)
    }

    @Test
    fun `a channel opens the live player in its category, and a refused favourite is told`() = liveTest {
        val vm = page()
        vm.setCategory(LiveCategory.Group("News"))
        runCurrent()
        vm.watch(vm.channels.value[0]!!)
        assertEquals(listOf(1L to "group:News"), watched)

        val events = mutableListOf<LiveTvEvent>()
        backgroundScope.launch(main.dispatcher) { vm.events.collect { events += it } }
        live.favoriteFails = ApiException(500, "internal", "no")
        vm.toggleFavorite(vm.channels.value[0]!!)
        runCurrent()
        assertEquals(listOf("favorite 1 true"), live.sent.filter { it.startsWith("favorite") })
        assertTrue(events.single() is LiveTvEvent.Failed)
    }

    @Test
    fun `the guide asks for the programmes of the pages in view, chunk by chunk`() = liveTest {
        val vm = page()
        vm.showingTimes(vm.guideStart, vm.guideStart + 3600)
        runCurrent()
        val asked = live.sent.filter { it.startsWith("guide") }
        // The first page's hundred channels, for each chunk asked.
        assertTrue(asked.isNotEmpty() && asked.all { it.substringAfter("[").split(", ").size == 100 })
        assertFalse(vm.guide.value.loaded)
        for (c in live.guides.keys) {
            live.guideChunk(c).value = Loadable.Ready(LiveGuide(c, c + 1, mapOf(1L to listOf(LiveProgram(1, c, c + 1800, "At $c")))), fresh = true)
        }
        runCurrent()
        assertTrue(vm.guide.value.loaded)
        assertEquals(live.guides.keys.sorted().map { "At $it" }, vm.guide.value.byChannel.getValue(1).map { it.title })
    }

    // ---------------------------------------------------------------- the live player

    private val engines = mutableListOf<FakePlayerEngine>()

    private fun TestScope.player(channelId: Long, category: LiveCategory = LiveCategory.All, saved: SavedStateHandle = SavedStateHandle()): LivePlayerViewModel {
        val deps = LivePlayerDeps(
            live = live,
            playback = { scope: CoroutineScope -> LivePlayback(live, { FakePlayerEngine().also { engines += it } }, { "android-1" }, scope) },
            origin = flowOf("http://nas:7539"),
            clock = { clock },
            work = backgroundScope,
        )
        val vm = held { LivePlayerViewModel(deps, saved, channelId, category.key) }
        settle()
        return vm
    }

    private fun TestScope.settle() {
        runCurrent()
        shadowOf(Looper.getMainLooper()).idle()
        runCurrent()
    }

    private fun lastWatch() = live.sent.last { it.startsWith("watch") }

    @Test
    fun `channel up and down go through the category's pages, round its ends`() = liveTest {
        val vm = player(1)
        assertEquals("watch 1 android-1", lastWatch())
        assertEquals("Pley News HD", vm.channel.value?.name)
        assertEquals(1L, LastWatched.channel.value)
        vm.step(1)
        settle()
        assertEquals("watch 2 android-1", lastWatch())
        // Round the start: the last page's last.
        vm.step(-1)
        settle()
        vm.step(-1)
        settle()
        assertEquals("watch 1200 android-1", lastWatch())
        // Round the end: the first page's first.
        vm.step(1)
        settle()
        assertEquals("watch 1 android-1", lastWatch())
        // From the first page's last to the second's first.
        vm.tune(1097)
        settle()
        vm.step(1)
        settle()
        assertEquals("watch 1098 android-1", lastWatch())
    }

    @Test
    fun `in a group, channel up stays in it`() = liveTest {
        player(1, LiveCategory.Group("News")).step(1)
        settle()
        assertEquals("watch 5 android-1", lastWatch())
    }

    @Test
    fun `Recently watched keeps the order it had as the player opened`() = liveTest {
        val vm = player(2, LiveCategory.Recent)
        vm.step(1)
        settle()
        assertEquals("watch 1 android-1", lastWatch())
        // Watching changed the server's order; zapping keeps the first.
        live.recent.value = listOf(1, 2)
        vm.step(1)
        settle()
        assertEquals("watch 2 android-1", lastWatch())
    }

    @Test
    fun `a typed number tunes once the digits stop, or says there is no such channel`() = liveTest {
        val vm = player(1, LiveCategory.Group("Sport"))
        vm.digit('1')
        vm.digit('0')
        vm.digit('5')
        assertEquals("105", vm.digits.value.typed)
        advanceTimeBy(LivePlayerViewModel.DIGITS_WAIT_MS + 1)
        settle()
        assertEquals("watch 5 android-1", lastWatch())

        vm.digit('9')
        advanceTimeBy(LivePlayerViewModel.DIGITS_WAIT_MS + 1)
        settle()
        assertEquals("9", vm.digits.value.notFound)
        advanceTimeBy(LivePlayerViewModel.NOT_FOUND_MS + 1)
        assertEquals(ChannelDigits(), vm.digits.value)
    }

    @Test
    fun `the player comes back on the channel it was on, in its category`() = liveTest {
        val saved = SavedStateHandle()
        val vm = player(1, LiveCategory.Group("News"), saved)
        vm.tune(5)
        settle()
        val again = player(1, LiveCategory.All, saved)
        assertEquals("watch 5 android-1", lastWatch())
        assertEquals(LiveCategory.Group("News"), again.category)
        assertNull(again.digits.value.notFound)
    }

    @Test
    fun `the channel list opens on the channel playing`() = liveTest {
        val vm = player(5)
        backgroundScope.launch(main.dispatcher) { vm.listIndex.collect { } }
        backgroundScope.launch(main.dispatcher) { vm.list.collect { } }
        settle()
        assertEquals(2, vm.listIndex.value)
        vm.showingListRows(0, 3)
        settle()
        assertEquals(listOf(1L, 2L, 5L), vm.list.value.loaded.take(3).map(LiveChannel::id))
    }
}
