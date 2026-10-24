package app.pley.android

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.RenderProcessGoneDetail
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.activity.addCallback
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updatePadding
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature

/**
 * A server's web app, full screen. Videos go to [PlayerActivity] (through
 * `window.PleyAndroid`, which only the server's own pages get);
 * `/__app/servers` leads back to the list of servers, and links to other
 * sites open in the browser.
 */
class WebActivity : ComponentActivity() {
    private lateinit var web: WebView
    private lateinit var places: Places
    /** The origin `window.PleyAndroid` is offered to. */
    private var bridgeOrigin: String? = null
    /** The web view's page process went away, and the web view with it. */
    private var gone = false
    private var pendingFiles: ValueCallback<Array<Uri>>? = null
    private val pickFiles = registerForActivityResult(ActivityResultContracts.GetMultipleContents()) { uris ->
        pendingFiles?.onReceiveValue(uris.toTypedArray())
        pendingFiles = null
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val server = savedInstanceState?.getString(EXTRA_SERVER) ?: intent.getStringExtra(EXTRA_SERVER) ?: run {
            finish()
            return
        }
        places = Places(server)
        CookieManager.getInstance().setAcceptCookie(true)
        web = WebView(this).apply {
            setBackgroundColor(0xFF0B0D10.toInt())
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.mediaPlaybackRequiresUserGesture = false
            // Pages of servers have no business with this device's files.
            settings.allowFileAccess = false
            settings.allowContentAccess = false
            // The web app knows it runs here by this (src/lib/shell.ts).
            settings.userAgentString = "${settings.userAgentString} pleyAndroid/${BuildConfig.VERSION_NAME}"
            webViewClient = Client()
            webChromeClient = Chrome()
        }
        offerPlayer()
        // The page stays clear of the status and navigation bars (and the
        // keyboard): a web view's own padding wouldn't move its content.
        val root = FrameLayout(this).apply { setBackgroundColor(0xFF0B0D10.toInt()) }
        root.addView(web, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime())
            view.updatePadding(left = bars.left, top = bars.top, right = bars.right, bottom = bars.bottom)
            WindowInsetsCompat.CONSUMED
        }
        setContentView(root)
        onBackPressedDispatcher.addCallback(this) {
            if (web.canGoBack()) web.goBack() else finish()
        }
        if (savedInstanceState == null || web.restoreState(savedInstanceState) == null) web.loadUrl(server)
    }

    /**
     * Offers `window.PleyAndroid` (the native player) to the server's pages:
     * only to its origin and only to the page itself, not to frames in it.
     */
    private fun offerPlayer() {
        if (!WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) return
        val origin = Links.address(places.server) ?: return
        if (origin == bridgeOrigin) return
        if (bridgeOrigin != null) WebViewCompat.removeWebMessageListener(web, BRIDGE)
        WebViewCompat.addWebMessageListener(web, BRIDGE, setOf(origin)) { _, message, _, isMainFrame, _ ->
            val request = message.data?.takeIf { isMainFrame }?.let(PlayRequest::parse) ?: return@addWebMessageListener
            startActivity(PlayerActivity.intent(this, places.server, request))
        }
        bridgeOrigin = origin
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString(EXTRA_SERVER, places.server)
        web.saveState(outState)
    }

    override fun onResume() {
        super.onResume()
        web.onResume()
        // Back from the player: the web app refreshes progress and watched marks.
        web.evaluateJavascript("window.dispatchEvent(new Event('pley:resume'))", null)
    }

    override fun onPause() {
        web.onPause()
        CookieManager.getInstance().flush()
        super.onPause()
    }

    override fun onDestroy() {
        if (::web.isInitialized && !gone) web.destroy()
        super.onDestroy()
    }

    private inner class Client : WebViewClient() {
        // The page's process ended (out of memory, or a crash): open the
        // server again rather than take the app down with it.
        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            (view.parent as? ViewGroup)?.removeView(view)
            view.destroy()
            gone = true
            startActivity(intent(this@WebActivity, places.server))
            finish()
            return true
        }

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
            when (Links.navigation(request.url.toString(), places)) {
                Navigation.Allow -> false
                Navigation.Servers -> {
                    startActivity(ServersActivity.pick(this@WebActivity))
                    finish()
                    true
                }
                Navigation.Browser -> {
                    try {
                        startActivity(Intent(Intent.ACTION_VIEW, request.url))
                    } catch (_: ActivityNotFoundException) {
                        // No browser: the link stays closed.
                    }
                    true
                }
                Navigation.Ignore -> true
            }
    }

    private inner class Chrome : WebChromeClient() {
        // Picture uploads (profile pictures, posters).
        override fun onShowFileChooser(
            webView: WebView,
            callback: ValueCallback<Array<Uri>>,
            params: FileChooserParams,
        ): Boolean {
            pendingFiles?.onReceiveValue(null)
            pendingFiles = callback
            pickFiles.launch(params.acceptTypes.firstOrNull { it.isNotEmpty() } ?: "*/*")
            return true
        }
    }

    companion object {
        private const val EXTRA_SERVER = "server"
        /** `window.PleyAndroid` in the web app (src/lib/shell.ts). */
        private const val BRIDGE = "PleyAndroid"

        fun intent(context: Context, server: String): Intent =
            Intent(context, WebActivity::class.java).putExtra(EXTRA_SERVER, server)
    }
}
