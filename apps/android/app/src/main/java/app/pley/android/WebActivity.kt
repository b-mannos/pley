package app.pley.android

import android.annotation.SuppressLint
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.addCallback
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updatePadding

/** The web app's way to the native player (`window.PleyAndroid` there). */
class PlayerBridge(private val activity: Activity, private val server: String) {
    @JavascriptInterface
    fun play(json: String): Boolean {
        val request = PlayRequest.parse(json) ?: return false
        activity.runOnUiThread { activity.startActivity(PlayerActivity.intent(activity, server, request)) }
        return true
    }
}

/**
 * A server's web app, full screen. Videos go to [PlayerActivity] (through
 * [PlayerBridge]); `/__app/servers` leads back to the list of servers, and
 * links to other sites open in the browser.
 */
class WebActivity : ComponentActivity() {
    private lateinit var web: WebView
    private lateinit var server: String
    private var pendingFiles: ValueCallback<Array<Uri>>? = null
    private val pickFiles = registerForActivityResult(ActivityResultContracts.GetMultipleContents()) { uris ->
        pendingFiles?.onReceiveValue(uris.toTypedArray())
        pendingFiles = null
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        server = intent.getStringExtra(EXTRA_SERVER) ?: run {
            finish()
            return
        }
        CookieManager.getInstance().setAcceptCookie(true)
        web = WebView(this).apply {
            setBackgroundColor(0xFF0B0D10.toInt())
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.mediaPlaybackRequiresUserGesture = false
            // The web app knows it runs here by this (src/lib/shell.ts).
            settings.userAgentString = "${settings.userAgentString} pleyAndroid/${BuildConfig.VERSION_NAME}"
            webViewClient = Client()
            webChromeClient = Chrome()
            addJavascriptInterface(PlayerBridge(this@WebActivity, server), "PleyAndroid")
        }
        ViewCompat.setOnApplyWindowInsetsListener(web) { view, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.ime())
            view.updatePadding(left = bars.left, top = bars.top, right = bars.right, bottom = bars.bottom)
            WindowInsetsCompat.CONSUMED
        }
        setContentView(web)
        onBackPressedDispatcher.addCallback(this) {
            if (web.canGoBack()) web.goBack() else finish()
        }
        if (savedInstanceState == null || web.restoreState(savedInstanceState) == null) web.loadUrl(server)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
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
        if (::web.isInitialized) web.destroy()
        super.onDestroy()
    }

    private inner class Client : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
            when (Links.navigation(request.url.toString(), server)) {
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

        fun intent(context: Context, server: String): Intent =
            Intent(context, WebActivity::class.java).putExtra(EXTRA_SERVER, server)
    }
}
