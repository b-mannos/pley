package app.pley.android.screens.onboarding

import androidx.lifecycle.viewModelScope
import app.pley.android.auth.AuthException
import app.pley.android.auth.SignInState
import app.pley.android.auth.TOO_OLD
import app.pley.android.auth.UNENCRYPTED
import app.pley.android.screens.PleyKey
import java.net.ConnectException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/** A phone's sign-in in the browser: its states, its errors in the module's words, and a closed tab. */
@OptIn(ExperimentalCoroutinesApi::class)
class SignInViewModelTest {
    private val server = FakeOnboardingSource.SERVER
    private val source = FakeOnboardingSource()
    private var vm: SignInViewModel? = null

    @After
    fun tearDown() {
        vm?.viewModelScope?.cancel()
        Dispatchers.resetMain()
    }

    private fun TestScope.signIn(): SignInViewModel {
        Dispatchers.setMain(StandardTestDispatcher(testScheduler))
        return SignInViewModel(source).also { vm = it }
    }

    @Test
    fun opensWaitsFinishesThenHome() = runTest {
        val vm = signIn()
        vm.signIn(null, server)
        assertEquals(BrowserSignIn.Opening, vm.state.value)
        advanceUntilIdle()
        assertEquals(listOf(server), source.browserSignIns)
        assertEquals(BrowserSignIn.Waiting, vm.state.value)
        source.signInState.value = SignInState.Finishing
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Finishing, vm.state.value)
        source.signInState.value = SignInState.SignedIn(server)
        advanceUntilIdle()
        assertEquals(BrowserSignIn.SignedIn, vm.state.value)
        assertEquals(1, source.refreshes)
        assertEquals(OnboardingMove.Restart(PleyKey.Home), vm.navigation.first())
    }

    @Test
    fun closingTheTabCancelsWithoutAWord() = runTest {
        val vm = signIn()
        vm.signIn(null, server)
        advanceUntilIdle()
        vm.paused()
        vm.resumed()
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Idle, vm.state.value)
    }

    @Test
    fun comingBackWithAnAnswerIsntACancel() = runTest {
        val vm = signIn()
        vm.signIn(null, server)
        advanceUntilIdle()
        vm.paused()
        source.signInState.value = SignInState.Finishing
        vm.resumed()
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Finishing, vm.state.value)
    }

    @Test
    fun anOlderServerAnUnencryptedOneAndOneThatDoesntAnswer() = runTest {
        val vm = signIn()
        source.browserError = AuthException(TOO_OLD)
        vm.signIn(null, server)
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Failed(server, SignInProblem.TooOld, TOO_OLD), vm.state.value)
        vm.dismiss()
        source.browserError = AuthException(UNENCRYPTED)
        vm.signIn(null, server)
        advanceUntilIdle()
        assertEquals(SignInProblem.Unencrypted, (vm.state.value as BrowserSignIn.Failed).problem)
        source.browserError = ConnectException("refused")
        vm.signIn(null, server)
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Failed(server, SignInProblem.Unreachable, "nothing answers there"), vm.state.value)
    }

    @Test
    fun aRefusalFromTheServerShowsItsWords() = runTest {
        val vm = signIn()
        vm.signIn(null, server)
        advanceUntilIdle()
        source.signInState.value = SignInState.Failed("you didn't allow the app")
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Failed(server, SignInProblem.Refused, "you didn't allow the app"), vm.state.value)
    }

    @Test
    fun withoutABrowserItSignsInWithACode() = runTest {
        val vm = signIn()
        source.browserError = AuthException("there's no browser on this device to sign in with")
        vm.signIn(null, server)
        advanceUntilIdle()
        assertEquals(BrowserSignIn.Idle, vm.state.value)
        assertEquals(OnboardingMove.Open(PleyKey.Link(server)), vm.navigation.first())
    }
}
