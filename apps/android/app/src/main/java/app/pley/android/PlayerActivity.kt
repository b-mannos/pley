package app.pley.android

import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.Gravity
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.CookieManager
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.annotation.OptIn
import androidx.core.content.edit
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
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import androidx.media3.ui.SubtitleView
import androidx.media3.common.VideoSize
import androidx.media3.common.text.CueGroup
import java.util.UUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient

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
    private lateinit var zoomButton: TextView
    private lateinit var backButton: TextView
    private lateinit var header: LinearLayout
    private var fill = false
    private var zoom = 1f
    private lateinit var subtitles: SubtitleView
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
        // The picture may use the whole screen, beside the camera cutout too:
        // filling the width is up to the viewer (pinch, or the Fill button).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
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
            gravity = Gravity.CENTER_VERTICAL
            // The title is also a way back, for a thumb that misses the arrow.
            isClickable = true
            setOnClickListener { onBackPressedDispatcher.onBackPressed() }
        }
        val touch = (56 * resources.displayMetrics.density).toInt()
        backButton = TextView(this).apply {
            text = "←"
            setTextColor(0xFFFFFFFF.toInt())
            textSize = 30f
            gravity = Gravity.CENTER
            minWidth = touch
            minHeight = touch
            setPadding(touch / 3, touch / 4, touch / 6, touch / 4)
            setShadowLayer(8f, 0f, 2f, 0xCC000000.toInt())
            isClickable = true
            isFocusable = true
            contentDescription = "Back"
            setOnClickListener { onBackPressedDispatcher.onBackPressed() }
        }
        zoomButton = TextView(this).apply {
            setTextColor(0xFFFFFFFF.toInt())
            textSize = 16f
            setPadding(48, 36, 48, 36)
            setShadowLayer(8f, 0f, 2f, 0xCC000000.toInt())
            isClickable = true
            isFocusable = true
            setOnClickListener { nextZoom() }
        }
        view.setControllerVisibilityListener(PlayerView.ControllerVisibilityListener { visibility ->
            header.visibility = visibility
            zoomButton.visibility = visibility
        })
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(backButton)
            addView(title)
        }
        this.header = header
        // Subtitles are drawn over the screen, not inside the video frame, so
        // they stay in view however far the picture is zoomed.
        view.subtitleView?.visibility = android.view.View.GONE
        subtitles = SubtitleView(this).apply {
            setUserDefaultStyle()
            setUserDefaultTextSize()
        }
        view.addOnLayoutChangeListener { _, l, t, r, b, ol, ot, or, ob ->
            // A turned screen changes what covering it takes.
            if (fill && (r - l != or - ol || b - t != ob - ot)) view.post { setZoom(coverZoom()) }
        }
        val root = FrameLayout(this)
        root.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(subtitles, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        root.addView(
            header,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.TOP or Gravity.START),
        )
        root.addView(
            zoomButton,
            FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.TOP or Gravity.END),
        )
        setContentView(root)
        setFill(getSharedPreferences("player", Context.MODE_PRIVATE).getBoolean("fill", false))
        start(first, mode = "auto", startMs = first.startMs)
    }

    /**
     * Fill zooms the picture until it covers the whole screen (a 16:9 or
     * 2.39:1 film on a 20:9 phone loses a little at the edges); fit shows all
     * of it. Pinching zooms freely in between and beyond, for films whose
     * black bars are part of the picture. Fill or fit is remembered.
     */
    private fun setFill(on: Boolean) {
        fill = on
        setZoom(if (on) coverZoom() else 1f)
        getSharedPreferences("player", Context.MODE_PRIVATE).edit { putBoolean("fill", on) }
    }

    /**
     * The button steps through fit, fill and zoom. Zoom goes one step past
     * fill for wide films stored with black bars inside a 16:9 picture: it
     * grows the picture until a 2.39:1 image covers the screen's height.
     */
    private fun nextZoom() {
        val cover = coverZoom()
        val crop = cover * barsCrop()
        when {
            zoom <= 1.001f -> setFill(true)
            zoom < crop - 0.01f && crop > cover + 0.02f -> {
                fill = true
                setZoom(crop)
            }
            else -> setFill(false)
        }
    }

    /** How much more than fill it takes to hide 2.39:1 bars stored in a 16:9 picture. */
    private fun barsCrop(): Float {
        val size = player?.videoSize ?: return 1f
        if (size.width == 0 || size.height == 0 || view.width == 0 || view.height == 0) return 1f
        val video = size.width * size.pixelWidthHeightRatio / size.height
        val screen = view.width.toFloat() / view.height
        // Only pictures narrower than the screen can hide bars this way.
        return if (video < 2f && screen > video) maxOf(1f, SCOPE / screen) else 1f
    }

    /** How much the fitted picture must grow to cover the screen. */
    private fun coverZoom(): Float {
        val size = player?.videoSize ?: return 1f
        if (size.width == 0 || size.height == 0 || view.width == 0 || view.height == 0) return 1f
        val video = size.width * size.pixelWidthHeightRatio / size.height
        val screen = view.width.toFloat() / view.height
        return maxOf(video / screen, screen / video)
    }

    /**
     * Zooms by giving the video frame a box larger than the screen; the screen
     * shows its middle. The frame keeps fitting the picture inside that box.
     * Subtitles burned into the picture sit at its bottom, so then the bottom
     * stays on screen and only the top is cropped.
     */
    private fun setZoom(value: Float) {
        zoom = value.coerceIn(1f, MAX_ZOOM)
        val frame = view.findViewById<AspectRatioFrameLayout>(androidx.media3.ui.R.id.exo_content_frame)
        frame ?: return
        val params = frame.layoutParams as? FrameLayout.LayoutParams ?: return
        val burned = playback?.burnSubtitles == true
        if (zoom <= 1.001f || view.width == 0) {
            params.width = ViewGroup.LayoutParams.MATCH_PARENT
            params.height = ViewGroup.LayoutParams.MATCH_PARENT
            params.gravity = Gravity.CENTER
        } else {
            params.width = (view.width * zoom).toInt()
            params.height = (view.height * zoom).toInt()
            params.gravity = if (burned) Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL else Gravity.CENTER
        }
        frame.layoutParams = params
        // The button names what it does next.
        val cover = coverZoom()
        val crop = cover * barsCrop()
        val next = when {
            zoom <= 1.001f -> "Fill"
            zoom < crop - 0.01f && crop > cover + 0.02f -> "Zoom"
            else -> "Fit"
        }
        zoomButton.text = next
        zoomButton.contentDescription = when (next) {
            "Fill" -> "Fill the screen"
            "Zoom" -> "Zoom past the black bars"
            else -> "Show the whole picture"
        }
    }

    // Pinching zooms the picture, as in other video apps; taps still reach
    // the controls.
    private val pinch by lazy {
        ScaleGestureDetector(
            this,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    setZoom(zoom * detector.scaleFactor)
                    return true
                }

                override fun onScaleEnd(detector: ScaleGestureDetector) {
                    // Close to covering the screen counts as fill.
                    val cover = coverZoom()
                    if (zoom > 1.001f && kotlin.math.abs(zoom - cover) < 0.04f) setZoom(cover)
                    fill = zoom > 1.001f
                    getSharedPreferences("player", Context.MODE_PRIVATE).edit { putBoolean("fill", fill) }
                }
            },
        )
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        pinch.onTouchEvent(event)
        // A two-finger gesture is a pinch: the controls don't see it.
        if (event.pointerCount > 1 || pinch.isInProgress) return true
        return super.dispatchTouchEvent(event)
    }

    private fun newPlayer(): ExoPlayer {
        val home = Links.address(server)
        val client = OkHttpClient.Builder()
            // Never from https down to http.
            .followSslRedirects(false)
            // The session token goes to the server only: not to another site a
            // playlist or a redirect points to.
            .addNetworkInterceptor { chain ->
                val request = chain.request()
                val signed = if (token != null && Links.address(request.url.toString()) == home) {
                    request.newBuilder().header("Authorization", "Bearer $token").build()
                } else {
                    request
                }
                chain.proceed(signed)
            }
            .build()
        val http = OkHttpDataSource.Factory(client).setUserAgent(USER_AGENT)
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

            override fun onCues(cueGroup: CueGroup) {
                subtitles.setCues(cueGroup.cues)
            }

            override fun onVideoSizeChanged(videoSize: VideoSize) {
                // Covering the screen depends on the picture's shape.
                if (fill) setZoom(coverZoom())
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
                if (fill) setZoom(coverZoom())
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
                // Paused too: the server forgets players it hasn't heard from.
                val p = player
                if (p?.isPlaying == true) {
                    report("playing")
                } else if (p?.playbackState == Player.STATE_READY) {
                    report("paused")
                }
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
        private const val MAX_ZOOM = 3f

        /** The widest common film shape (CinemaScope). */
        private const val SCOPE = 2.39f

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
                prefs.edit { putString("client_id", it) }
            }
        }
    }
}
