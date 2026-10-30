package app.pley.android.ui.phone.livetv

import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.pley.android.player.PlayerEngine
import app.pley.android.player.engine.VideoMediaSession
import app.pley.android.player.live.LivePhase
import app.pley.android.player.live.LiveSessionPlayer
import app.pley.android.screens.livetv.LivePlayerViewModel
import app.pley.android.ui.phone.theme.PhoneTheme
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/**
 * The phone and tablet live player, in a window of its own, as the
 * phone's player: immersive and black, landscape on phones, the screen
 * kept on while it plays. Live has no pause: the media session offers
 * none either ([LiveSessionPlayer]), and no picture in picture (the
 * web's). Leaving lets the channel's connection and the decoders go; back
 * to it, the channel is tuned again at the live edge.
 */
class LivePlayerActivity : ComponentActivity() {
    private val vm: LivePlayerViewModel by viewModels {
        LivePlayerViewModel.factory(intent.getLongExtra(EXTRA_CHANNEL, 0), intent.getStringExtra(EXTRA_CATEGORY) ?: "all")
    }
    private var attached: PlayerEngine? = null
    private var mediaSession: VideoMediaSession? = null
    private var screenOn = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        immersive()
        if (resources.configuration.smallestScreenWidthDp < TABLET_DP) {
            requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
        setContent {
            PhoneTheme {
                LivePlayerScreen(vm = vm, onBack = ::finish)
            }
        }
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                combine(vm.engine, vm.state) { engine, state -> engine to state }.collect { (engine, state) ->
                    attach(engine)
                    keepScreenOn(state.phase == LivePhase.Playing || state.phase == LivePhase.Tuning)
                }
            }
        }
    }

    /** Edge to edge, the system bars hidden until swiped in, the picture beside the camera cutout too. */
    private fun immersive() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) immersive()
    }

    override fun onStart() {
        super.onStart()
        vm.resume()
        immersive()
    }

    override fun onStop() {
        super.onStop()
        detach()
        // Out of sight, the connection goes.
        if (!isChangingConfigurations) vm.stop()
    }

    override fun onDestroy() {
        detach()
        super.onDestroy()
    }

    /** A keyboard's or a remote's channel keys and digits on a tablet, where the screen didn't take them. */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        when (keyCode) {
            KeyEvent.KEYCODE_CHANNEL_UP, KeyEvent.KEYCODE_PAGE_UP -> {
                vm.step(1)
                return true
            }
            KeyEvent.KEYCODE_CHANNEL_DOWN, KeyEvent.KEYCODE_PAGE_DOWN -> {
                vm.step(-1)
                return true
            }
            // Live has no pause, no going back: these do nothing here.
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_MEDIA_PLAY, KeyEvent.KEYCODE_MEDIA_PAUSE, KeyEvent.KEYCODE_SPACE,
            KeyEvent.KEYCODE_MEDIA_REWIND, KeyEvent.KEYCODE_MEDIA_FAST_FORWARD,
            -> return true
        }
        val digit = event.unicodeChar.toChar()
        if (digit.isDigit()) {
            vm.digit(digit)
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    /** The engine, once there is one: this window for it, and the media session while started. */
    private fun attach(engine: PlayerEngine?) {
        if (attached === engine) return
        detach()
        if (engine == null || !lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) return
        attached = engine
        engine.attachWindow(window)
        mediaSession = VideoMediaSession(this, LiveSessionPlayer(engine.player)).also { it.start() }
    }

    private fun detach() {
        mediaSession?.stop()
        mediaSession = null
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

        /** The category it was chosen in (a LiveCategory's key): channel up and down stay in it. */
        const val EXTRA_CATEGORY = "category"

        private const val TABLET_DP = 600

        fun intent(context: Context, channelId: Long, category: String): Intent =
            Intent(context, LivePlayerActivity::class.java).putExtra(EXTRA_CHANNEL, channelId).putExtra(EXTRA_CATEGORY, category)
    }
}
