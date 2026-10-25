package app.pley.android.ui.tv.shell

import android.app.Application
import app.pley.android.auth.AuthGraph
import app.pley.android.caps.CapsGraph
import app.pley.android.common.DeviceProfile
import app.pley.android.common.FormFactor
import app.pley.android.common.Tier
import app.pley.android.data.AppSettings
import app.pley.android.data.CollectionsRepository
import app.pley.android.data.DataGraph
import app.pley.android.data.HistoryRepository
import app.pley.android.data.LibraryItemsRepository
import app.pley.android.data.LiveUpdates
import app.pley.android.data.Loadable
import app.pley.android.data.PeopleRepository
import app.pley.android.data.PlaylistsRepository
import app.pley.android.data.ScanState
import app.pley.android.data.SearchRepository
import app.pley.android.data.ServersRepository
import app.pley.android.data.Session
import app.pley.android.data.SessionRepository
import app.pley.android.data.SessionState
import app.pley.android.data.SnapshotStore
import app.pley.android.data.TrackDefaultsRepository
import app.pley.android.model.Hubs
import app.pley.android.player.PlayerGraph
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.ScreensGraph
import app.pley.android.testing.Fixtures
import app.pley.android.ui.tv.home.TestHomeSources
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * The app's graph for the TV frame's tests: Home's data from the JSON
 * fixtures (Continue Watching starts with Severance's "Half Loop"), every
 * other part failing when asked, as the frame never asks for it.
 */
class TvTestGraph : ScreensGraph {
    val sources = TestHomeSources().apply {
        home.value = Loadable.Ready(Fixtures.decode<Hubs>("hubs/home.json"), fresh = false)
    }

    override val device = DeviceProfile(FormFactor.Tv, fireTv = false, tier = Tier.T2, model = "test", totalRamBytes = 4L shl 30, lowRam = false)
    override val auth: AuthGraph get() = unused("auth")
    override val caps: CapsGraph get() = unused("caps")
    override val player: PlayerGraph get() = unused("player")
    override val playerLauncher: PlayerLauncher get() = sources.launcher

    override val data: DataGraph = object : DataGraph {
        override val session = object : SessionRepository {
            override val session: StateFlow<Session?> = sources.sessionFlow
            override val state: StateFlow<SessionState> = MutableStateFlow(SessionState.Starting)
            override suspend fun signOut() = Unit
        }
        override val libraries = sources.libraries
        override val hubs = sources.hubs
        override val items = sources.items
        override val playItem = sources.playItem
        override val watchState = sources.watchState
        override val liveUpdates = object : LiveUpdates {
            override val scans: StateFlow<List<ScanState>> = sources.scans
        }
        override val servers: ServersRepository get() = unused("servers")
        override val snapshots: SnapshotStore get() = unused("snapshots")
        override val libraryItems: LibraryItemsRepository get() = unused("libraryItems")
        override val search: SearchRepository get() = unused("search")
        override val people: PeopleRepository get() = unused("people")
        override val collections: CollectionsRepository get() = unused("collections")
        override val playlists: PlaylistsRepository get() = unused("playlists")
        override val history: HistoryRepository get() = unused("history")
        override val trackDefaults: TrackDefaultsRepository get() = unused("trackDefaults")
        override val settings: AppSettings get() = unused("settings")
    }

    private fun unused(what: String): Nothing = error("the TV frame's tests don't use $what")
}

/** The tests' Application: screens' ViewModels are made from its graph, as from the app's. */
class TvTestApplication : Application(), PleyGraphOwner {
    override val screensGraph: ScreensGraph = TvTestGraph()
}
