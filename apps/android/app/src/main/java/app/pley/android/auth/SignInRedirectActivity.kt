package app.pley.android.auth

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Where the browser comes back after signing in
 * (`app.pley.android:/oauth/callback`). Any app can open this address, so
 * the answer counts only when it carries the state of a waiting sign-in
 * and comes from that sign-in's server ([SignIn.handle]); the rest is
 * ignored. It shows nothing: it hands the answer over, brings back the
 * activity that started the sign-in (closing the Custom Tab above it), and
 * ends.
 */
class SignInRedirectActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Recreated after process death: the answer was handled the first time.
        if (savedInstanceState == null) handle(intent)
        finish()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
        finish()
    }

    private fun handle(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val uri = intent.dataString ?: return
        Auth.callback(this, uri) { flow ->
            val back = flow.returnTo ?: return@callback
            try {
                // Only the app's own activities: the class name was kept by the app itself.
                startActivity(
                    Intent().setClassName(packageName, back)
                        .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
                )
            } catch (_: android.content.ActivityNotFoundException) {
                // That screen is gone from the app: Auth.state still says how it went.
            }
        }
    }
}
