package app.pley.android

import android.app.Application
import app.pley.android.auth.AuthGraph
import app.pley.android.caps.CapsGraph
import app.pley.android.common.DeviceProfile
import app.pley.android.common.UserAgent
import app.pley.android.data.DataGraph
import app.pley.android.network.HttpRoot
import app.pley.android.network.NetworkGraph
import app.pley.android.player.PlayerGraph
import app.pley.android.screens.PlayerLauncher
import app.pley.android.screens.ScreensGraph
import okhttp3.OkHttpClient

/**
 * The app's parts and how they connect, by hand (no injection framework).
 * Everything is made the first time it's asked for, so starting the app
 * builds nothing: the first frame never waits for a client, the Keystore
 * or the capability probe.
 */
class AppGraph(private val app: Application) : ScreensGraph {
    override val device: DeviceProfile by lazy { DeviceProfile.of(app) }
    val userAgent: String by lazy { UserAgent.of(BuildConfig.VERSION_NAME) }
    val root: OkHttpClient by lazy { HttpRoot.create(userAgent, device) }
    override val auth: AuthGraph by lazy { AuthGraph.create(app, root, userAgent) }
    override val network: NetworkGraph by lazy { NetworkGraph.create(app, root, auth, device, app.cacheDir) }
    override val data: DataGraph by lazy { DataGraph.create(app, network, auth, device) }
    override val caps: CapsGraph by lazy { CapsGraph.create(app, device) }
    override val player: PlayerGraph by lazy { PlayerGraph.create(app, data, network, caps, device) }
    override val playerLauncher: PlayerLauncher by lazy { PlayerLauncherImpl(app, { player }, device) }
}
