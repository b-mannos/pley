package app.pley.android.player

import android.content.Context
import app.pley.android.caps.CapsGraph
import app.pley.android.common.DeviceProfile
import app.pley.android.data.DataGraph
import app.pley.android.network.NetworkGraph
import app.pley.android.player.engine.Media3EngineFactory
import app.pley.android.player.session.SessionPlaybackServices
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** Playback for the rest of the app: engines, sessions and music. */
interface PlayerGraph {
    val engines: PlayerEngine.Factory
    val services: PlaybackServices
    val music: MusicControl

    companion object {
        /**
         * Media3 engines by device, and server sessions over them for the
         * server and profile signed in to now. Music in the background comes
         * with the music screens.
         */
        fun create(context: Context, data: DataGraph, network: NetworkGraph, caps: CapsGraph, profile: DeviceProfile): PlayerGraph {
            val engines = Media3EngineFactory.of(caps, network)
            val services = SessionPlaybackServices.create(context, data, network, caps, profile, engines)
            return AppPlayer(engines, services)
        }
    }
}

private class AppPlayer(
    override val engines: PlayerEngine.Factory,
    override val services: PlaybackServices,
) : PlayerGraph {
    override val music = object : MusicControl {
        override val nowPlaying: StateFlow<NowPlaying?> = MutableStateFlow(null)

        override fun pauseForVideo() = Unit
    }
}
