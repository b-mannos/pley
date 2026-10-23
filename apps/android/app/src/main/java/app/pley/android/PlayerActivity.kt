package app.pley.android

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.CookieManager
import android.widget.FrameLayout
import android.widget.TextView
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.annotation.OptIn
import androidx.core.net.toUri
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.lifecycleScope
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Plays a video with Media3, which plays more files as they are than the
 * web view (Matroska, HEVC, AC-3 and more, as the device decodes them). The
 * server decides how (direct play or HLS) from what this device decodes;
 * a file the device turns out not to decode is asked for again as a
 * transcode. Progress goes to the server as the web player reports it, and
 * the next episode follows at the end.
 */
@OptIn(UnstableApi::class)
class PlayerActivity : ComponentActivity() {
    private lateinit var server: String
    private lateinit var api: PleyApi
    private lateinit var view: PlayerView
    private lateinit var title: TextView
    private var player: ExoPlayer? = null
    private var request: PlayRequest? = null
    private var playback: Playback? = null
    private var transcodeAsked = false
    private var reporter: Job? = null
    private var token: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        server = intent.getStringExtra(EXTRA_SERVER) ?: return finish()
        val first = intent.getStringExtra(EXTRA_REQUEST)?.let(PlayRequest::parse) ?: return finish()
        token = Links.token(CookieManager.getInstance().getCookie(server))
        api = PleyApi(server, token)

        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }

        view = PlayerView(this).apply {
            setBackgroundColor(0xFF000000.toInt())
            setShowSubtitleButton(true)
            setShowNextButton(false)
            setShowPreviousButton(false)
        }
        title = TextView(this).apply {
            setTextColor(0xFFFFFFFF.toInt())
            textSize = 18f
            setPadding(48, 36, 48, 36)
            setShadowLayer(8f, 0f, 2f, 0xCC000000.toInt())
        }
        view.setControllerVisibilityListener(PlayerView.ControllerVisibilityListener { visibility ->
            title.visibility = visibility
        })
        val root = FrameLayout(this)
        root.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(
            title,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.TOP or Gravity.START),
        )
        setContentView(root)
        start(first, mode = "auto", startMs = first.startMs)
    }

    private fun newPlayer(): ExoPlayer {
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent("pleyAndroid/${BuildConfig.VERSION_NAME}")
            .setAllowCrossProtocolRedirects(true)
            .setDefaultRequestProperties(token?.let { mapOf("Authorization" to "Bearer $it") } ?: emptyMap())
        val exo = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(this).setDataSourceFactory(http))
            .build()
        exo.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED) finished()
            }

            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (!isPlaying && exo.playbackState == Player.STATE_READY) report("paused")
            }

            override fun onPlayerError(error: PlaybackException) {
                failed(error)
            }
        })
        view.player = exo
        return exo
    }

    /** Asks the server for `next` and plays it from `startMs`. */
    private fun start(next: PlayRequest, mode: String, startMs: Long) {
        val previous = playback
        playback = null
        request = next
        title.text = next.title
        lifecycleScope.launch {
            if (previous != null) end(previous, player?.currentPosition ?: 0)
            try {
                val caps = DeviceCaps.detect(this@PlayerActivity)
                val answer = api.startPlayback(Playback.body(next, caps, clientId(this@PlayerActivity), mode, startMs))
                playback = answer
                val exo = player ?: newPlayer().also { player = it }
                exo.setMediaItem(mediaItem(answer, next))
                exo.seekTo(startMs)
                exo.prepare()
                exo.play()
                startReporting()
            } catch (e: Exception) {
                Toast.makeText(this@PlayerActivity, "Can't play this: ${describe(e)}", Toast.LENGTH_LONG).show()
                finish()
            }
        }
    }

    private fun mediaItem(answer: Playback, next: PlayRequest): MediaItem {
        val base = server.trimEnd('/')
        // Direct play reads the file's own subtitle tracks; sidecar files and
        // HLS streams get theirs from the server, as WebVTT.
        val subtitles = answer.subtitles
            .filter { it.url != null && (answer.hls || it.external) }
            .map { s ->
                val flags = (if (s.index == next.sub || s.default) C.SELECTION_FLAG_DEFAULT else 0) or
                    (if (s.forced) C.SELECTION_FLAG_FORCED else 0)
                MediaItem.SubtitleConfiguration.Builder("$base${s.url}".toUri())
                    .setMimeType(MimeTypes.TEXT_VTT)
                    .setLanguage(s.language)
                    .setLabel(s.title ?: s.language)
                    .setSelectionFlags(flags)
                    .build()
            }
        return MediaItem.Builder()
            .setUri("$base${answer.url}")
            .setMimeType(if (answer.hls) MimeTypes.APPLICATION_M3U8 else null)
            .setMediaMetadata(MediaMetadata.Builder().setTitle(next.title).build())
            .setSubtitleConfigurations(subtitles)
            .build()
    }

    private fun startReporting() {
        reporter?.cancel()
        reporter = lifecycleScope.launch {
            while (isActive) {
                delay(REPORT_EVERY_MS)
                if (player?.isPlaying == true) report("playing")
            }
        }
    }

    private fun report(state: String) {
        val p = playback ?: return
        val r = request ?: return
        val exo = player ?: return
        val position = exo.currentPosition
        val duration = exo.duration.takeIf { it != C.TIME_UNSET } ?: p.durationMs
        background.launch { runCatching { api.progress(r.itemId, position, duration, state, p.sessionId) } }
    }

    /** Reports the end of a session and lets the server free its transcode. */
    private suspend fun end(p: Playback, positionMs: Long) {
        val r = request ?: return
        runCatching { api.progress(r.itemId, positionMs, p.durationMs, "stopped", p.sessionId) }
        runCatching { api.stop(p.sessionId) }
    }

    private fun finished() {
        val p = playback ?: return
        val r = request ?: return
        val exo = player ?: return
        val duration = exo.duration.takeIf { it != C.TIME_UNSET } ?: p.durationMs
        reporter?.cancel()
        lifecycleScope.launch {
            end(p, duration)
            playback = null
            val next = runCatching { api.next(r.itemId) }.getOrNull()
            if (next == null) {
                finish()
            } else {
                transcodeAsked = false
                start(PlayRequest(next.id, next.startMs, null, r.audio, r.sub, next.title), "auto", next.startMs)
            }
        }
    }

    private fun failed(error: PlaybackException) {
        val p = playback
        val r = request
        val undecodable = error.errorCode in setOf(
            PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
            PlaybackException.ERROR_CODE_DECODER_QUERY_FAILED,
            PlaybackException.ERROR_CODE_DECODING_FAILED,
            PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED,
            PlaybackException.ERROR_CODE_DECODING_FORMAT_EXCEEDS_CAPABILITIES,
            PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED,
            PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED,
        )
        if (p != null && r != null && p.method == "direct" && undecodable && !transcodeAsked) {
            // The device said it decodes this, and doesn't: ask for a transcode instead.
            transcodeAsked = true
            start(r, "transcode", player?.currentPosition ?: r.startMs)
            return
        }
        Toast.makeText(this, "Playback failed: ${error.errorCodeName}", Toast.LENGTH_LONG).show()
        finish()
    }

    override fun onStop() {
        super.onStop()
        player?.pause()
        if (isFinishing) release()
    }

    override fun onDestroy() {
        release()
        super.onDestroy()
    }

    private fun release() {
        reporter?.cancel()
        val p = playback
        val position = player?.currentPosition ?: 0
        playback = null
        if (p != null) background.launch { end(p, position) }
        player?.release()
        player = null
    }

    companion object {
        private const val EXTRA_SERVER = "server"
        private const val EXTRA_REQUEST = "request"
        private const val REPORT_EVERY_MS = 10_000L

        /** Reports that must reach the server after the screen is gone. */
        private val background = CoroutineScope(SupervisorJob() + Dispatchers.IO)

        fun intent(context: Context, server: String, request: PlayRequest): Intent =
            Intent(context, PlayerActivity::class.java)
                .putExtra(EXTRA_SERVER, server)
                .putExtra(EXTRA_REQUEST, request.toJson())

        /** This installation, as the server tells players apart (one video at a time each). */
        fun clientId(context: Context): String {
            val prefs = context.getSharedPreferences("device", Context.MODE_PRIVATE)
            return prefs.getString("client_id", null) ?: "android-${UUID.randomUUID()}".also {
                prefs.edit().putString("client_id", it).apply()
            }
        }
    }
}
