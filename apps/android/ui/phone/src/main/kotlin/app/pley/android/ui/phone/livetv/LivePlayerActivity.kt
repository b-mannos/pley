package app.pley.android.ui.phone.livetv

import android.app.AppOpsManager
import android.app.PendingIntent
import android.app.PictureInPictureParams
import android.app.RemoteAction
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.drawable.Icon
import android.os.Build
import android.os.Bundle
import android.os.Process
import android.util.Rational
import android.view.KeyEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.runtime.mutableStateOf
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.pley.android.player.PlayerEngine
import app.pley.android.player.engine.VideoMediaSession
import app.pley.android.player.live.LivePhase
import app.pley.android.screens.livetv.LivePlayerViewModel
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.theme.PhoneTheme
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch

/**
 * The phone and tablet live player, in a window of its own, as the
 * phone's player: immersive and black, landscape on phones, the screen
 * kept on while it plays, and picture in picture when leaving while it
 * plays (by itself from Android 12; Home before), with play and pause.
 * Leaving otherwise lets the channel's connection and the decoders go;
 * back to it, the channel is tuned again at the live edge.
 */
class LivePlayerActivity : ComponentActivity() {
    private val vm: LivePlayerViewModel by viewModels {
        LivePlayerViewModel.factory(intent.getLongExtra(EXTRA_CHANNEL, 0))
    }
    private val inPip = mutableStateOf(false)
    private var attached: PlayerEngine? = null
    private var mediaSession: VideoMediaSession? = null
    private var screenOn = false
    private var pipPlaying: Boolean? = null

    private val pipReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action == ACTION_PLAY_PAUSE) vm.togglePlay()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        immersive()
        if (resources.configuration.smallestScreenWidthDp < TABLET_DP) {
            requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        }
        ContextCompat.registerReceiver(this, pipReceiver, IntentFilter(ACTION_PLAY_PAUSE), ContextCompat.RECEIVER_NOT_EXPORTED)
        setContent {
            PhoneTheme {
                LivePlayerScreen(vm = vm, inPip = inPip.value, onBack = ::finish)
            }
        }
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                combine(vm.engine, vm.state) { engine, state -> engine to state }.collect { (engine, state) ->
                    attach(engine)
                    val playing = state.playWhenReady && state.phase !is LivePhase.Failed
                    keepScreenOn(playing && state.phase != LivePhase.Idle)
                    updatePip(playing && state.phase == LivePhase.Playing)
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
        // In picture in picture the screen turned off: it is still watched. Otherwise the connection goes.
        if (!isChangingConfigurations && !isInPictureInPictureMode) vm.stop()
    }

    override fun onDestroy() {
        runCatching { unregisterReceiver(pipReceiver) }
        detach()
        super.onDestroy()
    }

    /** A keyboard's or a remote's channel keys and digits on a tablet. */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (event.action == KeyEvent.ACTION_DOWN) {
            when (event.keyCode) {
                KeyEvent.KEYCODE_CHANNEL_UP, KeyEvent.KEYCODE_PAGE_UP -> {
                    vm.step(1)
                    return true
                }
                KeyEvent.KEYCODE_CHANNEL_DOWN, KeyEvent.KEYCODE_PAGE_DOWN -> {
                    vm.step(-1)
                    return true
                }
                KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE, KeyEvent.KEYCODE_SPACE -> {
                    vm.togglePlay()
                    return true
                }
            }
            val digit = event.unicodeChar.toChar()
            if (digit.isDigit() && currentFocus !is android.widget.EditText) {
                vm.digit(digit)
                return true
            }
        }
        return super.dispatchKeyEvent(event)
    }

    /** The engine, once there is one: this window for it, and the media session while started. */
    private fun attach(engine: PlayerEngine?) {
        if (attached === engine) return
        detach()
        if (engine == null || !lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) return
        attached = engine
        engine.attachWindow(window)
        mediaSession = VideoMediaSession(this, engine.player).also { it.start() }
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

    // ---------------------------------------------------------------- picture in picture

    private val pipSupported: Boolean by lazy {
        if (!packageManager.hasSystemFeature(PackageManager.FEATURE_PICTURE_IN_PICTURE)) return@lazy false
        val ops = getSystemService(AppOpsManager::class.java) ?: return@lazy true
        @Suppress("DEPRECATION")
        val mode = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            ops.unsafeCheckOpNoThrow(AppOpsManager.OPSTR_PICTURE_IN_PICTURE, Process.myUid(), packageName)
        } else {
            ops.checkOpNoThrow(AppOpsManager.OPSTR_PICTURE_IN_PICTURE, Process.myUid(), packageName)
        }
        mode == AppOpsManager.MODE_ALLOWED
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        // Android 12 and later enter by themselves (setAutoEnterEnabled).
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S && pipPlaying == true && pipSupported) {
            runCatching { enterPictureInPictureMode(pipParams(true)) }
        }
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        inPip.value = isInPictureInPictureMode
        // Left picture in picture while stopped: its window was closed, and watching ends.
        if (!isInPictureInPictureMode && lifecycle.currentState == Lifecycle.State.CREATED) {
            vm.stop()
            finish()
        }
    }

    /** Picture in picture's settings, set again only when playing changes. */
    private fun updatePip(playing: Boolean) {
        if (!pipSupported || pipPlaying == playing) return
        pipPlaying = playing
        runCatching { setPictureInPictureParams(pipParams(playing)) }
    }

    private fun pipParams(playing: Boolean): PictureInPictureParams {
        val builder = PictureInPictureParams.Builder()
            .setAspectRatio(Rational(16, 9))
            .setActions(listOf(playPause(playing)))
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            // Only while playing: a paused channel doesn't shrink into a corner.
            builder.setAutoEnterEnabled(playing)
            builder.setSeamlessResizeEnabled(false)
        }
        return builder.build()
    }

    /** Pause while [playing], Play otherwise; this screen's receiver does it. */
    private fun playPause(playing: Boolean): RemoteAction {
        val title = getString(if (playing) R.string.live_player_pause else R.string.live_player_play)
        val icon = Icon.createWithResource(this, if (playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play)
        val intent = Intent(ACTION_PLAY_PAUSE).setPackage(packageName)
        val pending = PendingIntent.getBroadcast(this, REQUEST_PLAY_PAUSE, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        return RemoteAction(icon, title, title, pending)
    }

    companion object {
        /** The channel to tune as it opens. */
        const val EXTRA_CHANNEL = "channel"

        private const val TABLET_DP = 600
        private const val REQUEST_PLAY_PAUSE = 2
        internal const val ACTION_PLAY_PAUSE = "app.pley.android.live.PLAY_PAUSE"

        fun intent(context: Context, channelId: Long): Intent =
            Intent(context, LivePlayerActivity::class.java).putExtra(EXTRA_CHANNEL, channelId)
    }
}
