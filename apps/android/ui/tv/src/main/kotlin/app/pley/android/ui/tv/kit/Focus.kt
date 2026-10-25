package app.pley.android.ui.tv.kit

import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusEventModifierNode
import androidx.compose.ui.focus.FocusRequesterModifierNode
import androidx.compose.ui.focus.FocusState
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.requestFocus
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.LayoutAwareModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.node.currentValueOf
import androidx.compose.ui.platform.InspectorInfo
import app.pley.android.designsystem.TvSafeArea
import kotlinx.coroutines.launch

/**
 * The rail, as content sees it: where focus goes when it leaves content on
 * the left (D-pad rule 3) and when Back leaves a top-level page. The shell
 * provides it; without one (tests, previews), leaving does nothing special.
 */
interface TvRailFocus {
    /**
     * Whether focus is in the rail, sent there while the page showing was
     * (Left before it had loaded, say): the page's initial focus leaves it
     * there. Not when the page was chosen in the rail, which it takes focus
     * from as ever.
     */
    val keepsFocus: Boolean

    /** Moves focus to the rail's current item, which opens the rail. */
    fun focusCurrent(): Boolean
}

val LocalTvRailFocus = staticCompositionLocalOf<TvRailFocus?> { null }

/**
 * Focus leaving this group to the left goes to the rail's current item
 * ([rail], [LocalTvRailFocus]'s), never to whatever is geometrically
 * nearest, such as another row's tile scrolled under the rail. Leaving it
 * to the right is refused when [stopRight], so Right stops at a row's end
 * (D-pad rule 3).
 *
 * Put it before the group's focusRestorer: the restorer's own exit (saving
 * the focused child) still runs first.
 */
fun Modifier.railOnLeftStopOnRight(rail: TvRailFocus?, stopRight: Boolean = true): Modifier =
    focusProperties {
        val inner = onExit
        onExit = {
            inner()
            when (requestedFocusDirection) {
                FocusDirection.Left -> if (rail != null) {
                    cancelFocusChange()
                    rail.focusCurrent()
                }
                FocusDirection.Right -> if (stopRight) cancelFocusChange()
                else -> Unit
            }
        }
    }

/**
 * Where focus was on a page, kept with the page's saved state, so coming
 * back to it (Back from an item) lands on the same tile (D-pad rule 2),
 * even though the page's composition was gone meanwhile.
 */
@Stable
class FocusMemory internal constructor(key: String?) {
    /** The last focused element's key; `null` before anything on the page had focus. */
    var key: String? = key
        internal set

    /** Set when the remembered element took focus back, so the page's initial focus stays put. */
    internal var restored = false

    internal companion object {
        val Saver = Saver<FocusMemory, String>({ it.key ?: "" }, { FocusMemory(it.ifEmpty { null }) })
    }
}

/** A page's [FocusMemory], saved with the page (its back stack entry). */
@Composable
fun rememberFocusMemory(): FocusMemory = rememberSaveable(saver = FocusMemory.Saver) { FocusMemory(null) }

/**
 * D-pad rule 1: this element takes focus once it is first laid out, so no
 * screen shows without a focused element. With [memory], it gives way to
 * the element focused last on the page ([rememberedFocus]) when that one
 * comes back. It never takes focus back from the rail when someone went
 * there while the page was loading ([TvRailFocus.keepsFocus]).
 *
 * Put it before the focusable it is for: `Modifier.initialFocus().focusable()`.
 */
fun Modifier.initialFocus(memory: FocusMemory? = null, enabled: Boolean = true): Modifier =
    if (enabled) this then InitialFocusElement(memory, key = null, initial = true) else this

/**
 * Remembers in [memory] when this element (called [key] on its page) has
 * focus, and takes focus back when the page is shown again with it
 * remembered. [initial] makes it the page's initial focus otherwise.
 */
fun Modifier.rememberedFocus(memory: FocusMemory, key: String, initial: Boolean = false): Modifier =
    this then InitialFocusElement(memory, key, initial)

private data class InitialFocusElement(
    val memory: FocusMemory?,
    val key: String?,
    val initial: Boolean,
) : ModifierNodeElement<InitialFocusNode>() {
    override fun create() = InitialFocusNode(memory, key, initial)

    override fun update(node: InitialFocusNode) {
        node.memory = memory
        node.key = key
        node.initial = initial
    }

    override fun InspectorInfo.inspectableProperties() {
        name = "initialFocus"
        properties["key"] = key
        properties["initial"] = initial
    }
}

private class InitialFocusNode(
    var memory: FocusMemory?,
    var key: String?,
    var initial: Boolean,
) : Modifier.Node(),
    FocusRequesterModifierNode,
    LayoutAwareModifierNode,
    FocusEventModifierNode,
    CompositionLocalConsumerModifierNode {
    private var placed = false

    /**
     * Takes focus while it is still on the page or nowhere: not once
     * someone has moved it to the rail ([TvRailFocus.keepsFocus]), as Left
     * while the page was still loading does. Asked when the request runs,
     * after the keys pressed meanwhile.
     */
    private fun takeFocus() {
        if (currentValueOf(LocalTvRailFocus)?.keepsFocus == true) return
        requestFocus()
    }

    override fun onPlaced(coordinates: LayoutCoordinates) {
        if (placed) return
        placed = true
        val memory = memory
        val key = key
        when {
            // The page remembers this element: it takes focus back.
            memory != null && key != null && memory.key == key -> {
                memory.restored = true
                coroutineScope.launch { takeFocus() }
            }
            !initial -> Unit
            // Nothing remembered: the page's initial focus.
            memory?.key == null -> coroutineScope.launch { takeFocus() }
            // Something else is remembered. If it doesn't come back (gone,
            // or not laid out), this one takes focus after all.
            else -> coroutineScope.launch {
                withFrameNanos { }
                withFrameNanos { }
                if (!memory.restored) takeFocus()
            }
        }
    }

    override fun onFocusEvent(focusState: FocusState) {
        val key = key ?: return
        if (focusState.isFocused) memory?.key = key
    }
}

/** D-pad rule 9: keeps text and focusable things inside the TV's safe area, 48 dp at the sides and 27 above and below. */
fun Modifier.tvSafeArea(): Modifier = padding(horizontal = TvSafeArea.Horizontal, vertical = TvSafeArea.Vertical)
