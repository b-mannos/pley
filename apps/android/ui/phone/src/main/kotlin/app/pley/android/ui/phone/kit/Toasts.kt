package app.pley.android.ui.phone.kit

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalAccessibilityManager
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyMotion
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.reducedMotion
import kotlinx.coroutines.delay
import java.util.concurrent.atomic.AtomicLong

/** A short message over the page ("Marked as watched · Undo"). */
@Stable
class Toast internal constructor(
    val id: Long,
    val message: String,
    /** The action's label ("Undo", "Sign in"); null for none. */
    val actionLabel: String?,
    val onAction: (() -> Unit)?,
    /** Errors stay longer: 6 s rather than 3.5. */
    val error: Boolean,
) {
    internal val visible = MutableTransitionState(false).apply { targetState = true }
}

/**
 * The page's toasts (design spec §3.4): at most [MAX_TOASTS] at once,
 * the oldest going first. Each stays 3.5 s, errors 6 s, longer when a
 * screen reader asks for more time. The shell has one, above the
 * navigation bar.
 */
@Stable
class ToastHostState {
    private val ids = AtomicLong()
    internal val toasts = mutableStateListOf<Toast>()

    /** What shows now, oldest first. */
    val current: List<Toast> get() = toasts

    /** Shows [message], with [actionLabel] calling [onAction] (and closing it) when given. */
    fun show(message: String, actionLabel: String? = null, onAction: (() -> Unit)? = null, error: Boolean = false): Toast {
        val toast = Toast(ids.incrementAndGet(), message, actionLabel, onAction, error)
        toasts += toast
        while (toasts.size > MAX_TOASTS) toasts.removeAt(0)
        return toast
    }

    /** Closes [toast], if it still shows. */
    fun dismiss(toast: Toast) {
        toasts.remove(toast)
    }

    companion object {
        const val MAX_TOASTS = 4
        const val DURATION_MS = 3_500L
        const val ERROR_DURATION_MS = 6_000L
    }
}

/**
 * Draws [state]'s toasts stacked from the bottom of where it is put, the
 * newest lowest: [surface3][PleyColors.Surface3] cards with [fg][PleyColors.Fg]
 * text and the action in [accentBright][PleyColors.AccentBright]. Each is
 * announced as it appears.
 */
@Composable
fun ToastHost(state: ToastHostState, modifier: Modifier = Modifier) {
    val reduced = reducedMotion()
    val a11y = LocalAccessibilityManager.current
    Column(
        modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.Bottom),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        for (toast in state.toasts) {
            key(toast.id) {
                LaunchedEffect(toast) {
                    val base = if (toast.error) ToastHostState.ERROR_DURATION_MS else ToastHostState.DURATION_MS
                    val wait = a11y?.calculateRecommendedTimeoutMillis(
                        base,
                        containsIcons = false,
                        containsText = true,
                        containsControls = toast.actionLabel != null,
                    ) ?: base
                    delay(wait)
                    state.dismiss(toast)
                }
                AnimatedVisibility(
                    visibleState = toast.visible,
                    enter = if (reduced) fadeIn(PleyMotion.reveal()) else fadeIn(PleyMotion.overlay()) + slideInVertically(PleyMotion.overlay()) { it / 2 },
                    exit = fadeOut(PleyMotion.reveal()),
                ) {
                    ToastCard(toast) { state.dismiss(toast) }
                }
            }
        }
    }
}

@Composable
private fun ToastCard(toast: Toast, close: () -> Unit) {
    Row(
        Modifier
            .widthIn(max = 560.dp)
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .background(PleyColors.Surface3, PleyShape.Card)
            .padding(start = 16.dp, end = if (toast.actionLabel != null) 4.dp else 16.dp, top = 6.dp, bottom = 6.dp)
            .semantics(mergeDescendants = true) { liveRegion = if (toast.error) LiveRegionMode.Assertive else LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            toast.message,
            style = MaterialTheme.typography.bodyMedium,
            color = PleyColors.Fg,
            modifier = Modifier.weight(1f).padding(vertical = 8.dp),
        )
        val label = toast.actionLabel
        if (label != null) {
            Spacer(Modifier.padding(start = 8.dp))
            TextButton(
                onClick = {
                    close()
                    toast.onAction?.invoke()
                },
                colors = ButtonDefaults.textButtonColors(contentColor = PleyColors.AccentBright),
            ) {
                Text(label, style = MaterialTheme.typography.labelLarge)
            }
        }
    }
}
