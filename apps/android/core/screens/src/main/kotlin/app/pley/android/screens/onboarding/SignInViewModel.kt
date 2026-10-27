package app.pley.android.screens.onboarding

import android.app.Activity
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.pley.android.auth.AuthException
import app.pley.android.auth.SignInState
import app.pley.android.auth.TOO_OLD
import app.pley.android.auth.UNENCRYPTED
import app.pley.android.common.describe
import app.pley.android.screens.PleyKey
import java.io.IOException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch

/** Why a browser sign-in didn't go through, for the screen to word. */
enum class SignInProblem {
    /** The server has no sign-in for apps: "Update pley on <server>…", with Open in browser. */
    TooOld,

    /** Plain http beyond the home network. */
    Unencrypted,

    /** The server couldn't be reached. */
    Unreachable,

    /** The server or the sign-in refused, in its own words. */
    Refused,
}

/** Where a phone's browser sign-in stands. */
sealed interface BrowserSignIn {
    data object Idle : BrowserSignIn

    /** Reading the server's sign-in details, then opening the browser. */
    data object Opening : BrowserSignIn

    /** The browser shows the server's page. */
    data object Waiting : BrowserSignIn

    /** The browser came back; the app collects its tokens. */
    data object Finishing : BrowserSignIn

    data object SignedIn : BrowserSignIn

    data class Failed(val server: String, val problem: SignInProblem, val message: String) : BrowserSignIn
}

/**
 * Signing a phone in to a server on the server's own page, in a private
 * Custom Tab (the sign-in module's PKCE flow): no password or two-step
 * field in the app. The states are opening, waiting and finishing; closing
 * the tab cancels without a word; a phone without a browser signs in with
 * a code from another device (the Link screen).
 *
 * The server signed in to is kept in [saved] while the browser shows its
 * page: Android may end the app meanwhile, and the browser's answer then
 * comes back to this screen made again, which still takes it.
 */
class SignInViewModel(
    private val source: OnboardingSource,
    private val saved: SavedStateHandle,
) : ViewModel() {
    private val _state = MutableStateFlow<BrowserSignIn>(BrowserSignIn.Idle)
    val state: StateFlow<BrowserSignIn> = _state.asStateFlow()

    private val moves = Channel<OnboardingMove>(Channel.BUFFERED)

    /** Where to go: Home once signed in, the Link screen without a browser. */
    val navigation: Flow<OnboardingMove> = moves.receiveAsFlow()

    /** The sign-in's server, from its start until it ends (signed in, or back to the list). */
    private var server: String?
        get() = saved[SERVER]
        set(value) {
            saved[SERVER] = value
        }
    private var job: Job? = null

    /** The browser covered the screen since it was opened. */
    private var away = false

    init {
        // Made again with a sign-in under way: the app was ended while the browser
        // showed the page. Its answer, if there is one, is (or soon will be) in
        // the sign-in module's state; without one, the tab was closed.
        if (server != null) {
            _state.value = BrowserSignIn.Waiting
            away = true
        }
        viewModelScope.launch {
            source.signInState.collect { auth ->
                // Only the sign-in this screen started.
                if (server == null || _state.value == BrowserSignIn.Idle) return@collect
                when (auth) {
                    SignInState.Idle -> Unit
                    SignInState.Finishing -> _state.value = BrowserSignIn.Finishing
                    is SignInState.SignedIn -> signedIn()
                    is SignInState.Failed -> _state.value = BrowserSignIn.Failed(server.orEmpty(), SignInProblem.Refused, auth.message)
                }
            }
        }
    }

    /** Opens [server]'s sign-in page from [activity]. */
    fun signIn(activity: Activity?, server: String) {
        if (job?.isActive == true) return
        this.server = server
        away = false
        _state.value = BrowserSignIn.Opening
        job = viewModelScope.launch {
            try {
                source.signInWithBrowser(activity, server)
                if (_state.value == BrowserSignIn.Opening) _state.value = BrowserSignIn.Waiting
            } catch (e: CancellationException) {
                throw e
            } catch (e: AuthException) {
                val message = e.message.orEmpty()
                when {
                    NO_BROWSER in message -> {
                        this@SignInViewModel.server = null
                        _state.value = BrowserSignIn.Idle
                        moves.send(OnboardingMove.Open(PleyKey.Link(server)))
                    }
                    message == TOO_OLD -> _state.value = BrowserSignIn.Failed(server, SignInProblem.TooOld, message)
                    message == UNENCRYPTED -> _state.value = BrowserSignIn.Failed(server, SignInProblem.Unencrypted, message)
                    else -> _state.value = BrowserSignIn.Failed(server, SignInProblem.Refused, message)
                }
            } catch (e: IOException) {
                _state.value = BrowserSignIn.Failed(server, SignInProblem.Unreachable, describe(e))
            }
        }
    }

    /** The screen went under the browser. */
    fun paused() {
        if (_state.value == BrowserSignIn.Waiting || _state.value == BrowserSignIn.Opening) away = true
    }

    /**
     * The screen is back. If the browser didn't come back with an answer
     * (the tab was closed), the sign-in was cancelled: back to the list,
     * without a word.
     */
    fun resumed() {
        if (!away) return
        away = false
        viewModelScope.launch {
            // The answer, when there is one, is handed over before the screen comes back; a moment for it to arrive.
            delay(SETTLE_MS)
            if (_state.value == BrowserSignIn.Waiting && source.signInState.value == SignInState.Idle) {
                server = null
                _state.value = BrowserSignIn.Idle
            }
        }
    }

    /** The error was read: back to the list. */
    fun dismiss() {
        if (_state.value !is BrowserSignIn.Failed) return
        server = null
        _state.value = BrowserSignIn.Idle
    }

    private suspend fun signedIn() {
        if (_state.value == BrowserSignIn.SignedIn) return
        server = null
        _state.value = BrowserSignIn.SignedIn
        // Tokens that replaced others for the same server (another profile): who the app is follows.
        runCatching { source.session.refresh() }
        moves.send(OnboardingMove.Restart(PleyKey.Home))
    }

    private companion object {
        /** What the sign-in module says when no browser can open the page (SignIn.open). */
        const val NO_BROWSER = "no browser"
        const val SETTLE_MS = 300L
        const val SERVER = "sign-in.server"
    }
}
