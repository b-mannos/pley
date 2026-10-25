package app.pley.android.player

import android.content.Context
import app.pley.android.caps.CapsGraph
import app.pley.android.common.DeviceProfile
import app.pley.android.common.NotBuiltYet
import app.pley.android.data.DataGraph
import app.pley.android.network.NetworkGraph
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** Playback for the rest of the app: engines, sessions and music. */
interface PlayerGraph {
    val engines: PlayerEngine.Factory
    val services: PlaybackServices
    val music: MusicControl

    companion object {
        /** Not built yet: nothing plays, and saying so is all it does. */
        fun create(context: Context, data: DataGraph, network: NetworkGraph, caps: CapsGraph, profile: DeviceProfile): PlayerGraph =
            NotBuiltPlayer
    }
}

private object NotBuiltPlayer : PlayerGraph {
    override val engines = PlayerEngine.Factory { _, _, _ -> throw NotBuiltYet("The player") }

    override val services = object : PlaybackServices {
        override fun prepare(request: PlayRequest): String = "not-built:${request.itemId}"

        override fun controller(key: String): PlaybackController? = null
    }

    override val music = object : MusicControl {
        override val nowPlaying: StateFlow<NowPlaying?> = MutableStateFlow(null)

        override fun pauseForVideo() = Unit
    }
}
