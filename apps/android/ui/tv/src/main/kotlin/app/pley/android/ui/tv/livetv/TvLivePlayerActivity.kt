package app.pley.android.ui.tv.livetv

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.pley.android.player.PlayerEngine
import app.pley.android.player.engine.VideoMediaSession
import app.pley.android.player.live.LivePhase
import app.pley.android.screens.PleyGraphOwner
import app.pley.android.screens.livetv.LivePlayerViewModel
import app.pley.android.ui.tv.theme.TvTheme
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/**
 * The TV's live player, in a window of its own as the TV player is:
 * onStart tunes the channel (again, coming back from Home or the screen
 * saver) and starts the media session, so remotes' play and pause and
 * Alexa reach it; onStop lets the provider's connection, the player and
 * its decoders go. Nothing plays behind other apps; the screen stays on
 * only while it plays.
 */
class TvLivePlayerActivity : ComponentActivity() {
    private val vm: LivePlayerViewModel by viewModels {
        LivePlayerViewModel.factory(intent.getLongExtra(EXTRA_CHANNEL, 0))
    }
    private var attached: PlayerEngine? = null
    private var session: VideoMediaSession? = null
    private var screenOn = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val graph = (application as PleyGraphOwner).screensGraph
        setContent {
            TvTheme(tier = graph.device.tier) {
                TvLivePlayerScreen(vm = vm, onExit = ::finish)
            }
        }
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                combine(vm.engine, vm.state) { engine, state -> engine to state }.collect { (engine, state) ->
                    attach(engine)
                    keepScreenOn(state.playWhenReady && (state.phase == LivePhase.Playing || state.phase == LivePhase.Tuning))
                }
            }
        }
    }

    override fun onStart() {
        super.onStart()
        vm.resume()
    }

    override fun onStop() {
        detach()
        if (!isChangingConfigurations) {
            vm.stop()
            keepScreenOn(false)
        }
        super.onStop()
    }

    /** The engine, once there is one: this window for it, and the media session while started. */
    private fun attach(engine: PlayerEngine?) {
        if (attached === engine) return
        detach()
        if (engine == null || !lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) return
        attached = engine
        engine.attachWindow(window)
        session = VideoMediaSession(this, engine.player).also { it.start() }
    }

    private fun detach() {
        session?.stop()
        session = null
        attached?.detachWindow()
        attached = null
    }

    private fun keepScreenOn(on: Boolean) {
        if (on == screenOn) return
        screenOn = on
        if (on) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON) else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    companion object {
        /** The channel to tune as it opens. */
        const val EXTRA_CHANNEL = "channel"

        fun intent(context: Context, channelId: Long): Intent =
            Intent(context, TvLivePlayerActivity::class.java).putExtra(EXTRA_CHANNEL, channelId)
    }
}
