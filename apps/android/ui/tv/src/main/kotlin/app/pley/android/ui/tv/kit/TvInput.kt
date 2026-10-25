package app.pley.android.ui.tv.kit

import android.view.KeyEvent as AndroidKeyEvent
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.text.input.TextFieldLineLimits
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyType

/**
 * Text entry on TV (D-pad rule 7; design spec §2.11), only for search and
 * server addresses. Focus lands on the field without opening the
 * keyboard, so D-pad users pass through it; Select opens the system
 * keyboard (Gboard, or Amazon's), and Back or the keyboard's action closes
 * it again, leaving focus on the field.
 *
 * 48 dp, [surface2][PleyColors.Surface2], radius 8, tvBody. At rest a 1 dp
 * [control][PleyColors.Control] edge; focused a 2 dp [accentBright][PleyColors.AccentBright]
 * edge, without scale (fields are wide).
 */
@Composable
fun TvTextField(
    state: TextFieldState,
    placeholder: String,
    modifier: Modifier = Modifier,
    keyboardType: KeyboardType = KeyboardType.Text,
    imeAction: ImeAction = ImeAction.Search,
    onAction: () -> Unit = {},
    focusModifier: Modifier = Modifier,
) {
    var editing by remember { mutableStateOf(false) }
    val outer = remember { MutableInteractionSource() }
    val outerFocused by outer.collectIsFocusedAsState()
    var innerFocused by remember { mutableStateOf(false) }
    val field = remember { FocusRequester() }
    val shell = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current

    fun stop() {
        editing = false
        keyboard?.hide()
        shell.requestFocus()
    }

    BackHandler(enabled = editing) { stop() }
    LaunchedEffect(editing) {
        if (editing) {
            field.requestFocus()
            keyboard?.show()
        }
    }
    val ring = outerFocused || innerFocused
    Box(
        modifier
            .height(48.dp)
            .fillMaxWidth()
            .background(PleyColors.Surface2, PleyShape.Field)
            .border(if (ring) 2.dp else 1.dp, if (ring) PleyColors.AccentBright else PleyColors.Control, PleyShape.Field)
            .then(focusModifier)
            .focusRequester(shell)
            .focusProperties { canFocus = !editing }
            .onKeyEvent { event ->
                val code = event.nativeKeyEvent.keyCode
                val select = code == AndroidKeyEvent.KEYCODE_DPAD_CENTER || code == AndroidKeyEvent.KEYCODE_ENTER ||
                    code == AndroidKeyEvent.KEYCODE_NUMPAD_ENTER
                if (select && event.type == KeyEventType.KeyUp && !editing) {
                    editing = true
                    true
                } else {
                    select
                }
            }
            .focusable(interactionSource = outer)
            .padding(horizontal = 16.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (state.text.isEmpty()) {
            BasicText(placeholder, style = PleyType.Tv.Body.copy(color = PleyColors.Muted), maxLines = 1)
        }
        BasicTextField(
            state = state,
            modifier = Modifier
                .fillMaxWidth()
                .focusRequester(field)
                .focusProperties { canFocus = editing }
                .onFocusChanged {
                    innerFocused = it.isFocused
                    // The keyboard closed (Back, or its own close key): back to the field.
                    if (!it.isFocused && editing) editing = false
                },
            textStyle = PleyType.Tv.Body.copy(color = PleyColors.Fg),
            cursorBrush = SolidColor(PleyColors.AccentBright),
            lineLimits = TextFieldLineLimits.SingleLine,
            keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction),
            onKeyboardAction = {
                onAction()
                stop()
            },
        )
    }
}

/**
 * D-pad rule 10: the remote's digit keys type into pads (PIN, address).
 * [onDigit] gets 0 to 9 from the number row and the number pad; [onDelete]
 * gets Delete and Clear. Other keys pass on.
 */
fun Modifier.digitKeys(onDigit: (Int) -> Unit, onDelete: () -> Unit = {}): Modifier = onKeyEvent { event ->
    if (event.type != KeyEventType.KeyDown) return@onKeyEvent false
    when (val code = event.nativeKeyEvent.keyCode) {
        in AndroidKeyEvent.KEYCODE_0..AndroidKeyEvent.KEYCODE_9 -> {
            onDigit(code - AndroidKeyEvent.KEYCODE_0)
            true
        }
        in AndroidKeyEvent.KEYCODE_NUMPAD_0..AndroidKeyEvent.KEYCODE_NUMPAD_9 -> {
            onDigit(code - AndroidKeyEvent.KEYCODE_NUMPAD_0)
            true
        }
        AndroidKeyEvent.KEYCODE_DEL, AndroidKeyEvent.KEYCODE_FORWARD_DEL, AndroidKeyEvent.KEYCODE_CLEAR -> {
            onDelete()
            true
        }
        else -> false
    }
}
